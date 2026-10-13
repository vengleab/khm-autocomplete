//! The protocol end to end over Unix sockets: client, server, deadlines, bad input, the breaker.
#![cfg(unix)]
use khipc::*;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Fake {
    calls: Arc<AtomicUsize>,
    delay: Duration,
}
impl Handler for Fake {
    fn vocab(&self) -> u64 {
        277
    }
    fn suggest(&self, text: &str, mode: u8, just: Option<&str>) -> Result<String, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(self.delay);
        if text == "boom" {
            panic!("handler bug");
        }
        if text == "refuse" {
            return Err("no".into());
        }
        Ok(format!(
            "[{{\"word\":\"w{mode}{}\",\"kind\":\"exact\",\"replace\":{},\"replace_len_utf16\":{},\"append\":false,\"prob\":0.5,\"roman\":null,\"why\":null,\"gain\":2}}]",
            just.unwrap_or(""),
            serde_json::to_string(text).unwrap(),
            text.encode_utf16().count()
        ))
    }
    fn key_action(&self, text: &str, _mode: u8, key: u8, _just: Option<&str>) -> Result<String, String> {
        if key == 0 && !text.is_empty() {
            Ok("{\"action\":\"insert\",\"candidate\":{\"word\":\"ខ្ញុំ\",\"kind\":\"exact\",\"replace\":\"knyom\",\"replace_len_utf16\":5,\"append\":false,\"prob\":null,\"roman\":\"knhom\",\"why\":null,\"gain\":0}}".into())
        } else {
            Ok("{\"action\":\"pass\"}".into())
        }
    }
}

fn pair(delay_ms: u64) -> (Client<UnixTimed>, Arc<AtomicUsize>, std::thread::JoinHandle<()>) {
    let (a, b) = UnixStream::pair().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let h = Fake { calls: calls.clone(), delay: Duration::from_millis(delay_ms) };
    let t = std::thread::spawn(move || {
        let _ = serve_connection(b, &h);
    });
    (Client::new(UnixTimed(a)), calls, t)
}
const T: Duration = Duration::from_millis(500);

#[test]
fn ping_suggest_and_key_action() {
    let (mut c, _, _) = pair(0);
    assert_eq!(c.ping(T).unwrap(), PingInfo { v: 1, vocab: 277 });
    let s = c.suggest("ស្រ", 0, Some("ខ្ញុំ"), T).unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!((s[0].word.as_str(), s[0].replace.as_str(), s[0].replace_len_utf16, s[0].gain), ("w0ខ្ញុំ", "ស្រ", 3, 2));
    match c.key_action("knyom", 1, 0, None, T).unwrap() {
        KeyAction::Insert(x) => {
            assert_eq!((x.word.as_str(), x.replace_len_utf16, x.roman.as_deref()), ("ខ្ញុំ", 5, Some("knhom")))
        }
        a => panic!("{a:?}"),
    }
    assert_eq!(c.key_action("", 1, 0, None, T).unwrap(), KeyAction::PassThrough);
}

#[test]
fn a_slow_engine_times_out_within_the_deadline() {
    let (mut c, _, _) = pair(400);
    let t0 = Instant::now();
    let e = c.suggest("x", 0, None, Duration::from_millis(60)).unwrap_err();
    assert!(matches!(e, ClientError::Timeout), "{e}");
    assert!(t0.elapsed() < Duration::from_millis(250), "must give up near the deadline, took {:?}", t0.elapsed());
}

#[test]
fn engine_errors_and_panics_are_answers_not_crashes() {
    let (mut c, _, _) = pair(0);
    assert!(matches!(c.suggest("refuse", 0, None, T).unwrap_err(), ClientError::Engine(m) if m == "no"));
    // a panicking handler is reported, and the same connection keeps working
    assert!(matches!(c.suggest("boom", 0, None, T).unwrap_err(), ClientError::Engine(_)));
    assert_eq!(c.suggest("ok", 2, None, T).unwrap()[0].word, "w2");
}

#[test]
fn a_dead_engine_is_unavailable() {
    let (mut c, _, t) = pair(0);
    drop(c.ping(T));
    let (a, b) = UnixStream::pair().unwrap();
    drop(b);
    let mut dead = Client::new(UnixTimed(a));
    assert!(matches!(dead.ping(T).unwrap_err(), ClientError::Unavailable(_)));
    drop(c);
    t.join().unwrap();
}

#[test]
fn server_rejects_garbage_and_oversize_frames() {
    // not JSON: an error response, connection stays usable
    let (a, b) = UnixStream::pair().unwrap();
    let _t = std::thread::spawn(move || {
        let _ = serve_connection(b, &Fake { calls: Arc::new(AtomicUsize::new(0)), delay: Duration::ZERO });
    });
    let mut a = a;
    write_frame(&mut a, b"this is not json").unwrap();
    let r = read_frame(&mut a).unwrap().unwrap();
    assert!(String::from_utf8(r).unwrap().contains("\"ok\":false"));
    // wrong protocol version
    write_frame(&mut a, br#"{"v":99,"id":5,"op":"ping"}"#).unwrap();
    let r = String::from_utf8(read_frame(&mut a).unwrap().unwrap()).unwrap();
    assert!(r.contains("\"id\":5") && r.contains("not supported"), "{r}");
    // a frame header announcing more than MAX_FRAME ends the connection
    a.write_all(&((MAX_FRAME as u32) + 1).to_le_bytes()).unwrap();
    let mut buf = [0u8; 1];
    assert_eq!(a.read(&mut buf).unwrap_or(0), 0, "server must hang up");
}

#[test]
fn read_frame_handles_split_reads_and_eof() {
    struct Dribble(Vec<u8>, usize);
    impl Read for Dribble {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            if self.1 >= self.0.len() {
                return Ok(0);
            }
            b[0] = self.0[self.1];
            self.1 += 1;
            Ok(1)
        }
    }
    let mut v = Vec::new();
    write_frame(&mut v, b"hello").unwrap();
    assert_eq!(read_frame(&mut Dribble(v.clone(), 0)).unwrap().unwrap(), b"hello");
    assert!(read_frame(&mut Dribble(vec![], 0)).unwrap().is_none(), "clean EOF between frames");
    assert!(read_frame(&mut Dribble(v[..6].to_vec(), 0)).is_err(), "EOF inside a frame is an error");
    assert!(write_frame(&mut Vec::new(), &vec![0u8; MAX_FRAME + 1]).is_err());
}

#[test]
fn breaker_opens_after_repeated_failures_and_recovers() {
    let t0 = Instant::now();
    let mut b = Breaker::new(2, Duration::from_secs(3));
    assert!(b.allow(t0));
    b.failure(t0);
    assert!(b.allow(t0), "one failure does not open it");
    b.failure(t0);
    assert!(!b.allow(t0 + Duration::from_secs(1)), "open during the cooldown");
    assert!(!b.allow(t0 + Duration::from_millis(2999)));
    assert!(b.allow(t0 + Duration::from_secs(3)), "one trial after the cooldown");
    b.failure(t0 + Duration::from_secs(3));
    assert!(!b.allow(t0 + Duration::from_secs(4)), "a failed trial reopens it at once");
    assert!(b.allow(t0 + Duration::from_secs(7)));
    b.success();
    b.failure(t0 + Duration::from_secs(7));
    assert!(b.allow(t0 + Duration::from_secs(7)), "success reset the count");
}

#[test]
fn skipped_calls_never_reach_the_engine() {
    // what the DLL does: consult the breaker first; with the engine dead it makes no calls after the threshold
    let mut b = Breaker::new(2, Duration::from_secs(5));
    let t0 = Instant::now();
    let mut attempts = 0;
    for i in 0..50u64 {
        let now = t0 + Duration::from_millis(10 * i);
        if b.allow(now) {
            attempts += 1;
            b.failure(now); // the engine is dead
        }
    }
    assert_eq!(attempts, 2, "50 key presses, only 2 slow attempts");
}
