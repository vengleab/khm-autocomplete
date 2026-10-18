//! The consonant / vowel charts as data: the default mapping file equals the built-in charts, a custom mapping behaves
//! like the JS reference (tests/golden/engine_mapped.json), and bad mappings are errors, never panics.
mod common;
use common::*;
use khcore::compose::compose_with;
use khcore::romanize::romanize_with;
use khcore::tables::Tables;
use khcore::{Engine, Kind, Mode, Model};

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn kind(k: Kind) -> String {
    serde_json::to_value(k).unwrap().as_str().unwrap().to_string()
}

fn mapped_engine() -> Engine {
    Engine::from_model(
        Model::from_json_full(
            &read("data/data.json"),
            &read("data/lexicon.json"),
            Some(&read("data/english.json")),
            Some(&read("tests/golden/mapping_custom.json")),
        )
        .unwrap(),
    )
}

#[test]
fn default_mapping_file_is_the_builtin_charts() {
    let t = Tables::from_json(&read("admin/default_mapping.json")).unwrap();
    assert_eq!(
        &t,
        Tables::builtin(),
        "admin/default_mapping.json must equal the built-in charts (cargo run -p khcore --example dump_mapping)"
    );
    // the file round-trips
    assert_eq!(Tables::from_json(&t.to_json()).unwrap(), t);
    // and an engine built with it behaves like one built without
    let (a, b) = (seed_engine(), {
        let m = Model::from_json_full(
            &read("data/data.json"),
            &read("data/lexicon.json"),
            Some(&read("data/english.json")),
            Some(&read("admin/default_mapping.json")),
        )
        .unwrap();
        Engine::from_model(m)
    });
    for t in ["knhom", "leap", "ស្រ", "water", "xjJuM"] {
        for m in [Mode::Khmer, Mode::Latin, Mode::English, Mode::Auto] {
            assert_eq!(a.suggest(t, m, None), b.suggest(t, m, None), "{t:?} {m:?}");
        }
    }
    // the default compiled file carries no mapping (it is byte-identical to what it was before mappings existed)
    assert!(!a.model().to_bytes().windows(9).any(|w| w == b"\"mapping\""));
}

#[test]
fn the_active_mapping_file_loads() {
    // data/mapping.json is what the admin pages export and the demo / khbuild use; whatever is in it must be a valid mapping
    let t = Tables::from_json(&read("data/mapping.json")).unwrap();
    assert_eq!(Tables::from_json(&t.to_json()).unwrap(), t);
}

#[test]
fn custom_mapping_matches_the_reference() {
    let e = mapped_engine();
    let g = golden("engine_mapped.json");
    let t = &e.model().tables;
    let mut f = Fails::default();
    for c in g["romanize"].as_array().unwrap() {
        let w = s(&c["word"]);
        let got = romanize_with(t, w);
        f.check(got == s(&c["roman"]), || format!("romanize {w:?} = {got:?}, JS {}", c["roman"]));
    }
    for c in g["compose"].as_array().unwrap() {
        let tk = s(&c["t"]);
        let got = compose_with(t, tk, &e.model().prior);
        let want = c["out"].as_array().unwrap();
        let same = got.len() == want.len()
            && got.iter().zip(want).all(|(a, b)| {
                a.word == s(&b["word"]) && a.why == s(&b["why"]) && close(a.score, b["score"].as_f64().unwrap())
            });
        f.check(same, || {
            format!("compose {tk:?}: got {:?}, JS {}", got.iter().map(|x| &x.word).collect::<Vec<_>>(), c["out"])
        });
    }
    for c in g["latin"].as_array().unwrap() {
        let tk = s(&c["token"]);
        let got = e.latin_candidates(tk, s(&c["prev"]));
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
                "latin {tk:?}: got {:?}, JS {}",
                got.iter().map(|x| (&x.word, x.tier)).collect::<Vec<_>>(),
                c["out"]
            )
        });
    }
    for c in g["auto"].as_array().unwrap() {
        let run = s(&c["run"]);
        let got = e.auto_candidates(run, s(&c["prev"]));
        let want = c["out"].as_array().unwrap();
        let same = got.len() == want.len()
            && got.iter().zip(want).all(|(a, b)| {
                a.word == s(&b["word"])
                    && kind(a.kind) == s(&b["kind"])
                    && a.tier.unwrap() == b["tier"].as_f64().unwrap()
                    && a.span.as_deref() == b["replace"].as_str()
                    && a.source == b["source"].as_str()
            });
        f.check(same, || {
            format!("auto {run:?}: got {:?}, JS {}", got.iter().map(|x| &x.word).collect::<Vec<_>>(), c["out"])
        });
    }
    f.finish("mapped engine");
}

#[test]
fn the_mapping_changes_what_the_engine_understands() {
    let (plain, mapped) = (seed_engine(), mapped_engine());
    // `leab` is ល + ា + ប (final spelled B): the default charts write a final ប as P, the custom mapping accepts B
    assert!(!plain.suggest("leab", Mode::Latin, None).iter().any(|c| c.word == "លាប"));
    let m = mapped.suggest("leab", Mode::Latin, None);
    assert_eq!(m[0].word, "លាប");
    assert_eq!(m[0].kind, Kind::Rule);
    // new vowel rows: ោះ with the reading "aso" after a first-series consonant
    assert!(compose_with(&mapped.model().tables, "kaso", &mapped.model().prior).iter().any(|c| c.word == "កោះ"));
}

#[test]
fn a_mapping_travels_inside_the_compiled_file() {
    let e = mapped_engine();
    let bytes = e.model().to_bytes();
    let back = Engine::from_model(Model::from_bytes(&bytes).unwrap());
    assert_eq!(back.model().tables, e.model().tables);
    assert_ne!(&back.model().tables, Tables::builtin());
    for t in ["leab", "kos", "knhom"] {
        assert_eq!(back.suggest(t, Mode::Auto, None), e.suggest(t, Mode::Auto, None), "{t}");
    }
    assert_eq!(Model::from_bytes(&bytes).unwrap().to_bytes(), bytes, "re-serialization is identical");
}

#[test]
fn bad_mappings_are_errors_not_panics() {
    let good: serde_json::Value = serde_json::from_str(&read("admin/default_mapping.json")).unwrap();
    let load = |v: &serde_json::Value| Tables::from_json(&v.to_string());
    assert!(load(&good).is_ok());
    let mut cases: Vec<(&str, serde_json::Value)> = Vec::new();
    let tweak = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = good.clone();
        f(&mut v);
        v
    };
    cases.push(("series not A/O", tweak(&|v| v["consonants"][0]["series"] = "X".into())));
    cases.push(("two letters", tweak(&|v| v["consonants"][0]["letter"] = "កខ".into())));
    cases.push(("duplicate consonant", tweak(&|v| v["consonants"][1]["letter"] = "ក".into())));
    cases.push(("uppercase onset", tweak(&|v| v["consonants"][0]["onset"] = "K".into())));
    cases.push((
        "vowel sign missing",
        tweak(&|v| {
            v["vowels"].as_array_mut().unwrap().remove(1);
        }),
    ));
    cases.push(("vowel without readings", tweak(&|v| v["vowels"][2]["A"] = serde_json::json!([]))));
    cases.push(("empty reading", tweak(&|v| v["vowels"][2]["A"] = serde_json::json!([""]))));
    cases.push(("unknown letter in a final", tweak(&|v| v["finals"][0]["letters"] = "z".into())));
    cases.push(("empty final key", tweak(&|v| v["finals"][0]["latin"] = "".into())));
    for (what, v) in &cases {
        assert!(load(v).is_err(), "{what} must be rejected");
    }
    assert!(Tables::from_json("not json").is_err());
    assert!(Tables::from_json("{}").is_err());
    assert!(Model::from_json_full(&read("data/data.json"), &read("data/lexicon.json"), None, Some("{}")).is_err());
}
