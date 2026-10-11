//! Khmer clusters: a base character U+1780..=U+17B3 followed by marks. Same as the reference regex
//! `([ក-ឳ])((?:្[ក-អ]|[឴-៓៝])*)`. This is deliberately not UAX #29: coeng + consonant is one unit.

pub const COENG: char = '\u{17D2}';

pub fn is_base(c: char) -> bool {
    ('\u{1780}'..='\u{17B3}').contains(&c)
}
pub fn is_cons(c: char) -> bool {
    ('\u{1780}'..='\u{17A2}').contains(&c)
}
pub fn is_mark(c: char) -> bool {
    ('\u{17B4}'..='\u{17D3}').contains(&c) || c == '\u{17DD}'
}

/// Byte length of the run of marks at the start of `s` (what follows a base character).
pub fn marks_len(s: &str) -> usize {
    let mut n = 0;
    let mut it = s.chars().peekable();
    while let Some(&c) = it.peek() {
        if c == COENG {
            it.next();
            n += c.len_utf8();
            if let Some(&d) = it.peek() {
                if is_cons(d) {
                    it.next();
                    n += d.len_utf8();
                }
            }
        } else if is_mark(c) {
            it.next();
            n += c.len_utf8();
        } else {
            break;
        }
    }
    n
}

/// Byte offsets of cluster boundaries: `[0, ..., s.len()]`.
pub fn cluster_offsets(s: &str) -> Vec<usize> {
    let mut out = vec![0];
    let mut i = 0;
    while i < s.len() {
        let c = s[i..].chars().next().unwrap();
        let mut end = i + c.len_utf8();
        if is_base(c) {
            end += marks_len(&s[end..]);
        }
        out.push(end);
        i = end;
    }
    out
}

/// Split text into clusters. A character that is not a Khmer base is its own cluster.
pub fn clusters(s: &str) -> Vec<&str> {
    let off = cluster_offsets(s);
    off.windows(2).map(|w| &s[w[0]..w[1]]).collect()
}
