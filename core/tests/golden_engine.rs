//! Engine-level parity with the JS reference on two data sets: the seed corpus and a 21,000-word synthetic one.
mod common;
use common::*;
use khcore::engine::latin_suffix;
use khcore::{Engine, Key, KeyAction, Kind, Mode};
use serde_json::Value;

fn kind(k: Kind) -> String {
    serde_json::to_value(k).unwrap().as_str().unwrap().to_string()
}

fn run(name: &str, e: &Engine) {
    let g = golden(&format!("engine_{name}.json"));
    let mut f = Fails::default();

    for c in g["analyze"].as_array().unwrap() {
        let text = s(&c["text"]);
        let ctx = e.analyze(&khcore::norm::normalize(text));
        let toks: Vec<&str> = c["tokens"].as_array().unwrap().iter().map(s).collect();
        f.check(ctx.tokens == toks && ctx.partial == s(&c["partial"]) && ctx.prev == s(&c["prev"]), || {
            format!("analyze({text:?}) = {ctx:?}, JS {c}")
        });
    }

    for c in g["khmer"].as_array().unwrap() {
        let text = s(&c["text"]);
        let before = khcore::norm::normalize(text);
        let mut ctx = e.analyze(&before);
        if let Some(w) = c["completed"].as_str() {
            ctx = khcore::suggest::Ctx { tokens: ctx.tokens, partial: String::new(), prev: w.to_string() };
        }
        let got = e.khmer_candidates(&ctx);
        let want = c["out"].as_array().unwrap();
        let same = got.len() == want.len()
            && got.iter().zip(want).all(|(a, b)| {
                a.word == s(&b["word"])
                    && kind(a.kind) == s(&b["kind"])
                    && a.append == b["append"].as_bool().unwrap()
                    && close(a.p.unwrap(), b["p"].as_f64().unwrap())
            });
        f.check(same, || {
            format!(
                "khmer {text:?} completed={}: got {:?}\n   JS {}",
                c["completed"],
                got.iter().map(|x| (&x.word, kind(x.kind))).collect::<Vec<_>>(),
                c["out"]
            )
        });
    }

    for c in g["latin"].as_array().unwrap() {
        let token = s(&c["token"]);
        let got = e.latin_candidates(token, s(&c["prev"]));
        let want = c["out"].as_array().unwrap();
        let same = got.len() == want.len()
            && got.iter().zip(want).all(|(a, b)| {
                a.word == s(&b["word"])
                    && kind(a.kind) == s(&b["kind"])
                    && a.tier.unwrap() == b["tier"].as_f64().unwrap()
                    && a.roman.as_deref() == b["rom"].as_str()
                    && a.why.as_deref() == b["why"].as_str()
                    && match (a.p, b["p"].as_f64()) {
                        (Some(x), Some(y)) => close(x, y),
                        (None, None) => true,
                        _ => false,
                    }
            });
        f.check(same, || {
            format!(
                "latin {token:?} prev={}: got {:?}\n   JS {}",
                c["prev"],
                got.iter().map(|x| (&x.word, kind(x.kind), x.tier)).collect::<Vec<_>>(),
                c["out"]
            )
        });
    }

    for c in g["compose"].as_array().unwrap() {
        let t = s(&c["t"]);
        let got = khcore::compose::compose(t, &e.model().prior);
        let want = c["out"].as_array().unwrap();
        let same = got.len() == want.len()
            && got.iter().zip(want).all(|(a, b)| {
                a.word == s(&b["word"]) && a.why == s(&b["why"]) && close(a.score, b["score"].as_f64().unwrap())
            });
        f.check(same, || format!("compose({t:?}) = {got:?}, JS {}", c["out"]));
    }

    for c in g["flows"].as_array().unwrap() {
        let text = s(&c["text"]);
        let mode = if s(&c["mode"]) == "latin" { Mode::Latin } else { Mode::Khmer };
        let completed = c["completed"].as_str();
        let got = e.suggest(text, mode, completed);
        let want = c["current"].as_array().unwrap();
        let same = got.len() == want.len()
            && got.iter().zip(want).all(|(a, b)| {
                a.word == s(&b["word"]) && kind(a.kind) == s(&b["kind"]) && a.append == b["append"].as_bool().unwrap() && a.replace == s(&b["replace"])
                    && a.roman.as_deref() == b["rom"].as_str() && a.why.as_deref() == b["why"].as_str()
                    && match (a.prob, b["p"].as_f64()) { (Some(x), Some(y)) => x == y as f32, (None, None) => true, _ => false }
                    // un-normalized input: the replacement length must cover the raw tail
                    && (!a.replace.is_empty() == (a.replace_len_utf16 > 0))
            });
        f.check(same, || {
            format!(
                "suggest {text:?} {mode:?} completed={completed:?}: got {:?}\n   JS {}",
                got.iter().map(|x| (&x.word, kind(x.kind), &x.replace)).collect::<Vec<_>>(),
                c["current"]
            )
        });
        for (kname, key) in [("space", Key::Space), ("enter", Key::Enter), ("tab", Key::Tab)] {
            let act = e.key_action(text, mode, key, completed);
            let want = &c["keys"][kname];
            let ok = match (&act, want) {
                (KeyAction::PassThrough, Value::Null) => true,
                (KeyAction::Insert(x), w) if !w.is_null() => x.word == s(&w["word"]) && x.replace == s(&w["replace"]),
                _ => false,
            };
            f.check(ok, || format!("key {kname} {text:?} {mode:?} completed={completed:?}: got {act:?}, JS {want}"));
        }
        let _ = latin_suffix(text);
    }
    f.finish(&format!("engine {name}"));
}

#[test]
fn seed_engine_matches_reference() {
    run("seed", &seed_engine());
}

#[test]
fn synthetic_engine_matches_reference() {
    run("synth", &synth_engine());
}
