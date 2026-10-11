//! Text-level parity with the JS reference: tables, ln, normalize, clusters, romanize, romKey, edit distance.
mod common;
use common::*;
use khcore::cluster::clusters;
use khcore::norm::normalize;
use khcore::romanize::romanize;
use khcore::romkey::{lev, lev_s, rom_key};
use khcore::tables::*;
use serde_json::Value;

fn ser(c: Series) -> &'static str {
    if c == Series::A {
        "A"
    } else {
        "O"
    }
}

#[test]
fn tables_match_reference() {
    let g = golden("tables.json");
    let mut f = Fails::default();
    for (i, row) in g["cons"].as_array().unwrap().iter().enumerate() {
        let c = CONS[i];
        f.check(c.0.to_string() == s(&row[0]) && ser(c.1) == s(&row[1]) && c.2 == s(&row[2]), || {
            format!("CONS[{i}] {row}")
        });
    }
    f.check(g["cons"].as_array().unwrap().len() == CONS.len(), || "CONS length".into());
    for (i, row) in g["vow"].as_array().unwrap().iter().enumerate() {
        let v = &VOW[i];
        let list = |x: &Value| x.as_array().unwrap().iter().map(|y| s(y).to_string()).collect::<Vec<_>>();
        f.check(v.s == s(&row["s"]) && v.a.to_vec() == list(&row["A"]) && v.o.to_vec() == list(&row["O"]), || {
            format!("VOW[{i}] {row}")
        });
    }
    f.check(g["vow"].as_array().unwrap().len() == VOW.len(), || "VOW length".into());
    for (k, v) in g["final"].as_object().unwrap() {
        f.check(final_of(k.chars().next().unwrap()) == Some(s(v)), || format!("FINAL {k}"));
    }
    for (k, v) in g["indep"].as_object().unwrap() {
        f.check(indep(k.chars().next().unwrap()) == Some(s(v)), || format!("INDEP {k}"));
    }
    let onl = onset_lat();
    for (i, e) in g["onsetLat"].as_array().unwrap().iter().enumerate() {
        let want: Vec<String> = e[1].as_array().unwrap().iter().map(|x| s(x).to_string()).collect();
        let got: Vec<String> = onl[i].1.iter().map(|c| c.to_string()).collect();
        f.check(onl[i].0 == s(&e[0]) && got == want, || format!("ONSET_LAT {e}"));
    }
    f.check(onl.len() == g["onsetLat"].as_array().unwrap().len(), || "ONSET_LAT length".into());
    for (i, e) in g["finalLat"].as_array().unwrap().iter().enumerate() {
        let want: Vec<String> = e[1].as_array().unwrap().iter().map(|x| s(x).to_string()).collect();
        let got: Vec<String> = FINAL_LAT[i].1.iter().map(|c| c.to_string()).collect();
        f.check(FINAL_LAT[i].0 == s(&e[0]) && got == want, || format!("FINAL_LAT {e}"));
    }
    f.check(VOWSIGNS == g["vowsigns"].as_array().unwrap().iter().map(s).collect::<String>(), || "VOWSIGNS".into());
    let toa: String = g["toA"].as_array().unwrap().iter().map(s).collect();
    let too: String = g["toO"].as_array().unwrap().iter().map(s).collect();
    f.check(
        toa.chars().all(to_a) && too.chars().all(to_o) && toa.chars().count() == 9 && too.chars().count() == 5,
        || "TO_A/TO_O".into(),
    );
    f.finish("tables");
}

fn bits(h: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(h, 16).unwrap())
}

#[test]
fn ln_matches_v8_bit_for_bit() {
    let g = golden("ln.json");
    let mut f = Fails::default();
    for p in g.as_array().unwrap() {
        let (x, want) = (bits(s(&p[0])), bits(s(&p[1])));
        let got = khcore::fdlibm::log(x);
        f.check(got.to_bits() == want.to_bits(), || format!("ln({x:e}) = {got:e}, JS {want:e}"));
    }
    f.finish("ln");
}

#[test]
fn normalize_clusters_romanize() {
    let g = golden("text.json");
    let mut f = Fails::default();
    for c in g.as_array().unwrap() {
        let input = s(&c["s"]);
        let n = normalize(input);
        f.check(n == s(&c["norm"]), || format!("normalize({input:?}) = {n:?}, JS {}", c["norm"]));
        let cl: Vec<&str> = clusters(input);
        let want: Vec<&str> = c["clusters"].as_array().unwrap().iter().map(s).collect();
        f.check(cl == want, || format!("clusters({input:?}) = {cl:?}, JS {want:?}"));
        let r = romanize(input);
        f.check(r == s(&c["roman"]), || format!("romanize({input:?}) = {r:?}, JS {}", c["roman"]));
    }
    f.finish("normalize/clusters/romanize");
}

#[test]
fn rom_key_matches() {
    let g = golden("romkey.json");
    let mut f = Fails::default();
    for c in g.as_array().unwrap() {
        let got = rom_key(s(&c["s"]));
        f.check(got == s(&c["key"]), || format!("romKey({:?}) = {got:?}, JS {}", c["s"], c["key"]));
    }
    f.finish("romKey");
}

#[test]
fn edit_distances_match() {
    let g = golden("lev.json");
    let mut f = Fails::default();
    for c in g["latin"].as_array().unwrap() {
        let d = lev_s(s(&c["a"]).as_bytes(), s(&c["b"]).as_bytes());
        f.check(d as u64 == c["d"].as_u64().unwrap(), || format!("levS {c}"));
    }
    for c in g["clusters"].as_array().unwrap() {
        let a: Vec<&str> = c["a"].as_array().unwrap().iter().map(s).collect();
        let b: Vec<&str> = c["b"].as_array().unwrap().iter().map(s).collect();
        f.check(lev(&a, &b) as u64 == c["d"].as_u64().unwrap(), || format!("lev {c}"));
    }
    f.finish("lev");
}

#[test]
fn within_one_agrees_with_lev() {
    use khcore::romkey::within_one;
    let g = golden("lev.json");
    let mut f = Fails::default();
    for c in g["clusters"].as_array().unwrap() {
        let a: Vec<&str> = c["a"].as_array().unwrap().iter().map(s).collect();
        let b: Vec<&str> = c["b"].as_array().unwrap().iter().map(s).collect();
        f.check(within_one(&a, &b) == (lev(&a, &b) <= 1), || format!("within_one {c}"));
    }
    // exhaustive on small alphabets
    let alpha = ["a", "b", "c"];
    let mut all: Vec<Vec<&str>> = vec![vec![]];
    for _ in 0..4 {
        let more: Vec<Vec<&str>> = all
            .iter()
            .filter(|v| v.len() < 4)
            .flat_map(|v| {
                alpha.iter().map(move |x| {
                    let mut n = v.clone();
                    n.push(*x);
                    n
                })
            })
            .collect();
        all.extend(more);
        all.sort();
        all.dedup();
    }
    for a in &all {
        for b in &all {
            f.check(within_one(a, b) == (lev(a, b) <= 1), || format!("within_one {a:?} {b:?}"));
        }
    }
    f.finish("within_one");
}
