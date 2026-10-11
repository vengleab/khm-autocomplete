//! Latin sound key: "knyom", "knhom" and "khnhom" all reduce to the same key.
//! Also the two edit-distance helpers the reference uses.
use unicode_normalization::UnicodeNormalization;

/// JS `s.replace(/a|b|c/g, rep)`: leftmost match, alternatives tried in order.
fn replace_alts(s: &[u8], alts: &[&str], rep: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    'outer: while i < s.len() {
        for a in alts {
            if s[i..].starts_with(a.as_bytes()) {
                out.push(rep);
                i += a.len();
                continue 'outer;
            }
        }
        out.push(s[i]);
        i += 1;
    }
    out
}

fn replace_str(s: &[u8], alts: &[&str], rep: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    'outer: while i < s.len() {
        for a in alts {
            if s[i..].starts_with(a.as_bytes()) {
                out.extend_from_slice(rep.as_bytes());
                i += a.len();
                continue 'outer;
            }
        }
        out.push(s[i]);
        i += 1;
    }
    out
}

fn is_vowel(b: u8) -> bool {
    b"aeiouAEIOWu".contains(&b)
}

pub fn rom_key(s: &str) -> String {
    // 1. NFD, drop combining marks, map a few IPA letters, then lowercase and keep a-z only.
    //    (the reference maps before lowercasing, so uppercase IPA letters are dropped, not mapped)
    let mut t = String::with_capacity(s.len());
    for c in s.nfd() {
        if ('\u{0300}'..='\u{036F}').contains(&c) {
            continue;
        }
        t.push(match c {
            'ə' | 'ɛ' => 'e',
            'ɨ' => 'u',
            'ɔ' => 'o',
            'ɑ' => 'a',
            c => c,
        });
    }
    let mut b: Vec<u8> = t.to_lowercase().bytes().filter(|c| c.is_ascii_lowercase()).collect();
    b = replace_alts(&b, &["ng"], b'G');
    b = replace_alts(&b, &["nh", "ny", "gn"], b'N');
    b = replace_alts(&b, &["chh", "ch", "j"], b'c');
    b = replace_alts(&b, &["kh", "g", "q"], b'k');
    b = replace_alts(&b, &["ph"], b'p');
    b = replace_alts(&b, &["th"], b't');
    b = replace_alts(&b, &["w"], b'v');
    b = replace_alts(&b, &["x"], b's');
    b = replace_alts(&b, &["oeu", "aeu", "eua", "eu", "oe", "ue", "ur", "er"], b'E');
    b = replace_alts(&b, &["uo", "ua"], b'W');
    b = replace_alts(&b, &["ea", "ia", "ie"], b'A');
    b = replace_alts(&b, &["ou", "oo", "uu"], b'u');
    b = replace_alts(&b, &["ei", "ey", "ee", "ii"], b'i');
    b = replace_str(&b, &["ay", "aa"], "ai");
    b = replace_alts(&b, &["ao", "au", "ov", "av"], b'O');
    b = replace_alts(&b, &["ae"], b'e');
    // 6. r and h not followed by a vowel are silent (lookahead is evaluated on the string as it is now)
    for letter in *b"rh" {
        let mut out = Vec::with_capacity(b.len());
        for (i, &c) in b.iter().enumerate() {
            if c == letter && !b.get(i + 1).is_some_and(|&n| is_vowel(n)) {
                continue;
            }
            out.push(c);
        }
        b = out;
    }
    if b.last() == Some(&b'y') {
        *b.last_mut().unwrap() = b'i';
    }
    // 7. collapse repeated letters
    b.dedup();
    String::from_utf8(b).unwrap()
}

/// Levenshtein distance on bytes (Latin keys).
pub fn lev_s(a: &[u8], b: &[u8]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1]));
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Levenshtein distance on clusters, with the reference's early exit: lengths more than 1 apart give 2.
pub fn lev<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    if a.len().abs_diff(b.len()) > 1 {
        return 2;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1]));
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// `lev(a, b) <= 1`, without building the table: the typo scans only need this decision.
pub fn within_one<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let i = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    if i == a.len().min(b.len()) {
        return true;
    }
    match a.len().cmp(&b.len()) {
        std::cmp::Ordering::Equal => a[i + 1..] == b[i + 1..],
        std::cmp::Ordering::Greater => a[i + 1..] == b[i..],
        std::cmp::Ordering::Less => a[i..] == b[i + 1..],
    }
}
