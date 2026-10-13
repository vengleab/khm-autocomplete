#![allow(dead_code)]
pub mod synth;
use khcore::Engine;
use serde_json::Value;
use std::path::PathBuf;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
pub fn golden(name: &str) -> Value {
    let p = root().join("tests/golden").join(name);
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}
pub fn seed_engine() -> Engine {
    let r = root();
    let p = |f: &str| r.join(f).to_str().unwrap().to_string();
    Engine::load_with_english(&p("data/data.json"), &p("data/lexicon.json"), &p("data/english.json")).unwrap()
}
pub fn synth_engine() -> Engine {
    let r = root().join("tests/golden");
    let p = |f: &str| r.join(f).to_str().unwrap().to_string();
    Engine::load_with_english(&p("synth_data.json"), &p("synth_lexicon.json"), &p("synth_english.json")).unwrap()
}

/// Collects mismatches so one run shows many of them, then fails once.
#[derive(Default)]
pub struct Fails {
    pub n: usize,
    pub checked: usize,
    pub msgs: Vec<String>,
}
impl Fails {
    pub fn check(&mut self, ok: bool, msg: impl FnOnce() -> String) {
        self.checked += 1;
        if !ok {
            self.n += 1;
            if self.msgs.len() < 15 {
                self.msgs.push(msg());
            }
        }
    }
    pub fn finish(self, what: &str) {
        println!("{what}: {} checks, {} mismatches", self.checked, self.n);
        if self.n > 0 {
            panic!("{what}: {} mismatches, first {}:\n{}", self.n, self.msgs.len(), self.msgs.join("\n"));
        }
    }
}
pub fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}
pub fn close(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1e-300)
}
