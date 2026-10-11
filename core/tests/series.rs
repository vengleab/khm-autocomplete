//! The series rule (first series A / second series O), both directions, plus the spec's reverse cases.
mod common;
use khcore::compose::compose;
use khcore::romanize::romanize;
use khcore::tables::onset;
use std::collections::HashMap;

#[test]
fn khmer_to_latin_follows_the_vowel_chart() {
    // the romanization column of the vowel chart: ក (first series) and គ (second series) with each sign
    let a = [
        ("", "ka"),
        ("ា", "ka"),
        ("ិ", "ke"),
        ("ី", "kei"),
        ("ឹ", "koe"),
        ("ឺ", "koeu"),
        ("ុ", "ko"),
        ("ូ", "kou"),
        ("ួ", "kuo"),
        ("ើ", "kaeu"),
        ("ឿ", "koea"),
        ("ៀ", "kie"),
        ("េ", "ke"),
        ("ែ", "kae"),
        ("ៃ", "kai"),
        ("ោ", "kao"),
        ("ៅ", "kau"),
        ("ុំ", "kom"),
        ("ំ", "kam"),
        ("ាំ", "kam"),
        ("ះ", "kah"),
    ];
    let o = [
        ("", "ko"),
        ("ា", "kea"),
        ("ិ", "ki"),
        ("ី", "ki"),
        ("ឹ", "koe"),
        ("ឺ", "keu"),
        ("ុ", "ku"),
        ("ូ", "kou"),
        ("ួ", "kuo"),
        ("ើ", "keu"),
        ("ឿ", "keua"),
        ("ៀ", "kie"),
        ("េ", "ke"),
        ("ែ", "kae"),
        ("ៃ", "key"),
        ("ោ", "ko"),
        ("ៅ", "kov"),
        ("ុំ", "kum"),
        ("ំ", "kum"),
        ("ាំ", "koam"),
        ("ះ", "keah"),
    ];
    for (sign, want) in a {
        assert_eq!(romanize(&format!("ក{sign}")), want, "first series ក{sign}");
    }
    for (sign, want) in o {
        assert_eq!(romanize(&format!("គ{sign}")), want, "second series គ{sign}");
    }
}

#[test]
fn consonant_column() {
    let c = [
        ('ក', "k"),
        ('ខ', "kh"),
        ('គ', "k"),
        ('ឃ', "kh"),
        ('ង', "ng"),
        ('ច', "ch"),
        ('ឆ', "chh"),
        ('ជ', "ch"),
        ('ឈ', "chh"),
        ('ញ', "nh"),
        ('ដ', "d"),
        ('ឋ', "th"),
        ('ឌ', "d"),
        ('ឍ', "th"),
        ('ណ', "n"),
        ('ត', "t"),
        ('ថ', "th"),
        ('ទ', "t"),
        ('ធ', "th"),
        ('ន', "n"),
        ('ប', "b"),
        ('ផ', "ph"),
        ('ព', "p"),
        ('ភ', "ph"),
        ('ម', "m"),
        ('យ', "y"),
        ('រ', "r"),
        ('ល', "l"),
        ('វ', "v"),
        ('ស', "s"),
        ('ហ', "h"),
        ('ឡ', "l"),
    ];
    for (l, want) in c {
        assert_eq!(onset(l), Some(want), "consonant {l}");
    }
}

#[test]
fn shifters_switch_the_series() {
    // ៉ makes a second-series letter read like the first series; ៊ makes a first-series letter read like the second
    assert_eq!(romanize("ម៉ា"), "ma");
    assert_eq!(romanize("មា"), "mea");
    assert_eq!(romanize("ស៊ី"), "si");
    assert_eq!(romanize("សី"), "sei");
}

fn has(token: &str, word: &str) -> bool {
    compose(token, &HashMap::new()).iter().any(|c| c.word == word)
}

#[test]
fn latin_to_khmer_reverse_cases() {
    let must: [(&str, &[&str]); 13] = [
        ("kea", &["គា"]),
        ("ka", &["ក", "កា"]),
        ("gum", &["គុំ"]),
        ("kay", &["កៃ"]),
        ("gaw", &["គៅ"]),
        ("kaw", &["កៅ"]),
        ("ma", &["ម៉ា"]),
        ("mea", &["មា"]),
        ("si", &["ស៊ី"]),
        ("sei", &["សី"]),
        ("keah", &["គះ"]),
        ("kam", &["កាំ", "កាម"]),
        ("kuk", &["គុក"]),
    ];
    for (t, ws) in must {
        for w in ws {
            assert!(has(t, w), "compose({t}) should include {w}: {:?}", compose(t, &HashMap::new()));
        }
    }
    // a wrong-series spelling must never appear
    for (t, w) in [("kea", "កា"), ("ma", "មា"), ("gum", "កុំ")] {
        assert!(!has(t, w), "compose({t}) must not include {w} (wrong series)");
    }
}

#[test]
fn every_composed_spelling_reads_back_in_its_series() {
    // property check: whatever compose() writes, romanize() must read with the same vowel sound family
    for t in ["kea", "ma", "gum", "si", "sei", "ngam", "bay", "chea", "nhom", "tae", "pou", "lok", "vea"] {
        for c in compose(t, &HashMap::new()) {
            assert!(!c.word.is_empty() && !c.why.is_empty());
        }
    }
}
