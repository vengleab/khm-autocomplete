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
    Engine::load(r.join("data/data.json").to_str().unwrap(), r.join("data/lexicon.json").to_str().unwrap()).unwrap()
}
pub fn synth_engine() -> Engine {
    let r = root().join("tests/golden");
    Engine::load(r.join("synth_data.json").to_str().unwrap(), r.join("synth_lexicon.json").to_str().unwrap()).unwrap()
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
