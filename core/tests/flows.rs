//! Typing flows the way a shell drives the engine, plus raw (un-normalized) text and the compiled format.
mod common;
use common::*;
use khcore::{Engine, Key, KeyAction, Kind, Mode, Model};

/// Replace the last `n` UTF-16 units of `text` with `with`, like a shell does with `replacementRange`.
fn replace_tail(text: &str, n: usize, with: &str) -> String {
    let u: Vec<u16> = text.encode_utf16().collect();
    let mut out = String::from_utf16(&u[..u.len() - n]).unwrap();
    out.push_str(with);
    out
}

/// A tiny model of a text field with a Latin-mode keyboard: type one character at a time.
fn type_latin(e: &Engine, input: &str) -> String {
    type_in(e, Mode::Latin, input)
}

fn type_in(e: &Engine, mode: Mode, input: &str) -> String {
    let mut text = String::new();
    for ch in input.chars() {
        if ch == ' ' {
            if let KeyAction::Insert(c) = e.key_action(&text, mode, Key::Space, None) {
                text = replace_tail(&text, c.replace_len_utf16, &c.word);
                continue; // the space was swallowed
            }
        }
        text.push(ch);
    }
    text
}

#[test]
fn latin_flow_knyom_soksabay() {
    let e = seed_engine();
    assert_eq!(type_latin(&e, "knyom soksabay "), "ខ្ញុំសុខសប្បាយ");
    // no token: the space is not swallowed
    assert_eq!(type_latin(&e, " "), " ");
    // no candidate: text and space are left alone
    let e2 = seed_engine();
    assert!(matches!(e2.key_action("", Mode::Latin, Key::Space, None), KeyAction::PassThrough));
}

#[test]
fn english_flow_words_become_khmer() {
    let e = seed_engine();
    // each English word is converted when Space is pressed; Khmer is written without spaces between words
    assert_eq!(type_in(&e, Mode::English, "i love cambodia "), "ខ្ញុំស្រលាញ់កម្ពុជា");
    assert_eq!(type_in(&e, Mode::English, "eating rice "), "ញ៉ាំបាយ"); // eating -> eat (inflection)
    assert_eq!(type_in(&e, Mode::English, "thank you "), "អរគុណអ្នក");
    assert_eq!(type_in(&e, Mode::English, "good morning "), "ល្អព្រឹក");
    // a word with no gloss stays English and the space is kept
    assert_eq!(type_in(&e, Mode::English, "xyzq water "), "xyzq ទឹក");
    // a half-typed word is not converted by Space (only Tab/Enter or a click take a prefix match)
    assert_eq!(type_in(&e, Mode::English, "wat "), "wat ");
}

#[test]
fn english_mode_details() {
    let e = seed_engine();
    let c = e.suggest("water", Mode::English, None);
    assert_eq!(c[0].word, "ទឹក");
    assert_eq!(c[0].kind, Kind::Exact);
    assert_eq!(c[0].roman.as_deref(), Some("water"));
    assert_eq!(c[0].replace, "water");
    assert_eq!(c[0].replace_len_utf16, 5);
    assert_eq!(Engine::keystroke_gain(&c[0]), 0);
    // inflections, case and hyphens
    assert_eq!(e.suggest("Teachers", Mode::English, None)[0].word, "គ្រូ");
    assert!(e.suggest("good-morning", Mode::English, None).iter().any(|x| x.word == "អរុណសួស្តី"));
    // a close typo
    let t = e.suggest("watr", Mode::English, None);
    assert_eq!((t[0].word.as_str(), t[0].kind), ("ទឹក", Kind::Fuzzy));
    // nothing for gibberish, and after a Khmer word the context still ranks candidates
    assert!(e.suggest("xyzq", Mode::English, None).is_empty());
    assert!(!e.suggest("ខ្ញុំ go", Mode::English, None).is_empty());
    // Enter takes the top candidate of a pending English word, even a prefix match
    match e.key_action("wat", Mode::English, Key::Enter, None) {
        KeyAction::Insert(c) => assert_eq!(c.word, "ទឹក"),
        a => panic!("{a:?}"),
    }
    // an engine without a glossary offers nothing in English mode and does not fail
    let plain = Engine::load(
        root().join("data/data.json").to_str().unwrap(),
        root().join("data/lexicon.json").to_str().unwrap(),
    )
    .unwrap();
    assert!(plain.suggest("water", Mode::English, None).is_empty());
}

#[test]
fn every_gloss_finds_its_word() {
    // Single-word glosses must bring their Khmer word back as an exact match. A multi-word gloss ("good morning")
    // cannot be typed as one token (a token has no spaces), so it must be reachable by its first word instead.
    let e = seed_engine();
    let g: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(root().join("data/english.json")).unwrap()).unwrap();
    let mut bad = Vec::new();
    for (word, glosses) in &g {
        for gl in glosses.as_str().unwrap().split(',') {
            let gl = gl.trim();
            let first = gl.split(' ').next().unwrap();
            let multi = gl.contains(' ');
            if multi && first.len() <= 2 {
                continue; // "a lot", "in order to": a one- or two-letter first word matches too many entries
            }
            let hit = e
                .suggest(first, Mode::English, None)
                .into_iter()
                .any(|c| c.word == *word && (multi || c.kind == Kind::Exact));
            if !hit {
                bad.push(format!("{gl} -> {word}"));
            }
        }
    }
    assert!(bad.is_empty(), "glosses that do not find their word: {bad:?}");
}

#[test]
fn accepting_a_word_predicts_the_next() {
    let e = seed_engine();
    let text = "ខ្ញុំ ចូលចិត្ត ";
    let c = e.suggest(text, Mode::Khmer, None);
    assert!(
        c.iter().any(|x| x.word == "ញ៉ាំ"),
        "ញ៉ាំ should follow ចូលចិត្ត: {:?}",
        c.iter().map(|x| &x.word).collect::<Vec<_>>()
    );
    // accept it; until the next keystroke it counts as a finished word, never as a partial again
    let after = format!("{text}ញ៉ាំ");
    let next = e.suggest(&after, Mode::Khmer, Some("ញ៉ាំ"));
    assert!(
        next.iter().any(|x| x.word == "បាយ"),
        "បាយ should be among the next words: {:?}",
        next.iter().map(|x| &x.word).collect::<Vec<_>>()
    );
    assert!(next.iter().all(|x| x.replace.is_empty() && x.kind != Kind::Complete));
}

#[test]
fn typo_suggests_the_close_spelling() {
    let e = seed_engine();
    let c = e.suggest("ស្រលាង", Mode::Khmer, None);
    let hit = c.iter().find(|x| x.word == "ស្រលាញ់").expect("ស្រលាញ់ among candidates");
    assert_eq!(hit.kind, Kind::Fuzzy);
    assert_eq!(hit.replace, "ស្រលាង");
}

#[test]
fn replace_length_counts_the_raw_text() {
    let e = seed_engine();
    // a zero-width space inside the partial: the replacement must cover it, in UTF-16 units
    let raw = "ស្\u{200B}រ";
    let c = e.suggest(raw, Mode::Khmer, None);
    let hit = c.iter().find(|x| x.word == "ស្រលាញ់").expect("completion for ស្រ");
    assert_eq!(hit.replace, "ស្រ");
    assert_eq!(hit.replace_len_utf16, raw.encode_utf16().count());
    // marks typed in the wrong order are normalized before matching but the length is the raw length
    let raw2 = "ខ្ញុំ ម\u{17B6}\u{17C9}";
    let c2 = e.suggest(raw2, Mode::Khmer, None);
    for x in &c2 {
        assert!(x.replace_len_utf16 <= raw2.encode_utf16().count());
    }
    // append candidates replace nothing
    let c3 = e.suggest("ខ្ញុំ", Mode::Khmer, None);
    assert!(c3.iter().filter(|x| x.append).all(|x| x.replace.is_empty() && x.replace_len_utf16 == 0));
}

#[test]
fn keystroke_gain_counts_saved_presses() {
    let e = seed_engine();
    let c = e.suggest("ស្រ", Mode::Khmer, None);
    let hit = c.iter().find(|x| x.word == "ស្រលាញ់").unwrap();
    let want = hit.word.chars().count() - hit.replace.chars().count() - 1;
    assert_eq!(Engine::keystroke_gain(hit), want);
    let l = e.suggest("knyom", Mode::Latin, None);
    assert_eq!(Engine::keystroke_gain(&l[0]), 0);
}

#[test]
fn modes_and_edges_do_not_panic() {
    let e = seed_engine();
    for t in ["", " ", "\n", "។", "a", "ក្", "\u{17D2}", "😀", "ខ្ញុំ\u{200B}", "-", "'", "x1"] {
        for m in [Mode::Khmer, Mode::Latin] {
            for j in [None, Some("ខ្ញុំ")] {
                let _ = e.suggest(t, m, j);
                for k in [Key::Space, Key::Enter, Key::Tab] {
                    let _ = e.key_action(t, m, k, j);
                }
            }
        }
    }
}

fn digest(c: &[khcore::Candidate]) -> Vec<(String, String, usize, bool, Option<u32>, Option<String>)> {
    c.iter()
        .map(|x| {
            (
                x.word.clone(),
                format!("{:?}", x.kind),
                x.replace_len_utf16,
                x.append,
                x.prob.map(f32::to_bits),
                x.roman.clone(),
            )
        })
        .collect()
}

#[test]
fn compiled_format_matches_json_loading() {
    for (name, e) in [("seed", seed_engine()), ("synth", synth_engine())] {
        let bytes = e.model().to_bytes();
        let b = Engine::from_model(Model::from_bytes(&bytes).unwrap());
        let g = golden(&format!("engine_{name}.json"));
        let mut f = Fails::default();
        let all = g["flows"]
            .as_array()
            .unwrap()
            .iter()
            .chain(g["flowsEn"].as_array().unwrap())
            .chain(g["flowsAuto"].as_array().unwrap());
        for c in all {
            let mode = match s(&c["mode"]) {
                "latin" => Mode::Latin,
                "english" => Mode::English,
                "auto" => Mode::Auto,
                _ => Mode::Khmer,
            };
            let (t, j) = (s(&c["text"]), c["completed"].as_str());
            f.check(digest(&e.suggest(t, mode, j)) == digest(&b.suggest(t, mode, j)), || {
                format!("{name}: {t:?} {mode:?}")
            });
        }
        // a second round trip is byte-identical
        f.check(Model::from_bytes(&bytes).unwrap().to_bytes() == bytes, || format!("{name}: re-serialization differs"));
        f.finish(&format!("compiled {name}"));
    }
}

#[test]
fn compiled_v1_files_still_load() {
    // a v1 file is a v2 file without the English sections and with the old magic
    let e = Engine::from_model(
        Model::from_json(
            &std::fs::read_to_string(root().join("data/data.json")).unwrap(),
            &std::fs::read_to_string(root().join("data/lexicon.json")).unwrap(),
        )
        .unwrap(),
    );
    let mut b = e.model().to_bytes();
    b[..8].copy_from_slice(b"KHDATA01");
    // drop the last two sections (english keys, english ids): each is a u64 length + payload
    let mut pos = 8;
    let mut ends = vec![];
    while pos < b.len() {
        let n = u64::from_le_bytes(b[pos..pos + 8].try_into().unwrap()) as usize;
        pos += 8 + n;
        ends.push(pos);
    }
    b.truncate(ends[ends.len() - 3]);
    let v1 = Engine::from_model(Model::from_bytes(&b).unwrap());
    assert_eq!(v1.suggest("knyom", Mode::Latin, None)[0].word, "ខ្ញុំ");
    assert!(v1.suggest("water", Mode::English, None).is_empty());
}

#[test]
fn corrupt_compiled_data_is_an_error_not_a_panic() {
    let bytes = seed_engine().model().to_bytes();
    assert!(Model::from_bytes(&[]).is_err());
    assert!(Model::from_bytes(b"KHDATA99").is_err());
    for cut in (0..bytes.len()).step_by(97) {
        let _ = Model::from_bytes(&bytes[..cut]);
    }
    let mut r = common::synth::Rng(99);
    for _ in 0..300 {
        let mut b = bytes.clone();
        for _ in 0..1 + r.below(4) {
            let i = r.below(b.len());
            b[i] ^= 1 << r.below(8);
        }
        if let Ok(m) = Model::from_bytes(&b) {
            // a flip that still validates must not make the engine panic
            let e = Engine::from_model(m);
            let _ = e.suggest("ខ្ញុំ ស្រ", Mode::Khmer, None);
            let _ = e.suggest("knyom", Mode::Latin, None);
        }
    }
}

// ---------------------------------------------------------------------------------------------- Auto mode

/// The Khmer state of the shells: nothing is remapped, ASCII stays in the document until Space converts it.
fn type_auto(e: &Engine, input: &str) -> String {
    type_in(e, Mode::Auto, input)
}

fn kinds(c: &[khcore::Candidate]) -> Vec<Kind> {
    c.iter().map(|x| x.kind).collect()
}

#[test]
fn auto_flow_converts_what_was_typed() {
    let e = seed_engine();
    // Khmerlish
    assert_eq!(type_auto(&e, "knhom chong "), "ខ្ញុំចង់");
    assert_eq!(type_auto(&e, "knyom soksabay "), "ខ្ញុំសុខសប្បាយ");
    // English
    assert_eq!(type_auto(&e, "water "), "ទឹក");
    assert_eq!(type_auto(&e, "thank you "), "អរគុណអ្នក");
    assert_eq!(type_auto(&e, "eating rice love cambodia "), "ញ៉ាំបាយស្រលាញ់កម្ពុជា");
    // a short English word that is also a Khmerlish spelling goes to Khmerlish (same tier, Khmerlish first): `i` is អ្វី here
    assert_eq!(type_auto(&e, "i "), "អ្វី");
    assert_eq!(type_in(&e, Mode::English, "i "), "ខ្ញុំ");
    // keys of the Khmer (NiDA) layout, typed as the ASCII of a US keyboard: x j J u M -> ខ ្ ញ ុ ំ
    assert_eq!(type_auto(&e, "xjJuM "), "ខ្ញុំ");
    // all three in one sentence, and Khmer script typed with the OS keyboard in between
    assert_eq!(type_auto(&e, "knhom water xjJuM "), "ខ្ញុំទឹកខ្ញុំ");
    // the space after Khmer script, like a space with nothing pending, is an ordinary space
    assert_eq!(type_auto(&e, "ខ្ញុំ chong "), "ខ្ញុំ ចង់");
    assert_eq!(type_auto(&e, " "), " ");
    assert_eq!(type_auto(&e, "ខ្ញុំ "), "ខ្ញុំ ");
}

#[test]
fn auto_unknown_text_stays_as_typed_and_offers_two_chips() {
    let e = seed_engine();
    // `windows` is no Khmerlish word, has no gloss and its NiDA reading is no word
    assert_eq!(type_auto(&e, "windows "), "windows ");
    let c = e.suggest("windows", Mode::Auto, None);
    assert_eq!(kinds(&c), [Kind::Layout, Kind::Raw]);
    assert_eq!(c[0].word, "ឹិនដោឹស"); // the keys read as Khmer
    assert_eq!(c[0].roman.as_deref(), Some("NiDA"));
    assert_eq!(c[0].source, Some("nida"));
    assert_eq!((c[1].word.as_str(), c[1].source), ("windows", Some("raw")));
    assert!(c.iter().all(|x| x.replace == "windows" && x.replace_len_utf16 == 7));
    // Enter leaves a raw-only list alone (it is a newline); Tab is the explicit request: the NiDA reading
    assert!(matches!(e.key_action("windows", Mode::Auto, Key::Enter, None), KeyAction::PassThrough));
    match e.key_action("windows", Mode::Auto, Key::Tab, None) {
        KeyAction::Insert(c) => assert_eq!((c.word.as_str(), c.kind), ("ឹិនដោឹស", Kind::Layout)),
        a => panic!("{a:?}"),
    }
    // picking the raw chip keeps the text and ends the run
    let raw = &c[1];
    assert_eq!(replace_tail("windows", raw.replace_len_utf16, &raw.word), "windows");
    // typos only: weak candidates stay in front of the chips, Space still does not convert
    let t = e.suggest("sralan", Mode::Auto, None);
    assert_eq!(t[0].word, "ស្រលាញ់");
    assert_eq!(t[0].kind, Kind::Fuzzy);
    assert_eq!(&kinds(&t)[t.len() - 2..], [Kind::Layout, Kind::Raw]);
    assert_eq!(type_auto(&e, "sralan "), "sralan ");
    // Enter takes a real candidate even when it is only a typo fix
    match e.key_action("sralan", Mode::Auto, Key::Enter, None) {
        KeyAction::Insert(c) => assert_eq!(c.word, "ស្រលាញ់"),
        a => panic!("{a:?}"),
    }
    // digits never auto-convert; Tab can still read them as Khmer digits
    assert_eq!(type_auto(&e, "123 "), "123 ");
    match e.key_action("123", Mode::Auto, Key::Tab, None) {
        KeyAction::Insert(c) => assert_eq!(c.word, "១២៣"),
        a => panic!("{a:?}"),
    }
}

#[test]
fn auto_space_converts_only_a_confident_top() {
    let e = seed_engine();
    // a half-typed Khmerlish word and a half-typed English word: shown, not applied (Latin mode would convert the first)
    assert_eq!(type_auto(&e, "phnom "), "phnom ");
    assert_eq!(type_in(&e, Mode::Latin, "phnom "), "ភ្នំពេញ");
    assert_eq!(type_auto(&e, "wate "), "wate ");
    // Tab takes the top, a prefix match included
    match e.key_action("phnom", Mode::Auto, Key::Tab, None) {
        KeyAction::Insert(c) => assert_eq!((c.word.as_str(), c.source), ("ភ្នំពេញ", Some("khmerlish"))),
        a => panic!("{a:?}"),
    }
    // an unfinished NiDA word offers its completions at tier 1; Space leaves it, Tab completes it
    let p = e.suggest("xjJ", Mode::Auto, None);
    assert_eq!((p[0].word.as_str(), p[0].kind, p[0].source), ("ខ្ញុំ", Kind::Prefix, Some("nida")));
    assert_eq!(type_auto(&e, "xjJ "), "xjJ ");
    assert!(matches!(e.key_action("xjJ", Mode::Auto, Key::Tab, None), KeyAction::Insert(_)));
    // a glossed English inflection converts: `meeting` -> meet (the glossary entry), the cost of one Khmer state
    assert_eq!(type_auto(&e, "meeting "), "ជួប");
}

#[test]
fn auto_merge_order_and_dedupe() {
    let e = seed_engine();
    // `wat` is Khmerlish វត្ត (exact) and a prefix of English "water" (ទឹក): tier first, then source
    let c = e.suggest("wat", Mode::Auto, None);
    assert_eq!((c[0].word.as_str(), c[0].source), ("វត្ត", Some("khmerlish")));
    assert_eq!((c[1].word.as_str(), c[1].source), ("ទឹក", Some("english")));
    // one chip per word
    let mut words: Vec<&str> = c.iter().map(|x| x.word.as_str()).collect();
    words.sort();
    words.dedup();
    assert_eq!(words.len(), c.len());
    // a NiDA word is a hit between exact (0) and prefix (1)
    let n = e.suggest("xjJuM", Mode::Auto, None);
    assert_eq!((n[0].word.as_str(), n[0].source, n[0].kind), ("ខ្ញុំ", Some("nida"), Kind::Exact));
}

#[test]
fn auto_text_ending_in_khmer_uses_the_khmer_script_path() {
    let e = seed_engine();
    let a = e.suggest("ខ្ញុំ ចូលចិត្ត ", Mode::Auto, None);
    let k = e.suggest("ខ្ញុំ ចូលចិត្ត ", Mode::Khmer, None);
    assert!(a.iter().any(|x| x.word == "ញ៉ាំ"));
    assert_eq!(digest(&a), digest(&k));
    assert!(a.iter().all(|x| x.source.is_none()));
    // typos and completions of Khmer script, as before
    assert_eq!(e.suggest("ស្រ", Mode::Auto, None)[0].word, "ស្រលាញ់");
    assert!(e.suggest("ស្រលាង", Mode::Auto, None).iter().any(|x| x.word == "ស្រលាញ់"));
    // Enter is the app's, Tab takes the top, Space is a plain space
    assert!(matches!(e.key_action("ខ្ញុំ ", Mode::Auto, Key::Enter, None), KeyAction::PassThrough));
    assert!(matches!(e.key_action("ខ្ញុំ ", Mode::Auto, Key::Space, None), KeyAction::PassThrough));
    assert!(matches!(e.key_action("ស្រ", Mode::Auto, Key::Tab, None), KeyAction::Insert(_)));
    // a word just accepted ends the run, even though the text before the caret ends in ASCII
    let j = e.suggest("water", Mode::Auto, Some("ទឹក"));
    assert!(j.iter().all(|x| x.source.is_none() && x.replace.is_empty()));
    assert!(matches!(e.key_action("water", Mode::Auto, Key::Enter, Some("ទឹក")), KeyAction::PassThrough));
}

#[test]
fn auto_spans_are_per_candidate() {
    let e = seed_engine();
    // punctuation before the run: Khmerlish and English replace the trailing letters, NiDA the whole run
    let text = "hello,water";
    let c = e.suggest(text, Mode::Auto, None);
    assert_eq!((c[0].word.as_str(), c[0].source), ("ទឹក", Some("english")));
    assert_eq!((c[0].replace.as_str(), c[0].replace_len_utf16), ("water", 5));
    match e.key_action(text, Mode::Auto, Key::Space, None) {
        KeyAction::Insert(c) => assert_eq!(replace_tail(text, c.replace_len_utf16, &c.word), "hello,ទឹក"),
        a => panic!("{a:?}"),
    }
    assert_eq!(type_auto(&e, "hello,water "), "hello,ទឹក");
    // the NiDA chips of the same run cover all of it
    let all = e.suggest("hello,zzzz", Mode::Auto, None);
    assert_eq!(kinds(&all), [Kind::Layout, Kind::Raw]);
    assert!(all.iter().all(|x| x.replace == "hello,zzzz" && x.replace_len_utf16 == 10));
    // keys of the Khmer layout use digits and punctuation too: `.` is ។ and `[` is ៀ
    assert_eq!(khcore::suggest::nida_read("kd[").as_deref(), Some("កដៀ"));
    assert_eq!(khcore::suggest::nida_read("a.").as_deref(), Some("ា។"));
    // after Khmer script: the run starts where the ASCII starts
    let k = e.suggest("ខ្ញុំwater", Mode::Auto, None);
    assert_eq!((k[0].word.as_str(), k[0].replace.as_str(), k[0].replace_len_utf16), ("ទឹក", "water", 5));
    // raw text with a zero-width space before the run: lengths count the raw UTF-16 units of the span only
    let raw = "ខ្ញុំ\u{200B}water";
    let r = e.suggest(raw, Mode::Auto, None);
    assert_eq!((r[0].replace.as_str(), r[0].replace_len_utf16), ("water", 5));
    // and a zero-width space inside the run is part of the raw span (the normalized run is "water" again)
    let inner = "wa\u{200B}ter";
    let i = e.suggest(inner, Mode::Auto, None);
    assert_eq!(i[0].word, "ទឹក");
    assert_eq!(i[0].replace_len_utf16, inner.encode_utf16().count());
}

#[test]
fn auto_json_has_source_only_in_auto_mode() {
    let e = seed_engine();
    let a = khcore::api_json::candidates(&e.suggest("water", Mode::Auto, None));
    let a: serde_json::Value = serde_json::from_str(&a).unwrap();
    assert_eq!(a[0]["source"], "english");
    assert_eq!(a[0]["gain"], 0);
    let l = khcore::api_json::candidates(&e.suggest("knyom", Mode::Latin, None));
    let l: serde_json::Value = serde_json::from_str(&l).unwrap();
    assert!(l[0].get("source").is_none(), "other modes keep their JSON as it was");
    let k = khcore::api_json::key_action(&e.key_action("water", Mode::Auto, Key::Space, None));
    assert!(k.contains("\"source\":\"english\""));
}

#[test]
fn auto_edges_do_not_panic() {
    let e = seed_engine();
    for t in [
        "",
        " ",
        "\n",
        "។",
        "a",
        "ក្",
        "\u{17D2}",
        "😀",
        "ខ្ញុំ\u{200B}",
        "-",
        "'",
        "x1",
        "~",
        "\\",
        "\"",
        "ខ្ញុំ😀abc",
        "a\u{0301}b",
        "ក៏water",
    ] {
        for j in [None, Some("ខ្ញុំ")] {
            let _ = e.suggest(t, Mode::Auto, j);
            for k in [Key::Space, Key::Enter, Key::Tab] {
                let _ = e.key_action(t, Mode::Auto, k, j);
            }
        }
    }
}
