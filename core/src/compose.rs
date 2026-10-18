//! Latin syllable -> Khmer letters, working the charts backwards. The vowel sound decides the series,
//! the series decides the letter (k + "ea" can only be គ). Never emits a wrong-series spelling.
use crate::tables::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct ComposeCand {
    pub word: String,
    pub why: String,
    pub score: f64,
}

pub fn compose(token: &str, prior: &HashMap<char, f64>) -> Vec<ComposeCand> {
    compose_with(Tables::builtin(), token, prior)
}

/// `compose` with the charts of `tb` (a mapping file's tables instead of the built-in ones).
pub fn compose_with(tb: &Tables, token: &str, prior: &HashMap<char, f64>) -> Vec<ComposeCand> {
    let t: String = token.to_lowercase().chars().filter(|c| c.is_ascii_lowercase()).collect();
    if t.is_empty() {
        return vec![];
    }
    let mut found: Vec<ComposeCand> = Vec::new();
    let mut pos: HashMap<String, usize> = HashMap::new();

    let mut onsets: Vec<(&str, Vec<char>)> = vec![("", vec!['អ'])];
    for (o, letters) in tb.onset_lat() {
        if t.starts_with(o.as_str()) {
            onsets.push((o, letters.clone()));
        }
    }
    for (o, letters) in onsets {
        let rest = &t[o.len()..];
        let mut finals: Vec<(&str, Vec<String>)> = vec![("", vec![String::new()])];
        for (lat, ls) in tb.final_lat.iter() {
            if rest.len() > lat.len() && rest.ends_with(lat.as_str()) {
                finals.push((lat.as_str(), ls.iter().map(|c| c.to_string()).collect()));
            }
        }
        for (flat, fl) in &finals {
            let v = &rest[..rest.len() - flat.len()];
            if v.is_empty() {
                continue;
            }
            for e in tb.vow.iter() {
                for r in [Series::A, Series::O] {
                    if !e.readings(r).iter().any(|x| x == v) {
                        continue;
                    }
                    let mut ls: Vec<char> = letters.iter().copied().filter(|l| tb.series_of(*l) == Some(r)).collect();
                    let mut mark: Option<char> = None;
                    if ls.is_empty() {
                        ls = letters
                            .iter()
                            .copied()
                            .filter(|l| if r == Series::A { tb.to_a(*l) } else { tb.to_o(*l) })
                            .collect();
                        mark = Some(if r == Series::A { '\u{17C9}' } else { '\u{17CA}' });
                    }
                    for l in &ls {
                        for f in fl {
                            if mark.is_some() && e.s.is_empty() {
                                continue; // a switching mark needs a vowel sign to sit with
                            }
                            let mut head = l.to_string();
                            if let Some(m) = mark {
                                head.push(m);
                            }
                            head.push_str(&e.s);
                            let w = format!("{head}{f}");
                            let ser_word = if tb.series_of(*l) == Some(Series::A) { "first" } else { "second" };
                            let why = format!(
                                "{}: {}-series {}{}, reads \"{}\"",
                                head,
                                ser_word,
                                l,
                                match mark {
                                    Some(m) => format!(" switched by {m}"),
                                    None => String::new(),
                                },
                                v
                            );
                            let score = prior.get(l).copied().unwrap_or(0.0)
                                + if mark.is_some() { -0.5 } else { 0.0 }
                                + if flat.is_empty() { 0.05 } else { 0.0 };
                            match pos.get(&w) {
                                Some(&i) => {
                                    if found[i].score < score {
                                        found[i] = ComposeCand { word: w, why, score };
                                    }
                                }
                                None => {
                                    pos.insert(w.clone(), found.len());
                                    found.push(ComposeCand { word: w, why, score });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    found.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    found.truncate(4);
    found
}
