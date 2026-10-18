//! IPC between the Windows text service (a DLL loaded into every app) and `khengine.exe`.
//!
//! Rules this crate exists to enforce (the DLL must be tiny and crash-safe):
//! - a call has a hard deadline and never blocks the host app longer than that;
//! - when the engine is down or slow, a `Breaker` stops asking for a while and keys pass through untouched;
//! - frames are bounded and every parse error is an `Err`, never a panic;
//! - no language data and no dependency on the engine crate.
//!
//! Wire format: a frame is a `u32` little-endian length followed by that many bytes of UTF-8 JSON.
//! Request:  `{"v":1,"id":7,"op":"suggest","text":"...","mode":1,"just_accepted":null}`
//! Response: `{"id":7,"ok":true,"result":<json>}` or `{"id":7,"ok":false,"error":"..."}`.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

#[cfg(windows)]
pub mod winpipe;

pub const PROTOCOL_VERSION: u32 = 1;
/// Largest frame either side accepts. Requests carry a few dozen words of context, so this is generous.
pub const MAX_FRAME: usize = 64 * 1024;

// ------------------------------------------------------------------------------------------------ messages

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Ping,
    /// candidates for the text before the caret; `mode`: 0 Khmer, 1 Latin, 2 English, 3 Auto
    Suggest {
        text: String,
        mode: u8,
        just_accepted: Option<String>,
    },
    /// Space (0) / Enter (1) / Tab (2)
    KeyAction {
        text: String,
        mode: u8,
        key: u8,
        just_accepted: Option<String>,
    },
}

#[derive(Serialize, Deserialize)]
struct Envelope<T> {
    v: u32,
    id: u64,
    #[serde(flatten)]
    body: T,
}

#[derive(Deserialize)]
struct Response {
    id: u64,
    ok: bool,
    result: Option<Box<RawValue>>,
    error: Option<String>,
}

/// A candidate, as the engine reports it (same JSON as the C API).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Candidate {
    pub word: String,
    pub kind: String,
    pub replace: String,
    pub replace_len_utf16: usize,
    pub append: bool,
    pub prob: Option<f64>,
    pub roman: Option<String>,
    pub why: Option<String>,
    /// Auto mode: which reading produced it ("khmerlish", "english", "nida", "raw"); absent in the other modes
    pub source: Option<String>,
    pub gain: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KeyAction {
    PassThrough,
    Insert(Candidate),
}

#[derive(Deserialize)]
struct RawKeyAction {
    action: String,
    candidate: Option<Candidate>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PingInfo {
    pub v: u32,
    pub vocab: u64,
}

// ------------------------------------------------------------------------------------------------ framing

pub fn write_frame<W: Write>(w: &mut W, payload: &[u8]) -> io::Result<()> {
    if payload.len() > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "frame too large"));
    }
    // one write, so a frame is never interleaved or half-sent by a timeout between the header and the body
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(payload);
    w.write_all(&buf)?;
    w.flush()
}

/// Read one frame. `Ok(None)` means the peer closed the stream cleanly before a frame started.
pub fn read_frame<R: Read>(r: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut head = [0u8; 4];
    let mut got = 0;
    while got < 4 {
        match r.read(&mut head[got..]) {
            Ok(0) if got == 0 => return Ok(None),
            Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "stream ended inside a frame header")),
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    let len = u32::from_le_bytes(head) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    Ok(Some(body))
}

// ------------------------------------------------------------------------------------------------ client

/// A byte stream with a deadline for everything that happens until it is moved.
pub trait TimedStream: Read + Write {
    fn set_deadline(&mut self, deadline: Instant) -> io::Result<()>;
}

#[derive(Debug)]
pub enum ClientError {
    /// the engine did not answer in time
    Timeout,
    /// the engine is not running / the pipe is gone
    Unavailable(String),
    /// the engine answered something we cannot use (bad JSON, wrong id, wrong protocol version)
    Protocol(String),
    /// the engine understood the request and refused it
    Engine(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Timeout => write!(f, "engine timed out"),
            ClientError::Unavailable(m) => write!(f, "engine unavailable: {m}"),
            ClientError::Protocol(m) => write!(f, "protocol error: {m}"),
            ClientError::Engine(m) => write!(f, "engine error: {m}"),
        }
    }
}
impl std::error::Error for ClientError {}

fn io_to_client(e: io::Error) -> ClientError {
    match e.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => ClientError::Timeout,
        io::ErrorKind::InvalidData => ClientError::Protocol(e.to_string()),
        _ => ClientError::Unavailable(e.to_string()),
    }
}

pub struct Client<S: TimedStream> {
    stream: S,
    next_id: u64,
}

impl<S: TimedStream> Client<S> {
    pub fn new(stream: S) -> Client<S> {
        Client { stream, next_id: 1 }
    }

    /// One request and its response, within `timeout`. After any error the connection must be dropped:
    /// a late answer to an abandoned request would otherwise be read as the answer to the next one.
    pub fn call(&mut self, req: &Request, timeout: Duration) -> Result<Box<RawValue>, ClientError> {
        let id = self.next_id;
        self.next_id += 1;
        self.stream.set_deadline(Instant::now() + timeout).map_err(io_to_client)?;
        let payload = serde_json::to_vec(&Envelope { v: PROTOCOL_VERSION, id, body: req.clone() })
            .map_err(|e| ClientError::Protocol(e.to_string()))?;
        write_frame(&mut self.stream, &payload).map_err(io_to_client)?;
        let frame = read_frame(&mut self.stream)
            .map_err(io_to_client)?
            .ok_or_else(|| ClientError::Unavailable("engine closed the connection".into()))?;
        let resp: Response = serde_json::from_slice(&frame).map_err(|e| ClientError::Protocol(e.to_string()))?;
        if resp.id != id {
            return Err(ClientError::Protocol(format!("answer for request {} while waiting for {id}", resp.id)));
        }
        if !resp.ok {
            return Err(ClientError::Engine(resp.error.unwrap_or_default()));
        }
        resp.result.ok_or_else(|| ClientError::Protocol("ok response without a result".into()))
    }

    /// Check the engine speaks our protocol version.
    pub fn ping(&mut self, timeout: Duration) -> Result<PingInfo, ClientError> {
        let r = self.call(&Request::Ping, timeout)?;
        let info: PingInfo = serde_json::from_str(r.get()).map_err(|e| ClientError::Protocol(e.to_string()))?;
        if info.v != PROTOCOL_VERSION {
            return Err(ClientError::Protocol(format!(
                "engine speaks protocol {}, this client {PROTOCOL_VERSION}",
                info.v
            )));
        }
        Ok(info)
    }

    pub fn suggest(
        &mut self,
        text: &str,
        mode: u8,
        just_accepted: Option<&str>,
        timeout: Duration,
    ) -> Result<Vec<Candidate>, ClientError> {
        let r = self.call(
            &Request::Suggest { text: text.to_string(), mode, just_accepted: just_accepted.map(str::to_string) },
            timeout,
        )?;
        serde_json::from_str(r.get()).map_err(|e| ClientError::Protocol(e.to_string()))
    }

    pub fn key_action(
        &mut self,
        text: &str,
        mode: u8,
        key: u8,
        just_accepted: Option<&str>,
        timeout: Duration,
    ) -> Result<KeyAction, ClientError> {
        let r = self.call(
            &Request::KeyAction { text: text.to_string(), mode, key, just_accepted: just_accepted.map(str::to_string) },
            timeout,
        )?;
        let raw: RawKeyAction = serde_json::from_str(r.get()).map_err(|e| ClientError::Protocol(e.to_string()))?;
        Ok(match (raw.action.as_str(), raw.candidate) {
            ("insert", Some(c)) => KeyAction::Insert(c),
            _ => KeyAction::PassThrough,
        })
    }
}

// ------------------------------------------------------------------------------------------------ breaker

/// Stops calling a failing engine for a while, so a dead or stuck engine costs the host app one timeout,
/// not one per key press. Time is passed in, so the logic is testable.
#[derive(Debug, Clone)]
pub struct Breaker {
    threshold: u32,
    cooldown: Duration,
    failures: u32,
    open_until: Option<Instant>,
}

impl Breaker {
    pub fn new(threshold: u32, cooldown: Duration) -> Breaker {
        Breaker { threshold, cooldown, failures: 0, open_until: None }
    }
    /// May we call the engine now?
    pub fn allow(&mut self, now: Instant) -> bool {
        match self.open_until {
            Some(t) if now < t => false,
            Some(_) => {
                // cooldown over: let one attempt through (half-open); one more failure opens it again at once
                self.open_until = None;
                self.failures = self.threshold.saturating_sub(1);
                true
            }
            None => true,
        }
    }
    pub fn success(&mut self) {
        self.failures = 0;
        self.open_until = None;
    }
    pub fn failure(&mut self, now: Instant) {
        self.failures += 1;
        if self.failures >= self.threshold {
            self.open_until = Some(now + self.cooldown);
        }
    }
}

// ------------------------------------------------------------------------------------------------ server

/// The engine side. `suggest` and `key_action` return JSON text exactly as the C API does.
pub trait Handler {
    fn vocab(&self) -> u64;
    fn suggest(&self, text: &str, mode: u8, just_accepted: Option<&str>) -> Result<String, String>;
    fn key_action(&self, text: &str, mode: u8, key: u8, just_accepted: Option<&str>) -> Result<String, String>;
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).expect("a string serializes")
}

fn respond(id: u64, r: Result<String, String>) -> String {
    match r {
        Ok(json) => format!("{{\"id\":{id},\"ok\":true,\"result\":{json}}}"),
        Err(e) => format!("{{\"id\":{id},\"ok\":false,\"error\":{}}}", json_str(&e)),
    }
}

/// Serve one client until it disconnects. A panic in the handler becomes an error response, not a dead server.
pub fn serve_connection<S: Read + Write, H: Handler>(mut stream: S, handler: &H) -> io::Result<()> {
    while let Some(frame) = read_frame(&mut stream)? {
        let reply = match serde_json::from_slice::<Envelope<Request>>(&frame) {
            Err(e) => respond(0, Err(format!("bad request: {e}"))),
            Ok(env) if env.v != PROTOCOL_VERSION => {
                respond(env.id, Err(format!("protocol version {} not supported", env.v)))
            }
            Ok(env) => {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &env.body {
                    Request::Ping => Ok(format!("{{\"v\":{PROTOCOL_VERSION},\"vocab\":{}}}", handler.vocab())),
                    Request::Suggest { text, mode, just_accepted } => {
                        handler.suggest(text, *mode, just_accepted.as_deref())
                    }
                    Request::KeyAction { text, mode, key, just_accepted } => {
                        handler.key_action(text, *mode, *key, just_accepted.as_deref())
                    }
                }))
                .unwrap_or_else(|_| Err("internal error".to_string()));
                respond(env.id, result)
            }
        };
        write_frame(&mut stream, reply.as_bytes())?;
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------ unix streams (tests, dev)

#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::net::UnixStream;

    /// A Unix socket with the same deadline behavior the Windows pipe has; used by tests and for development.
    pub struct UnixTimed(pub UnixStream);

    impl Read for UnixTimed {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.read(buf)
        }
    }
    impl Write for UnixTimed {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.0.flush()
        }
    }
    impl TimedStream for UnixTimed {
        fn set_deadline(&mut self, deadline: Instant) -> io::Result<()> {
            // the socket timeout applies per operation; a request is one write and a short read, so the remaining
            // time at the start is a faithful bound for both
            let left = deadline.saturating_duration_since(Instant::now()).max(Duration::from_millis(1));
            self.0.set_read_timeout(Some(left))?;
            self.0.set_write_timeout(Some(left))
        }
    }
}
#[cfg(unix)]
pub use unix::UnixTimed;
