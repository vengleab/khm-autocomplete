//! khcore: the portable Khmer autocomplete engine.
//!
//! Every function here is a port of the reference implementation in `web/template.html`.
//! Where this code and the JS disagree, the JS is right (see `tests/golden`).
//! No platform code, no network, no logging of typed text.

pub mod api_json;
pub mod cluster;
pub mod compose;
pub mod engine;
pub mod english;
pub mod fdlibm;
pub mod key;
/// The Khmer (NiDA) keyboard layout; lives in its own dependency-free crate (`khlayout`).
pub use khlayout as layout;
pub mod manifest;
pub mod model;
#[cfg(feature = "neural")]
pub mod neural;
pub mod norm;
pub mod romanize;
pub mod romkey;
pub mod scorer;
pub mod suggest;
pub mod tables;
pub mod userdict;

pub use engine::{Candidate, Engine, Kind, Mode};
pub use key::{Key, KeyAction};
pub use model::Model;
pub use scorer::{NgramScorer, Scorer};

/// JS `\s`: used when the reference splits typed text into runs.
pub fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
    )
}

/// JS `s.split(/\s+/)`: runs of whitespace collapse, leading/trailing whitespace yields empty ends.
pub fn split_js_space(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut in_ws) = (0usize, false);
    for (i, c) in s.char_indices() {
        if is_js_space(c) {
            if !in_ws {
                out.push(&s[start..i]);
                in_ws = true;
            }
            start = i + c.len_utf8();
        } else if in_ws {
            in_ws = false;
            start = i;
        }
    }
    out.push(&s[start..]);
    out
}
