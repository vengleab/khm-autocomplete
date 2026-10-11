//! Khmer normalization: NFC, drop invisible characters, then put the marks of each cluster in one order.
use crate::cluster::{is_base, marks_len, COENG};
use unicode_normalization::UnicodeNormalization;

fn is_invisible(c: char) -> bool {
    matches!(c, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}')
}

fn rank(unit: &str) -> u8 {
    let c = unit.chars().next().unwrap();
    if c == COENG {
        1
    } else if c == '\u{17C9}' || c == '\u{17CA}' {
        2
    } else if ('\u{17B4}'..='\u{17C5}').contains(&c) {
        3
    } else {
        4
    }
}

/// Reorder the marks of one cluster. Units are (coeng + next char) pairs or single marks; the sort is stable.
fn fix_marks(marks: &str, out: &mut String) {
    let mut units: Vec<(u8, &str)> = Vec::new();
    let mut it = marks.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let mut end = i + c.len_utf8();
        if c == COENG {
            if let Some(&(_, d)) = it.peek() {
                end += d.len_utf8();
                it.next();
            }
        }
        let u = &marks[i..end];
        units.push((rank(u), u));
    }
    units.sort_by_key(|u| u.0);
    for (_, u) in units {
        out.push_str(u);
    }
}

pub fn normalize(s: &str) -> String {
    let nfc: String = s.nfc().collect();
    let t: String = nfc.chars().filter(|c| !is_invisible(*c)).collect();
    let mut out = String::with_capacity(t.len());
    let mut i = 0;
    while i < t.len() {
        let c = t[i..].chars().next().unwrap();
        out.push(c);
        i += c.len_utf8();
        if is_base(c) {
            let ml = marks_len(&t[i..]);
            fix_marks(&t[i..i + ml], &mut out);
            i += ml;
        }
    }
    out
}
