//! Khmer word -> rough Latin spelling (heuristic; the hand-written lexicon takes priority).
//! Port of `romanize()`, including its quirks (see CLAUDE.md "Known JS quirks").
use crate::cluster::{clusters, COENG};
use crate::tables::*;

pub fn romanize(word: &str) -> String {
    let cs = clusters(word);
    let n = cs.len();
    let mut out = String::new();
    let mut open = false;
    for (i, c) in cs.iter().enumerate() {
        let mut it = c.chars();
        let base = it.next().unwrap();
        let rest: Vec<char> = it.collect();
        let is_last = i == n - 1;
        let mut subs: Vec<char> = Vec::new();
        let mut vowel: Option<char> = None;
        let mut marks: Vec<char> = Vec::new();
        let mut k = 0;
        while k < rest.len() {
            if rest[k] == COENG && k + 1 < rest.len() {
                subs.push(rest[k + 1]);
                k += 2;
                continue;
            }
            if VOWSIGNS.contains(rest[k]) {
                if vowel.is_none() {
                    vowel = Some(rest[k]);
                }
            } else {
                marks.push(rest[k]);
            }
            k += 1;
        }
        let has = |m: char| marks.contains(&m);
        if has('\u{17CD}') || has('\u{17D1}') {
            continue; // silent letter
        }
        if let Some(s) = indep(base) {
            out.push_str(s);
            for x in &subs {
                out.push_str(final_of(*x).unwrap_or(""));
            }
            open = false;
            continue;
        }
        let nik = has('\u{17C6}');
        let reah = has('\u{17C7}');
        let has_nuc = vowel.is_some() || nik || reah || has('\u{17D0}');
        if open && vowel.is_none() && (has('\u{17CB}') || (!has_nuc && (is_last || subs.is_empty()))) {
            out.push_str(final_of(base).unwrap_or(""));
            open = false;
            continue;
        }
        let mut ser = series_of(base).unwrap_or(Series::O);
        if has('\u{17C9}') {
            ser = Series::A;
        } else if has('\u{17CA}') {
            ser = Series::O;
        }
        let vs: String = vowel.map(String::from).unwrap_or_default();
        let sign: String = if nik {
            if vs == "ា" || vs == "ុ" {
                format!("{vs}\u{17C6}")
            } else if !vs.is_empty() {
                vs.clone()
            } else {
                "\u{17C6}".to_string()
            }
        } else if reah && vowel.is_none() {
            "\u{17C7}".to_string()
        } else {
            vs.clone()
        };
        let mut v: String = if has('\u{17D0}') && vowel.is_none() {
            "a".to_string()
        } else {
            let row = sign_row(&sign).or_else(|| sign_row(&vs)).expect("every vowel sign is in the chart");
            row.readings(ser)[0].to_string()
        };
        if vowel.is_some() && sign_row(&sign).is_none() {
            // dead in the reference (every sign is in the chart); kept so the port stays line for line
            v.push_str(if nik {
                "m"
            } else if reah {
                "h"
            } else {
                ""
            });
        } else if vowel.is_some() && reah {
            v.push('h');
        }
        out.push_str(onset(base).unwrap_or(""));
        for x in &subs {
            out.push_str(onset(*x).unwrap_or(""));
        }
        out.push_str(&v);
        open = !(nik || reah);
    }
    out
}
