//! Segmentation of typed Khmer, Khmer-script suggestions and Latin (Khmerlish) suggestions.
//! These are the engine's inner functions, one per reference function; `engine.rs` is the public API.
use crate::cluster::cluster_offsets;
use crate::compose::compose;
use crate::engine::Kind;
use crate::model::{Model, FUZZY_WORDS};
use crate::romkey::{lev_s, rom_key, within_one};
use crate::scorer::Scorer;
use crate::split_js_space;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// What the engine understood of the text before the caret.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ctx {
    pub tokens: Vec<String>,
    /// unfinished word being typed (may be empty)
    pub partial: String,
    /// last finished word, or "^" at the start of a sentence
    pub prev: String,
}

/// A candidate before it is shaped for the shells. Scores are f64, like the reference.
#[derive(Debug, Clone)]
pub struct Cand {
    pub w: Option<u32>,
    pub word: String,
    pub kind: Kind,
    pub p: Option<f64>,
    pub append: bool,
    pub roman: Option<String>,
    pub why: Option<String>,
    pub tier: Option<f64>,
}

fn cmp_f(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

impl Model {
    /// Split a run of Khmer text into known words; with `allow_tail` the last piece may be unfinished.
    pub fn segment(&self, run: &str, allow_tail: bool) -> (Vec<String>, String) {
        const INF: f64 = 1e9;
        let off = cluster_offsets(run);
        let n = off.len() - 1;
        let mut best = vec![INF; n + 1];
        let mut back: Vec<Option<(usize, bool)>> = vec![None; n + 1];
        best[0] = 0.0;
        for i in 0..n {
            if best[i] >= INF {
                continue;
            }
            let base = best[i];
            let mut upd = |j: usize, cost: f64, tail: bool| {
                if base + cost < best[j] {
                    best[j] = base + cost;
                    back[j] = Some((i, tail));
                }
            };
            for j in i + 1..=n.min(i + self.max_len) {
                if let Some(id) = self.word_id(&run[off[i]..off[j]]) {
                    upd(j, -crate::fdlibm::log(self.freq[id as usize] as f64 / self.n) + 1.5, false);
                }
            }
            upd(i + 1, 14.0, false);
            if allow_tail {
                let s = &run[off[i]..];
                upd(n, if self.has_prefix(s) { 4.0 } else { 8.0 + 3.0 * (n - i) as f64 }, true);
            }
        }
        let mut toks: Vec<String> = Vec::new();
        let (mut j, mut partial) = (n, String::new());
        while j > 0 {
            let (i, tail) = back[j].expect("every position is reachable");
            let tok = &run[off[i]..off[j]];
            // a tail piece is the whole rest of the run, which is what `s` was above
            if tail {
                partial = tok.to_string();
            } else {
                toks.push(tok.to_string());
            }
            j = i;
        }
        toks.reverse();
        (toks, partial)
    }

    /// Read the (normalized) text before the caret: finished words, the unfinished tail, the previous word.
    pub fn analyze(&self, text: &str) -> Ctx {
        let sentence = text.rsplit(['។', '៕', '៖', '!', '?', '\n']).next().unwrap_or("");
        let mut runs = split_js_space(sentence);
        let last = runs.pop().unwrap_or("");
        let mut tokens: Vec<String> = Vec::new();
        for r in runs {
            if !r.is_empty() {
                tokens.extend(self.segment(r, false).0);
            }
        }
        let mut partial = String::new();
        if !last.is_empty() {
            let (t, p) = self.segment(last, true);
            tokens.extend(t);
            partial = p;
        }
        let prev = tokens.last().cloned().unwrap_or_else(|| "^".to_string());
        Ctx { tokens, partial, prev }
    }
}

/// Ids -> scores through the scorer. `context` is the previous words, most recent last.
fn score(scorer: &dyn Scorer, m: &Model, context: &[&str], ids: &[u32]) -> Vec<f64> {
    scorer.score(m, context, ids)
}

fn context_of(prev: &str) -> Vec<&str> {
    if prev == "^" {
        vec![]
    } else {
        vec![prev]
    }
}

fn make(m: &Model, w: u32, kind: Kind, p: f64, append: bool) -> Cand {
    Cand { w: Some(w), word: m.word(w).to_string(), kind, p: Some(p), append, roman: None, why: None, tier: None }
}

/// Score ids, stable-sort by score (highest first), keep `limit`, and only then build the candidates.
fn top_scored(m: &Model, scorer: &dyn Scorer, context: &[&str], ids: &[(u32, Kind)], limit: usize) -> Vec<Cand> {
    let ps = score(scorer, m, context, &ids.iter().map(|x| x.0).collect::<Vec<_>>());
    let mut v: Vec<(u32, Kind, f64)> = ids.iter().zip(ps).map(|(&(w, k), p)| (w, k, p)).collect();
    v.sort_by(|a, b| cmp_f(b.2, a.2));
    v.truncate(limit);
    v.into_iter().map(|(w, k, p)| make(m, w, k, p, false)).collect()
}

/// Khmer-script suggestions: complete the word, predict the next one, fix near typos.
pub fn khmer_suggest(m: &Model, scorer: &dyn Scorer, ctx: &Ctx, limit: usize) -> Vec<Cand> {
    let prev_ctx = context_of(&ctx.prev);
    let prev_row = m.row(&ctx.prev);
    let mut seen: HashSet<u32> = HashSet::new();
    let mut ids: Vec<(u32, Kind)> = Vec::new();
    let mut push = |ids: &mut Vec<(u32, Kind)>, w: u32, kind: Kind| {
        if seen.insert(w) {
            ids.push((w, kind));
        }
    };

    if ctx.partial.is_empty() {
        if let Some(row) = &prev_row {
            for (w, _) in row.by_count().into_iter().take(limit) {
                push(&mut ids, w, Kind::Next);
            }
        }
        let mut w = 0u32;
        while (w as usize) < m.vocab() && ids.len() < limit {
            push(&mut ids, w, Kind::Common);
            w += 1;
        }
        return top_scored(m, scorer, &prev_ctx, &ids, limit);
    }

    for w in m.with_prefix(&ctx.partial) {
        if m.word(w) != ctx.partial {
            push(&mut ids, w, Kind::Complete);
        }
    }
    let mut res = top_scored(m, scorer, &prev_ctx, &ids, limit);

    // the typed text may already be a whole word: then offer the words that usually follow it
    let exact = m.word_id(&ctx.partial);
    if exact.is_some() {
        if let Some(row2) = m.row(&ctx.partial) {
            let p2 = [ctx.partial.as_str()];
            for (w, _) in row2.by_count() {
                if res.len() < limit && !seen.contains(&w) {
                    seen.insert(w);
                    let p = score(scorer, m, &p2, &[w])[0];
                    res.push(make(m, w, Kind::Next, p, true));
                }
            }
        }
    }

    let pc = crate::cluster::clusters(&ctx.partial);
    if exact.is_none() && res.len() < 3 && pc.len() >= 2 {
        let mut fz_ids: Vec<(u32, Kind)> = Vec::new();
        let mut wc: Vec<&str> = Vec::new();
        for w in 0..m.vocab().min(FUZZY_WORDS) as u32 {
            if seen.contains(&w) || m.word(w) == ctx.partial {
                continue;
            }
            assert!(m.fill_clusters(w, &mut wc), "within the typo-scan window");
            // the reference compares the typed clusters with the word's first clusters and with the whole word
            if within_one(&pc, &wc[..pc.len().min(wc.len())]) || within_one(&pc, &wc) {
                fz_ids.push((w, Kind::Fuzzy));
            }
        }
        res.extend(top_scored(m, scorer, &prev_ctx, &fz_ids, limit - res.len()));
    }
    res
}

/// Latin (Khmerlish) suggestions for a pending token. `prev` is the word before it ("^" at sentence start).
pub fn rom_suggest(m: &Model, scorer: &dyn Scorer, token: &str, prev: &str, limit: usize) -> Vec<Cand> {
    let key = rom_key(token);
    if key.is_empty() {
        return vec![];
    }
    let mut best = Best::default();
    let mut i = m.lower_rom(&key);
    while i < m.rom_keys.len() && m.rom_keys.get(i).starts_with(&key) {
        best.hit(m, i, if m.rom_keys.get(i) == key { 0.0 } else { 1.0 });
        i += 1;
    }
    if best.v.len() < limit && key.len() >= 3 {
        let maxd: usize = if key.len() >= 6 { 2 } else { 1 };
        for &e in &m.rom_top {
            let ek = m.rom_keys.get(e as usize);
            if key.len() > ek.len() + maxd {
                continue;
            }
            let cut = &ek.as_bytes()[..key.len().min(ek.len())];
            let d = lev_s(key.as_bytes(), ek.as_bytes()).min(lev_s(key.as_bytes(), cut));
            if d >= 1 && d <= maxd {
                best.hit(m, e as usize, 2.0 + 0.1 * d as f64);
            }
        }
    }
    let best = best.v;
    let ids: Vec<u32> = best.iter().map(|b| b.0).collect();
    let ps = score(scorer, m, &context_of(prev), &ids);
    // (word id or none for a rule guess, tier, score): sorted first, built into candidates only for the survivors
    let mut items: Vec<(Option<usize>, u32, f64, Option<f64>)> =
        best.iter().zip(ps).map(|(&(w, t), p)| (None, w, t, Some(p))).collect();
    // nothing in the word list sounds like it: spell it from the charts, flagged as a guess
    let rules = if !best.iter().any(|b| b.1 < 2.0) { compose(token, &m.prior) } else { Vec::new() };
    for i in 0..rules.len() {
        items.push((Some(i), u32::MAX, 1.8, None));
    }
    items.sort_by(|a, b| cmp_f(a.2, b.2).then_with(|| cmp_f(b.3.unwrap_or(0.0), a.3.unwrap_or(0.0))));
    items.truncate(limit);
    items
        .into_iter()
        .map(|(rule, w, t, p)| match rule {
            Some(i) => Cand {
                w: None,
                word: rules[i].word.clone(),
                kind: Kind::Rule,
                p: None,
                append: false,
                roman: Some("by rule".into()),
                why: Some(rules[i].why.clone()),
                tier: Some(1.8),
            },
            None => Cand {
                w: Some(w),
                word: m.word(w).to_string(),
                kind: if t < 1.0 {
                    Kind::Exact
                } else if t < 2.0 {
                    Kind::Prefix
                } else {
                    Kind::Fuzzy
                },
                p,
                append: false,
                roman: Some(m.shown.get(w as usize).to_string()),
                why: None,
                tier: Some(t),
            },
        })
        .collect()
}

/// Best tier per word, in first-seen order (like the reference's Map).
#[derive(Default)]
struct Best {
    v: Vec<(u32, f64)>,
    pos: HashMap<u32, usize>,
}
impl Best {
    fn hit(&mut self, m: &Model, e: usize, tier: f64) {
        let t = tier + if m.rom_gen[e] { 0.5 } else { 0.0 };
        let w = m.rom_w[e];
        match self.pos.get(&w) {
            None => {
                self.pos.insert(w, self.v.len());
                self.v.push((w, t));
            }
            Some(&i) => {
                if t < self.v[i].1 {
                    self.v[i].1 = t;
                }
            }
        }
    }
}
