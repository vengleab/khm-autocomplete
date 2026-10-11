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
    let mut text = String::new();
    for ch in input.chars() {
        if ch == ' ' {
            if let KeyAction::Insert(c) = e.key_action(&text, Mode::Latin, Key::Space, None) {
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
        for c in g["flows"].as_array().unwrap() {
            let mode = if s(&c["mode"]) == "latin" { Mode::Latin } else { Mode::Khmer };
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
