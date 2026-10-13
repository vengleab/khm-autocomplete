//! The connection from this DLL to `khengine.exe`: connect lazily, never block the host app for long,
//! stop asking when the engine fails (circuit breaker), start the engine if it is not running.
use khipc::winpipe::{self, PipeStream};
use khipc::{Breaker, Candidate, Client, KeyAction};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

// Every call runs on the host app's UI thread, so each has a hard deadline. The engine answers in well under
// a millisecond when it is healthy; these only bound the damage when it is not.
const CONNECT: Duration = Duration::from_millis(20);
const PING: Duration = Duration::from_millis(30);
const CALL: Duration = Duration::from_millis(30);
/// after this many failures in a row, stop calling for COOLDOWN
const FAILURES: u32 = 2;
const COOLDOWN: Duration = Duration::from_secs(3);
/// do not start the engine more often than this
const SPAWN_EVERY: Duration = Duration::from_secs(10);

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;

pub struct EngineLink {
    client: Option<Client<PipeStream>>,
    breaker: Breaker,
    last_spawn: Option<Instant>,
    engine_exe: Option<PathBuf>,
}

impl EngineLink {
    /// `dll_dir`: the folder of khtsf.dll; khengine.exe is expected next to it.
    pub fn new(dll_dir: Option<PathBuf>) -> EngineLink {
        EngineLink {
            client: None,
            breaker: Breaker::new(FAILURES, COOLDOWN),
            last_spawn: None,
            engine_exe: dll_dir.map(|d| d.join("khengine.exe")),
        }
    }

    fn spawn_engine(&mut self) {
        let now = Instant::now();
        if self.last_spawn.is_some_and(|t| now.duration_since(t) < SPAWN_EVERY) {
            return;
        }
        self.last_spawn = Some(now);
        if let Some(exe) = self.engine_exe.as_ref().filter(|p| p.exists()) {
            // fire and forget: the engine takes a moment to load its data; the next attempt finds the pipe
            let _ = std::process::Command::new(exe).creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS).spawn();
        }
    }

    fn connect(&mut self) -> bool {
        if self.client.is_some() {
            return true;
        }
        match winpipe::connect(CONNECT) {
            Ok(stream) => {
                let mut c = Client::new(stream);
                // refuse an engine of another protocol version (e.g. half-finished update)
                if c.ping(PING).is_ok() {
                    self.client = Some(c);
                    return true;
                }
                false
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    self.spawn_engine();
                }
                false
            }
        }
    }

    /// Run one engine call. `None` means "no answer": the caller lets the key through / shows nothing.
    fn with<T>(&mut self, f: impl FnOnce(&mut Client<PipeStream>) -> Result<T, khipc::ClientError>) -> Option<T> {
        let now = Instant::now();
        if !self.breaker.allow(now) {
            return None;
        }
        if !self.connect() {
            self.breaker.failure(now);
            return None;
        }
        match f(self.client.as_mut().expect("connected")) {
            Ok(v) => {
                self.breaker.success();
                Some(v)
            }
            Err(_) => {
                // after any error the connection may be out of step with the engine: drop it
                self.client = None;
                self.breaker.failure(now);
                None
            }
        }
    }

    pub fn suggest(&mut self, text: &str, mode: u8, just_accepted: Option<&str>) -> Option<Vec<Candidate>> {
        self.with(|c| c.suggest(text, mode, just_accepted, CALL))
    }

    pub fn key_action(&mut self, text: &str, mode: u8, key: u8, just_accepted: Option<&str>) -> Option<KeyAction> {
        self.with(|c| c.key_action(text, mode, key, just_accepted, CALL))
    }
}
