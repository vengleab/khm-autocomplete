//! Memory check in a fresh process (section 2: < 50 MB without a neural model).
//!   cargo run --release -p khcore --example perf -- gen 100000 /tmp/khdata.bin
//!   cargo run --release -p khcore --example perf -- run /tmp/khdata.bin
#[path = "../tests/common/synth.rs"]
mod synth;
use khcore::{Engine, Mode, Model};

fn status(key: &str) -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with(key)).map(|l| l.trim().to_string()))
        .unwrap_or_else(|| format!("{key} n/a (no /proc)"))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("gen") => {
            let (data, lex, _) = synth::make(a[2].parse().unwrap(), 42);
            let b = Model::from_json(&data, &lex).unwrap().to_bytes();
            std::fs::write(&a[3], &b).unwrap();
            println!("wrote {} ({:.1} MB)", a[3], b.len() as f64 / 1e6);
        }
        Some("run") => {
            let t0 = std::time::Instant::now();
            let e = Engine::load_compiled(&a[2]).unwrap();
            println!("loaded {} words in {:?}", e.model().vocab(), t0.elapsed());
            println!("after load : {}", status("VmRSS"));
            let mut r = synth::Rng(5);
            for i in 0..3000 {
                let w = e.model().word(r.below(e.model().vocab().min(20_000)) as u32).to_string();
                let _ = e.suggest(&format!("{w} {}", &w[..3.min(w.len())]), Mode::Khmer, None);
                let _ = e.suggest(&format!("{w} knho{}", i % 7), Mode::Latin, None);
            }
            println!("after 6000 queries: {}", status("VmRSS"));
            println!("peak       : {}", status("VmHWM"));
        }
        _ => eprintln!("usage: perf gen N out.bin | perf run in.bin"),
    }
}
