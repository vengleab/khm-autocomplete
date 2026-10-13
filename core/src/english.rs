//! English input: keys and a tiny stemmer for the English -> Khmer glossary (data/english.json).
//! Ports of `enKey()` and `enStems()` in the reference.

/// Lowercase, hyphens become spaces, only a-z, apostrophe and spaces survive, runs of spaces collapse.
pub fn en_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    for c in s.to_lowercase().chars() {
        let c = if c == '-' { ' ' } else { c };
        if c == ' ' {
            pending_space = !out.is_empty();
        } else if c.is_ascii_lowercase() || c == '\'' {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(c);
        }
    }
    out
}

/// Plausible base forms of an inflected English word (eating -> eat, cities -> city, stopped -> stop).
pub fn stems(k: &str) -> Vec<String> {
    let mut o: Vec<String> = Vec::new();
    let mut add = |s: &str| {
        if s.len() >= 2 && s != k && !o.iter().any(|x| x == s) {
            o.push(s.to_string());
        }
    };
    let cut = |n: usize| &k[..k.len() - n];
    let doubled = |b: &str| {
        let t = b.as_bytes();
        t.len() > 2 && t[t.len() - 1] == t[t.len() - 2]
    };
    if k.ends_with("ies") && k.len() > 4 {
        add(&format!("{}y", cut(3)));
    }
    if k.ends_with("es") {
        add(cut(2));
    }
    if k.ends_with('s') {
        add(cut(1));
    }
    if k.ends_with("ing") && k.len() > 4 {
        let b = cut(3);
        add(b);
        add(&format!("{b}e"));
        if doubled(b) {
            add(&b[..b.len() - 1]);
        }
    }
    if k.ends_with("ed") && k.len() > 3 {
        let b = cut(2);
        add(b);
        add(&format!("{b}e"));
        if doubled(b) {
            add(&b[..b.len() - 1]);
        }
        if k.ends_with("ied") && k.len() > 4 {
            add(&format!("{}y", cut(3)));
        }
    }
    if k.ends_with("ly") && k.len() > 4 {
        add(cut(2));
    }
    o
}
