//! The engine behind the real protocol: what the TSF DLL would get, compared with calling the engine directly.
#![cfg(unix)]
use khcore::{Engine, Key, KeyAction, Mode};
use khengine::EngineHandler;
use khipc::{Client, ClientError, UnixTimed};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

const T: Duration = Duration::from_millis(1000);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn engine() -> Engine {
    let p = |f: &str| root().join(f).to_str().unwrap().to_string();
    Engine::load_with_english(&p("data/data.json"), &p("data/lexicon.json"), &p("data/english.json")).unwrap()
}
fn client() -> Client<UnixTimed> {
    let (a, b) = UnixStream::pair().unwrap();
    std::thread::spawn(move || {
        let _ = khipc::serve_connection(b, &EngineHandler(engine()));
    });
    Client::new(UnixTimed(a))
}

#[test]
fn suggestions_through_the_pipe_equal_direct_calls() {
    let direct = engine();
    let mut c = client();
    assert_eq!(c.ping(T).unwrap().vocab, 277);
    let cases: [(&str, Mode, u8, Option<&str>); 7] = [
        ("ខ្ញុំ ស្រ", Mode::Khmer, 0, None),
        ("ខ្ញុំ ចូលចិត្ត ", Mode::Khmer, 0, None),
        ("ខ្ញុំ ចូលចិត្ត ញ៉ាំ", Mode::Khmer, 0, Some("ញ៉ាំ")),
        ("ស្រលាង", Mode::Khmer, 0, None),
        ("knyom", Mode::Latin, 1, None),
        ("ខ្ញុំ sok", Mode::Latin, 1, None),
        ("eating", Mode::English, 2, None),
    ];
    for (text, mode, code, just) in cases {
        let got = c.suggest(text, code, just, T).unwrap();
        let want = direct.suggest(text, mode, just);
        assert_eq!(got.len(), want.len(), "{text:?}");
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(
                (g.word.as_str(), g.kind.as_str(), g.replace.as_str(), g.replace_len_utf16, g.append),
                (
                    w.word.as_str(),
                    format!("{:?}", w.kind).to_lowercase().as_str(),
                    w.replace.as_str(),
                    w.replace_len_utf16,
                    w.append
                ),
                "{text:?}"
            );
            assert_eq!(g.roman, w.roman);
        }
    }
}

#[test]
fn key_actions_through_the_pipe() {
    let direct = engine();
    let mut c = client();
    for (text, mode, mcode, key, kcode) in [
        ("knyom", Mode::Latin, 1u8, Key::Space, 0u8),
        ("", Mode::Latin, 1, Key::Space, 0),
        ("water", Mode::English, 2, Key::Space, 0),
        ("wat", Mode::English, 2, Key::Space, 0),
        ("wat", Mode::English, 2, Key::Tab, 2),
        ("ខ្ញុំ", Mode::Khmer, 0, Key::Space, 0),
        ("ស្រ", Mode::Khmer, 0, Key::Tab, 2),
        ("knyom", Mode::Latin, 1, Key::Enter, 1),
    ] {
        let got = c.key_action(text, mcode, kcode, None, T).unwrap();
        match (got, direct.key_action(text, mode, key, None)) {
            (khipc::KeyAction::PassThrough, KeyAction::PassThrough) => {}
            (khipc::KeyAction::Insert(g), KeyAction::Insert(w)) => {
                assert_eq!((g.word, g.replace_len_utf16), (w.word, w.replace_len_utf16), "{text:?}")
            }
            (g, w) => panic!("{text:?} {mode:?} {key:?}: pipe says {g:?}, direct says {w:?}"),
        }
    }
}

#[test]
fn bad_modes_and_keys_are_refused_not_fatal() {
    let mut c = client();
    assert!(matches!(c.suggest("x", 9, None, T).unwrap_err(), ClientError::Engine(_)));
    assert!(matches!(c.key_action("x", 0, 9, None, T).unwrap_err(), ClientError::Engine(_)));
    assert_eq!(c.suggest("x", 0, None, T).map(|_| ()).is_ok(), true, "connection still works");
}

#[test]
fn the_khengine_binary_serves_a_socket() {
    // the real executable, started the way the product starts it, but on a Unix socket
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("khengine-bin");
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("khdata.bin");
    std::fs::write(&bin, engine().model().to_bytes()).unwrap();
    let sock = dir.join("k.sock");
    let _ = std::fs::remove_file(&sock);
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_khengine"))
        .args(["--data", bin.to_str().unwrap(), "--socket", sock.to_str().unwrap()])
        .spawn()
        .unwrap();
    let mut stream = None;
    for _ in 0..100 {
        if let Ok(s) = UnixStream::connect(&sock) {
            stream = Some(s);
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut c = Client::new(UnixTimed(stream.expect("engine did not start")));
    let r = c.suggest("knyom", 1, None, T);
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(r.unwrap()[0].word, "ខ្ញុំ");
}

#[test]
fn the_khengine_binary_refuses_a_missing_data_file() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_khengine"))
        .args(["--data", "/no/such/khdata.bin", "--socket", "/tmp/never"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("cannot load"));
}
