//! khengine [--data khdata.bin] [--socket PATH]
//!
//! Windows: serves the per-user named pipe `\\.\pipe\KhmerAutocomplete.<SID>` (one thread per client).
//! Other systems (development and tests only): serves a Unix socket given with `--socket`.
//! No network, no logging of typed text; errors go to stderr and contain no user text.
#![cfg_attr(windows, windows_subsystem = "windows")]

use khcore::Engine;
use khengine::EngineHandler;
use std::sync::Arc;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).cloned()
}

fn default_data() -> String {
    // next to the executable: <install dir>\khdata.bin
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("khdata.bin")))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "khdata.bin".into())
}

fn main() {
    let data = arg("--data").unwrap_or_else(default_data);
    let engine = match Engine::load_compiled(&data) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("khengine: cannot load {data}: {e}");
            std::process::exit(1);
        }
    };
    let handler = Arc::new(EngineHandler(engine));
    serve(handler);
}

#[cfg(windows)]
fn serve(handler: Arc<EngineHandler>) {
    use windows::core::w;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;

    // one engine per user session: a second copy exits at once
    let _single = unsafe { CreateMutexW(None, true, w!("Local\\KhmerAutocompleteEngine")) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return;
    }
    let mut first = true;
    loop {
        match khipc::winpipe::accept(first) {
            Ok(pipe) => {
                first = false;
                let h = handler.clone();
                std::thread::spawn(move || {
                    let _ = khipc::serve_connection(pipe, &*h);
                });
            }
            // The very first instance failed: another process already owns our pipe name. Do not serve a name we do not
            // own (the text service refuses servers of another user anyway); exit.
            Err(_) if first => return,
            // later failures are usually transient (out of handles): never spin
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(200)),
        }
    }
}

#[cfg(not(windows))]
fn serve(handler: Arc<EngineHandler>) {
    use std::os::unix::net::UnixListener;
    let Some(path) = arg("--socket") else {
        eprintln!("khengine: on this system pass --socket PATH (development only; the product runs on Windows)");
        std::process::exit(2);
    };
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).unwrap_or_else(|e| {
        eprintln!("khengine: cannot bind {path}: {e}");
        std::process::exit(1);
    });
    for stream in listener.incoming().flatten() {
        let h = handler.clone();
        std::thread::spawn(move || {
            let _ = khipc::serve_connection(stream, &*h);
        });
    }
}
