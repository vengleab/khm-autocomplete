//! Language data: vocabulary, frequencies, bigrams and the Latin sound index.
//! Built from `data.json` + `lexicon.json` (interchange format) or loaded from the compiled `khdata.bin`.
use crate::cluster::{cluster_offsets, clusters};
use crate::romanize::romanize;
use crate::romkey::rom_key;
use crate::split_js_space;
use fst::{IntoStreamer, Streamer};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// Words below this index feed the typo scans (Khmer fuzzy and Latin tier 2).
pub const FUZZY_WORDS: usize = 20_000;
const MAGIC: &[u8; 8] = b"KHDATA01";

#[derive(Debug)]
pub enum Error {
    Json(String),
    Format(String),
    Io(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Json(s) => write!(f, "bad data.json/lexicon.json: {s}"),
            Error::Format(s) => write!(f, "bad compiled data: {s}"),
            Error::Io(s) => write!(f, "io error: {s}"),
        }
    }
}
impl std::error::Error for Error {}

/// Strings stored in one buffer; `get(i)` is a slice, so 100k words are not 100k allocations.
#[derive(Default, Clone)]
pub struct StrTable {
    blob: String,
    off: Vec<u32>,
}
impl StrTable {
    pub fn new<S: AsRef<str>>(items: impl IntoIterator<Item = S>) -> Self {
        let mut t = StrTable { blob: String::new(), off: vec![0] };
        for s in items {
            t.blob.push_str(s.as_ref());
            t.off.push(t.blob.len() as u32);
        }
        t
    }
    pub fn len(&self) -> usize {
        self.off.len() - 1
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, i: usize) -> &str {
        &self.blob[self.off[i] as usize..self.off[i + 1] as usize]
    }
}

/// One bigram row: the words that followed a context word, with counts.
pub struct Row<'a> {
    pub ids: &'a [u32],
    pub cnt: &'a [u32],
    pub total: f64,
}
impl Row<'_> {
    pub fn count(&self, w: u32) -> u32 {
        self.ids.iter().position(|&x| x == w).map_or(0, |i| self.cnt[i])
    }
    /// (word, count) sorted by count, highest first; ties keep file order.
    pub fn by_count(&self) -> Vec<(u32, u32)> {
        let mut v: Vec<(u32, u32)> = self.ids.iter().copied().zip(self.cnt.iter().copied()).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v
    }
}

#[derive(Deserialize, Default)]
struct JsonMeta {
    sentences: Option<u64>,
    tokens: Option<u64>,
    source: Option<String>,
}
#[derive(Deserialize)]
struct JsonData {
    #[serde(default)]
    meta: JsonMeta,
    words: Vec<String>,
    freq: Vec<u64>,
    #[serde(default)]
    bi: HashMap<String, Vec<u64>>,
}

pub struct Model {
    pub words: StrTable,
    pub freq: Vec<u32>,
    pub n: f64,
    pub sentences: u64,
    pub tokens: u64,
    pub source: String,
    pub index: fst::Map<Vec<u8>>,
    bi_off: Vec<u32>,
    bi_ids: Vec<u32>,
    bi_cnt: Vec<u32>,
    pub rom_keys: StrTable,
    pub rom_w: Vec<u32>,
    pub rom_gen: Vec<bool>,
    pub shown: StrTable,
    /// indices into the rom_* arrays whose word id is below FUZZY_WORDS
    pub rom_top: Vec<u32>,
    /// share of corpus tokens by first letter; ranks `compose` guesses
    pub prior: HashMap<char, f64>,
    pub max_len: usize,
    /// cluster byte offsets for the first FUZZY_WORDS words
    cl: Vec<Box<[u32]>>,
}

impl Model {
    pub fn from_json(data: &str, lexicon: &str) -> Result<Model, Error> {
        let d: JsonData = serde_json::from_str(data).map_err(|e| Error::Json(e.to_string()))?;
        let lex: HashMap<String, String> = serde_json::from_str(lexicon).map_err(|e| Error::Json(e.to_string()))?;
        if d.words.len() != d.freq.len() {
            return Err(Error::Json("words and freq differ in length".into()));
        }
        let freq: Vec<u32> = d
            .freq
            .iter()
            .map(|&f| u32::try_from(f).map_err(|_| Error::Json("frequency does not fit in u32".into())))
            .collect::<Result<_, _>>()?;
        let nw = d.words.len();
        // bigram rows, like the reference Map: a repeated next word keeps its first position, last count wins
        let mut rows: Vec<Vec<(u32, u32)>> = vec![Vec::new(); nw + 1];
        for (k, flat) in &d.bi {
            let slot = if k == "^" {
                nw
            } else {
                match k.parse::<usize>() {
                    Ok(i) if i < nw => i,
                    _ => continue,
                }
            };
            let r = &mut rows[slot];
            for p in flat.chunks_exact(2) {
                let (j, c) = (p[0] as u32, p[1] as u32);
                match r.iter_mut().find(|x| x.0 == j) {
                    Some(x) => x.1 = c,
                    None => r.push((j, c)),
                }
            }
        }
        let (mut bi_off, mut bi_ids, mut bi_cnt) = (vec![0u32], Vec::new(), Vec::new());
        for r in &rows {
            for &(j, c) in r {
                bi_ids.push(j);
                bi_cnt.push(c);
            }
            bi_off.push(bi_ids.len() as u32);
        }
        // sound index: lexicon spellings first, then the generated spelling (lower priority)
        let mut entries: Vec<(String, u32, bool)> = Vec::new();
        let mut shown: Vec<String> = Vec::with_capacity(nw);
        for (i, w) in d.words.iter().enumerate() {
            let sp: Vec<&str> = lex
                .get(w)
                .map(|s| split_js_space(s))
                .unwrap_or_default()
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect();
            let mut seen: HashSet<String> = HashSet::new();
            for x in &sp {
                let k = rom_key(x);
                if !k.is_empty() && seen.insert(k.clone()) {
                    entries.push((k, i as u32, false));
                }
            }
            let g = romanize(w);
            let k = rom_key(&g);
            if !k.is_empty() && seen.insert(k.clone()) {
                entries.push((k, i as u32, true));
            }
            shown.push(if sp.is_empty() { g } else { sp[0].replace('-', " ") });
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0)); // stable, like the reference
        let rom_keys = StrTable::new(entries.iter().map(|e| e.0.as_str()));
        let rom_w: Vec<u32> = entries.iter().map(|e| e.1).collect();
        let rom_gen: Vec<bool> = entries.iter().map(|e| e.2).collect();
        let meta = d.meta;
        Model::assemble(
            StrTable::new(&d.words),
            freq,
            meta.sentences.filter(|&s| s != 0).unwrap_or(1),
            meta.tokens.unwrap_or(0),
            meta.source.unwrap_or_default(),
            (bi_off, bi_ids, bi_cnt),
            rom_keys,
            rom_w,
            rom_gen,
            StrTable::new(&shown),
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn assemble(
        words: StrTable,
        freq: Vec<u32>,
        sentences: u64,
        tokens: u64,
        source: String,
        bi: (Vec<u32>, Vec<u32>, Vec<u32>),
        rom_keys: StrTable,
        rom_w: Vec<u32>,
        rom_gen: Vec<bool>,
        shown: StrTable,
        fst_bytes: Option<Vec<u8>>,
    ) -> Result<Model, Error> {
        let nw = words.len();
        let sum: u64 = freq.iter().map(|&f| f as u64).sum();
        let n = if sum == 0 { 1.0 } else { sum as f64 };
        let index = match fst_bytes {
            Some(b) => fst::Map::new(b).map_err(|e| Error::Format(e.to_string()))?,
            None => {
                // later duplicates win, like a JS Map built from [word, index] pairs
                let mut pairs: Vec<(&str, u64)> = (0..nw).map(|i| (words.get(i), i as u64)).collect();
                pairs.sort_by(|a, b| a.0.cmp(b.0));
                let mut b = fst::MapBuilder::memory();
                let mut i = 0;
                while i < pairs.len() {
                    let mut j = i;
                    while j + 1 < pairs.len() && pairs[j + 1].0 == pairs[i].0 {
                        j += 1;
                    }
                    b.insert(pairs[j].0, pairs[j].1).map_err(|e| Error::Format(e.to_string()))?;
                    i = j + 1;
                }
                fst::Map::new(b.into_inner().map_err(|e| Error::Format(e.to_string()))?)
                    .map_err(|e| Error::Format(e.to_string()))?
            }
        };
        let mut prior: HashMap<char, f64> = HashMap::new();
        let mut max_len = 1usize;
        let mut cl: Vec<Box<[u32]>> = Vec::with_capacity(FUZZY_WORDS.min(nw));
        for i in 0..nw {
            let w = words.get(i);
            if let Some(c) = w.chars().next() {
                *prior.entry(c).or_insert(0.0) += freq[i] as f64 / n;
            }
            if i < FUZZY_WORDS {
                let off: Vec<u32> = cluster_offsets(w).into_iter().map(|x| x as u32).collect();
                max_len = max_len.max(off.len() - 1);
                cl.push(off.into_boxed_slice());
            } else {
                max_len = max_len.max(clusters(w).len());
            }
        }
        let rom_top: Vec<u32> =
            (0..rom_w.len() as u32).filter(|&e| (rom_w[e as usize] as usize) < FUZZY_WORDS).collect();
        Ok(Model {
            words,
            freq,
            n,
            sentences,
            tokens,
            source,
            index,
            bi_off: bi.0,
            bi_ids: bi.1,
            bi_cnt: bi.2,
            rom_keys,
            rom_w,
            rom_gen,
            shown,
            rom_top,
            prior,
            max_len: max_len.min(24),
            cl,
        })
    }

    pub fn vocab(&self) -> usize {
        self.words.len()
    }
    pub fn word(&self, id: u32) -> &str {
        self.words.get(id as usize)
    }
    pub fn word_id(&self, w: &str) -> Option<u32> {
        self.index.get(w).map(|v| v as u32)
    }
    /// Fill `buf` with the clusters of word `id` (typo-scan window only); false when out of the window.
    /// The buffer is reused across calls so the scan does not allocate per word.
    pub fn fill_clusters<'a>(&'a self, id: u32, buf: &mut Vec<&'a str>) -> bool {
        let Some(off) = self.cl.get(id as usize) else { return false };
        let w = self.word(id);
        buf.clear();
        buf.extend(off.windows(2).map(|p| &w[p[0] as usize..p[1] as usize]));
        true
    }

    fn row_at(&self, slot: usize, total: f64) -> Option<Row<'_>> {
        let (a, b) = (self.bi_off[slot] as usize, self.bi_off[slot + 1] as usize);
        if a == b {
            return None;
        }
        Some(Row { ids: &self.bi_ids[a..b], cnt: &self.bi_cnt[a..b], total })
    }
    /// Bigram row for a context word; "^" is the start of a sentence.
    pub fn row(&self, prev: &str) -> Option<Row<'_>> {
        if prev == "^" {
            return self.row_at(self.vocab(), self.sentences as f64);
        }
        let id = self.word_id(prev)?;
        self.row_at(id as usize, self.freq[id as usize] as f64)
    }
    /// P(w | row): bigram mixed with unigram.
    pub fn prob(&self, w: u32, row: Option<&Row>) -> f64 {
        let uni = self.freq[w as usize] as f64 / self.n;
        match row {
            None => uni,
            Some(r) => {
                let lam = r.total / (r.total + 3.0);
                lam * (r.count(w) as f64 / r.total) + (1.0 - lam) * uni
            }
        }
    }

    /// Ids of all words that start with `p`, in sorted order.
    pub fn with_prefix(&self, p: &str) -> Vec<u32> {
        let mut out = Vec::new();
        let mut s = self.index.range().ge(p).into_stream();
        while let Some((k, v)) = s.next() {
            if !k.starts_with(p.as_bytes()) {
                break;
            }
            out.push(v as u32);
        }
        out
    }
    pub fn has_prefix(&self, p: &str) -> bool {
        let mut s = self.index.range().ge(p).into_stream();
        matches!(s.next(), Some((k, _)) if k.starts_with(p.as_bytes()))
    }
    /// First index in the sorted sound index whose key is >= p.
    pub fn lower_rom(&self, p: &str) -> usize {
        let (mut lo, mut hi) = (0, self.rom_keys.len());
        while lo < hi {
            let m = (lo + hi) / 2;
            if self.rom_keys.get(m) < p {
                lo = m + 1;
            } else {
                hi = m;
            }
        }
        lo
    }

    // ------------------------------------------------------------ compiled format
    /// Serialize to the compiled `khdata.bin` format (little endian, versioned).
    pub fn to_bytes(&self) -> Vec<u8> {
        fn put(out: &mut Vec<u8>, b: &[u8]) {
            out.extend_from_slice(&(b.len() as u64).to_le_bytes());
            out.extend_from_slice(b);
        }
        fn u32s(v: &[u32]) -> Vec<u8> {
            v.iter().flat_map(|x| x.to_le_bytes()).collect()
        }
        fn table(t: &StrTable) -> Vec<u8> {
            let mut b = (t.len() as u32).to_le_bytes().to_vec();
            b.extend(u32s(&t.off));
            b.extend_from_slice(t.blob.as_bytes());
            b
        }
        let meta = serde_json::json!({
            "sentences": self.sentences, "tokens": self.tokens, "source": self.source,
            "vocab": self.vocab(), "n": self.n
        });
        let mut out = MAGIC.to_vec();
        put(&mut out, meta.to_string().as_bytes());
        put(&mut out, &table(&self.words));
        put(&mut out, &u32s(&self.freq));
        put(&mut out, &u32s(&self.bi_off));
        put(&mut out, &u32s(&self.bi_ids));
        put(&mut out, &u32s(&self.bi_cnt));
        put(&mut out, &table(&self.rom_keys));
        put(&mut out, &u32s(&self.rom_w));
        put(&mut out, &self.rom_gen.iter().map(|&g| g as u8).collect::<Vec<_>>());
        put(&mut out, &table(&self.shown));
        put(&mut out, self.index.as_fst().as_bytes());
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Model, Error> {
        let bad = |m: &str| Error::Format(m.to_string());
        if bytes.len() < 8 || &bytes[..8] != MAGIC {
            return Err(bad("not a khdata file, or an unsupported version"));
        }
        let mut pos = 8usize;
        let mut next = || -> Result<&[u8], Error> {
            let len = bytes.get(pos..pos + 8).ok_or_else(|| bad("truncated"))?;
            let len = u64::from_le_bytes(len.try_into().unwrap()) as usize;
            pos += 8;
            let s = bytes
                .get(pos..pos.checked_add(len).ok_or_else(|| bad("truncated"))?)
                .ok_or_else(|| bad("truncated"))?;
            pos += len;
            Ok(s)
        };
        fn to_u32s(b: &[u8]) -> Result<Vec<u32>, Error> {
            if !b.len().is_multiple_of(4) {
                return Err(Error::Format("array length".into()));
            }
            Ok(b.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect())
        }
        fn to_table(b: &[u8]) -> Result<StrTable, Error> {
            let e = || Error::Format("string table".into());
            let n = u32::from_le_bytes(b.get(..4).ok_or_else(e)?.try_into().unwrap()) as usize;
            let end =
                4usize.checked_add(n.checked_add(1).and_then(|x| x.checked_mul(4)).ok_or_else(e)?).ok_or_else(e)?;
            let off = to_u32s(b.get(4..end).ok_or_else(e)?)?;
            let blob = std::str::from_utf8(&b[end..]).map_err(|_| e())?.to_string();
            let ok = off[0] == 0
                && off.windows(2).all(|w| w[0] <= w[1])
                && *off.last().unwrap() as usize == blob.len()
                && off.iter().all(|&o| blob.is_char_boundary(o as usize));
            if !ok {
                return Err(e());
            }
            Ok(StrTable { blob, off })
        }
        let meta: serde_json::Value = serde_json::from_slice(next()?).map_err(|e| Error::Format(e.to_string()))?;
        let words = to_table(next()?)?;
        let freq = to_u32s(next()?)?;
        let bi = (to_u32s(next()?)?, to_u32s(next()?)?, to_u32s(next()?)?);
        let rom_keys = to_table(next()?)?;
        let rom_w = to_u32s(next()?)?;
        let rom_gen: Vec<bool> = next()?.iter().map(|&b| b != 0).collect();
        let shown = to_table(next()?)?;
        let fst_bytes = next()?.to_vec();
        if freq.len() != words.len()
            || bi.0.len() != words.len() + 2
            || rom_w.len() != rom_keys.len()
            || rom_gen.len() != rom_w.len()
            || shown.len() != words.len()
        {
            return Err(bad("section sizes disagree"));
        }
        if bi.1.len() != bi.2.len()
            || bi.0[0] != 0
            || bi.0.windows(2).any(|w| w[0] > w[1])
            || bi.0.last().copied().unwrap_or(0) as usize != bi.1.len()
            || bi.1.iter().any(|&j| j as usize >= words.len())
            || rom_w.iter().any(|&w| w as usize >= words.len())
        {
            return Err(bad("index out of range"));
        }
        // every vocabulary word must be in the fst exactly once, pointing at its own position
        let map = fst::Map::new(fst_bytes.clone()).map_err(|e| Error::Format(e.to_string()))?;
        // the fst crate only checks its header on open; the checksum catches corrupted blobs before any lookup
        map.as_fst().verify().map_err(|e| Error::Format(e.to_string()))?;
        if map.len() > words.len()
            || (0..words.len()).any(|i| map.get(words.get(i)).is_none_or(|v| v as usize >= words.len()))
        {
            return Err(bad("word index disagrees with word list"));
        }
        Model::assemble(
            words,
            freq,
            meta["sentences"].as_u64().unwrap_or(1).max(1),
            meta["tokens"].as_u64().unwrap_or(0),
            meta["source"].as_str().unwrap_or("").to_string(),
            bi,
            rom_keys,
            rom_w,
            rom_gen,
            shown,
            Some(fst_bytes),
        )
    }
}
