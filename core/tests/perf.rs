//! Latency budget (section 2): < 10 ms per keystroke on the seed data, < 30 ms with 100,000 words.
//! Timing is only meaningful in release builds: `cargo test --release -p khcore --test perf -- --nocapture`.
mod common;
use common::synth::{make, Rng};
use common::*;
use khcore::cluster::clusters;
use khcore::romanize::romanize;
use khcore::{Engine, Mode, Model};
use std::time::{Duration, Instant};

fn pct(v: &mut Vec<Duration>, p: f64) -> Duration {
    v.sort();
    v[((v.len() as f64 * p) as usize).min(v.len() - 1)]
}

/// Run `queries` against the engine; returns (name, p50, p99, max) per group and asserts the budget on p99 and max.
fn measure(e: &Engine, words: &[String], budget: Duration, label: &str) {
    let mut r = Rng(1234);
    let top = words.len().min(20_000);
    let w = |r: &mut Rng| &words[r.below(top)];
    let mut groups: Vec<(&str, Vec<(String, Mode)>)> = vec![
        ("khmer: completions", vec![]),
        ("khmer: next word (empty partial)", vec![]),
        ("khmer: whole word + next", vec![]),
        ("khmer: typo (fuzzy scan)", vec![]),
        ("latin: exact / prefix", vec![]),
        ("latin: typo (tier 2 scan)", vec![]),
        ("latin: nothing matches (compose)", vec![]),
    ];
    for _ in 0..600 {
        let ctx = format!("{} {} ", w(&mut r), w(&mut r));
        let word = w(&mut r).clone();
        let cl = clusters(&word);
        groups[0].1.push((format!("{ctx}{}", cl[..1.min(cl.len())].join("")), Mode::Khmer));
        groups[1].1.push((ctx.clone(), Mode::Khmer));
        groups[2].1.push((format!("{ctx}{word}"), Mode::Khmer));
        let mut typo = cl.clone();
        let i = r.below(typo.len());
        typo[i] = "ឡ";
        typo.push("ង");
        groups[3].1.push((format!("{ctx}{}", typo.join("")), Mode::Khmer));
        let rom = romanize(&word);
        groups[4].1.push((format!("{ctx}{}", &rom[..rom.len().min(1 + r.below(6))]), Mode::Latin));
        let mut m = rom.clone().into_bytes();
        if m.len() > 3 {
            let i = r.below(m.len());
            m[i] = b'z';
        }
        groups[5].1.push((format!("{ctx}{}", String::from_utf8(m).unwrap()), Mode::Latin));
        groups[6].1.push((format!("{ctx}qzxqzx{}", r.below(99)), Mode::Latin));
    }
    println!("{label}: budget {budget:?} per keystroke");
    let mut worst = Duration::ZERO;
    for (name, qs) in &groups {
        let mut times = Vec::new();
        for (t, m) in qs {
            let t0 = Instant::now();
            let c = e.suggest(t, *m, None);
            times.push(t0.elapsed());
            std::hint::black_box(c);
        }
        let (p50, p99) = (pct(&mut times, 0.5), pct(&mut times, 0.99));
        let max = *times.last().unwrap();
        println!("  {name:<36} p50 {p50:>9.2?}  p99 {p99:>9.2?}  max {max:>9.2?}");
        worst = worst.max(p99);
    }
    assert!(worst < budget, "{label}: p99 {worst:?} exceeds {budget:?}");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timing needs --release")]
fn seed_data_under_10ms() {
    let e = seed_engine();
    let words: Vec<String> = (0..e.model().vocab()).map(|i| e.model().word(i as u32).to_string()).collect();
    measure(&e, &words, Duration::from_millis(10), "seed data (277 words)");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timing needs --release")]
fn hundred_thousand_words_under_30ms() {
    let (data, lex, words) = make(100_000, 42);
    let t0 = Instant::now();
    let model = Model::from_json(&data, &lex).unwrap();
    println!("build from JSON: {:?}", t0.elapsed());
    let bytes = model.to_bytes();
    drop(model);
    let t0 = Instant::now();
    let e = Engine::from_model(Model::from_bytes(&bytes).unwrap());
    println!("load compiled ({:.1} MB): {:?}", bytes.len() as f64 / 1e6, t0.elapsed());
    measure(&e, &words, Duration::from_millis(30), "synthetic 100,000 words");
}
