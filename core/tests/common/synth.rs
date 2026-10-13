#![allow(dead_code)]
//! Deterministic synthetic corpus of any size, for latency / memory tests (not for parity).
use khcore::norm::normalize;
use khcore::romanize::romanize;
use khcore::tables::{CONS, VOWSIGNS};
use std::collections::HashSet;

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    pub fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// (data.json text, lexicon.json text, words)
pub fn make(n_words: usize, seed: u64) -> (String, String, Vec<String>) {
    let mut r = Rng(seed);
    let letters: Vec<char> = CONS.iter().map(|c| c.0).collect();
    let vs: Vec<char> = VOWSIGNS.chars().collect();
    let mut seen = HashSet::new();
    let mut words: Vec<String> = Vec::with_capacity(n_words);
    while words.len() < n_words {
        let mut w = String::new();
        let k = 1 + if r.f() < 0.4 {
            0
        } else if r.f() < 0.6 {
            1
        } else if r.f() < 0.7 {
            2
        } else {
            3
        };
        for _ in 0..k {
            w.push(letters[r.below(letters.len())]);
            if r.f() < 0.2 {
                w.push('\u{17D2}');
                w.push(letters[r.below(letters.len() - 1)]);
            }
            if r.f() < 0.6 {
                w.push(vs[r.below(vs.len())]);
            }
            if r.f() < 0.12 {
                w.push(['ំ', 'ះ', '់', '៉'][r.below(4)]);
            }
        }
        let w = normalize(&w);
        if seen.insert(w.clone()) {
            words.push(w);
        }
    }
    let freq: Vec<u64> = (0..n_words).map(|i| (2_000_000.0 / ((i + 1) as f64).powf(1.05)) as u64 + 1).collect();
    let mut bi = serde_json::Map::new();
    let row = |r: &mut Rng, m: usize| -> serde_json::Value {
        let mut v = Vec::new();
        for _ in 0..m {
            v.push((r.f().powi(2) * 20000f64.min(n_words as f64)) as u64);
            v.push(1 + r.below(30) as u64);
        }
        v.into()
    };
    bi.insert("^".into(), row(&mut r, 30));
    for i in 0..10_000.min(n_words) {
        let m = 5 + r.below(15);
        bi.insert(i.to_string(), row(&mut r, m));
    }
    let data = serde_json::json!({
        "meta": {"sentences": n_words / 10, "tokens": freq.iter().sum::<u64>(), "vocab": n_words, "source": "synthetic"},
        "words": words, "freq": freq, "bi": bi,
    });
    let mut lex = serde_json::Map::new();
    let subs =
        [("kh", "k"), ("ou", "u"), ("ea", "ia"), ("nh", "ny"), ("ei", "ey"), ("oe", "u"), ("ch", "j"), ("ph", "p")];
    for i in 0..n_words / 3 {
        let w = &words[r.below(n_words)];
        let base = romanize(w);
        let mut sp = vec![base.clone()];
        for _ in 0..r.below(3) {
            let (a, b) = subs[r.below(subs.len())];
            sp.push(base.replacen(a, b, 1));
        }
        lex.insert(w.clone(), sp.join(" ").into());
        let _ = i;
    }
    (data.to_string(), serde_json::Value::Object(lex).to_string(), words)
}

/// Synthetic English glossary (pseudo-words, a few phrases) for `words`, about two glosses for every third word.
pub fn make_english(words: &[String], seed: u64) -> String {
    let mut r = Rng(seed);
    let on =
        ["b", "c", "d", "f", "g", "h", "k", "l", "m", "n", "p", "r", "s", "t", "w", "st", "tr", "ch", "sh", "th", ""];
    let vw = ["a", "e", "i", "o", "u", "ai", "ea", "oo", "ou"];
    let co = ["", "n", "t", "r", "s", "l", "m", "d", "ck", "ng"];
    let word = |r: &mut Rng| {
        let mut w = String::new();
        for _ in 0..1 + r.below(3) {
            w.push_str(on[r.below(on.len())]);
            w.push_str(vw[r.below(vw.len())]);
            w.push_str(co[r.below(co.len())]);
        }
        w
    };
    let mut m = serde_json::Map::new();
    for i in (0..words.len()).step_by(3) {
        let mut gl = Vec::new();
        for _ in 0..1 + r.below(3) {
            let mut g = word(&mut r);
            if r.f() < 0.25 {
                g.push(' ');
                g.push_str(&word(&mut r));
            }
            gl.push(g);
        }
        m.insert(words[i].clone(), gl.join(", ").into());
    }
    serde_json::Value::Object(m).to_string()
}
