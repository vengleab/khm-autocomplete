//! Public engine API: text before the caret in, candidates out.
use crate::model::{Error, Model};
use crate::norm::normalize;
use crate::scorer::{NgramScorer, Scorer};
use crate::suggest::{self, Cand, Ctx};
use serde::Serialize;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Complete,
    Next,
    Common,
    Fuzzy,
    Exact,
    Prefix,
    Rule,
    /// Auto mode: the typed keys read as Khmer by the NiDA layout (offered when nothing else matched)
    Layout,
    /// Auto mode: the typed keys kept as they are, English (offered when nothing else matched)
    Raw,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// type Khmer script
    Khmer,
    /// type Khmer in Latin letters (Khmerlish)
    Latin,
    /// type English words, get Khmer suggestions
    English,
    /// the one Khmer state: reads the typed run as Khmerlish, English, NiDA layout keys or Khmer script
    Auto,
}

/// What a shell needs to show and insert one suggestion.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Candidate {
    pub word: String,
    pub kind: Kind,
    /// text that the word replaces (normalized form)
    pub replace: String,
    /// length of the span to replace, in UTF-16 units of the text the shell sent (which may be un-normalized)
    pub replace_len_utf16: usize,
    /// insert after the word before the caret instead of replacing it
    pub append: bool,
    pub prob: Option<f32>,
    /// Latin candidates: how the word is spelled in Latin ("by rule" for guesses)
    pub roman: Option<String>,
    pub why: Option<String>,
    /// Auto mode: which reading produced the candidate ("khmerlish", "english", "nida", "raw")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<&'static str>,
}

pub const LIMIT: usize = 8;

pub struct Engine {
    model: Model,
    scorer: Box<dyn Scorer>,
}

/// The trailing run of Latin letters, apostrophes and hyphens: the pending Khmerlish word.
pub fn latin_suffix(s: &str) -> &str {
    let start = s
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_alphabetic() || *c == '\'' || *c == '-')
        .last()
        .map_or(s.len(), |(i, _)| i);
    &s[start..]
}

/// The trailing run of ASCII graphic characters: the pending run in Auto mode (broader than `latin_suffix`,
/// since the NiDA layout also uses digits and punctuation keys).
pub fn ascii_run(s: &str) -> &str {
    let start = s.char_indices().rev().take_while(|(_, c)| c.is_ascii_graphic()).last().map_or(s.len(), |(i, _)| i);
    &s[start..]
}

pub(crate) struct Resolved {
    pub before: String,
    pub token: String,
    pub ctx: Ctx,
    pub cands: Vec<Cand>,
    pub just_accepted: bool,
}

impl Engine {
    pub fn from_model(model: Model) -> Engine {
        Engine { model, scorer: Box::new(NgramScorer) }
    }
    /// Load `data.json` + `lexicon.json`.
    pub fn load(data_path: &str, lexicon_path: &str) -> Result<Engine, Error> {
        let rd = |p: &str| std::fs::read_to_string(p).map_err(|e| Error::Io(format!("{p}: {e}")));
        Ok(Engine::from_model(Model::from_json(&rd(data_path)?, &rd(lexicon_path)?)?))
    }
    /// Load `data.json` + `lexicon.json` + `english.json` (the English -> Khmer glossary).
    pub fn load_with_english(data_path: &str, lexicon_path: &str, english_path: &str) -> Result<Engine, Error> {
        let rd = |p: &str| std::fs::read_to_string(p).map_err(|e| Error::Io(format!("{p}: {e}")));
        Ok(Engine::from_model(Model::from_json_with_english(
            &rd(data_path)?,
            &rd(lexicon_path)?,
            Some(&rd(english_path)?),
        )?))
    }
    /// Load the compiled `khdata.bin` made by `khbuild` (fast start).
    pub fn load_compiled(path: &str) -> Result<Engine, Error> {
        let b = std::fs::read(path).map_err(|e| Error::Io(format!("{path}: {e}")))?;
        Ok(Engine::from_model(Model::from_bytes(&b)?))
    }
    pub fn with_scorer(mut self, scorer: Box<dyn Scorer>) -> Engine {
        self.scorer = scorer;
        self
    }
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Segment normalized text before the caret (finished words, unfinished tail, previous word).
    pub fn analyze(&self, text: &str) -> Ctx {
        self.model.analyze(text)
    }
    /// Khmer-script candidates for a context, with full-precision scores.
    pub fn khmer_candidates(&self, ctx: &Ctx) -> Vec<Cand> {
        suggest::khmer_suggest(&self.model, self.scorer.as_ref(), ctx, LIMIT)
    }
    /// Auto candidates for a pending run after `prev` ("^" at sentence start): Khmerlish, English and NiDA readings merged.
    pub fn auto_candidates(&self, run: &str, prev: &str) -> Vec<Cand> {
        suggest::auto_suggest(&self.model, self.scorer.as_ref(), run, prev, LIMIT)
    }

    /// Latin candidates for a pending token after `prev` ("^" at sentence start).
    pub fn latin_candidates(&self, token: &str, prev: &str) -> Vec<Cand> {
        suggest::rom_suggest(&self.model, self.scorer.as_ref(), token, prev, LIMIT)
    }

    /// English -> Khmer candidates for a pending English token after `prev`; `roman` is the matched gloss.
    pub fn english_candidates(&self, token: &str, prev: &str) -> Vec<Cand> {
        suggest::english_suggest(&self.model, self.scorer.as_ref(), token, prev, LIMIT)
    }

    /// The reference's `refresh()`: decide what is pending and rank candidates.
    pub(crate) fn resolve(&self, text_before: &str, mode: Mode, just_accepted: Option<&str>) -> Resolved {
        let before = normalize(text_before);
        let token = match mode {
            _ if just_accepted.is_some() => String::new(),
            Mode::Khmer => String::new(),
            Mode::Auto => ascii_run(&before).to_string(),
            _ => latin_suffix(&before).to_string(),
        };
        if !token.is_empty() {
            let mut ctx = self.analyze(&before[..before.len() - token.len()]);
            if !ctx.partial.is_empty() {
                ctx.tokens.push(ctx.partial.clone());
                ctx.prev = std::mem::take(&mut ctx.partial);
            }
            let cands = match mode {
                Mode::English => self.english_candidates(&token, &ctx.prev),
                Mode::Auto => self.auto_candidates(&token, &ctx.prev),
                _ => self.latin_candidates(&token, &ctx.prev),
            };
            return Resolved { before, token, ctx, cands, just_accepted: false };
        }
        let ctx = match just_accepted {
            Some(w) => Ctx { tokens: self.analyze(&before).tokens, partial: String::new(), prev: w.to_string() },
            None => self.analyze(&before),
        };
        let cands = self.khmer_candidates(&ctx);
        Resolved { before, token, ctx, cands, just_accepted: just_accepted.is_some() }
    }

    pub(crate) fn shape(&self, raw: &str, r: &Resolved, c: &Cand) -> Candidate {
        let latin = !r.token.is_empty();
        let replace = if let Some(span) = &c.span {
            span.clone()
        } else if latin {
            r.token.clone()
        } else if c.append || r.just_accepted {
            String::new()
        } else {
            r.ctx.partial.clone()
        };
        let replace_len_utf16 =
            if latin && c.span.is_none() { r.token.len() } else { raw_suffix_len_utf16(raw, &r.before, &replace) };
        Candidate {
            word: c.word.clone(),
            kind: c.kind,
            replace,
            replace_len_utf16,
            append: c.append,
            prob: c.p.map(|p| p as f32),
            roman: c.roman.clone(),
            why: c.why.clone(),
            source: c.source,
        }
    }

    /// Candidates for the text before the caret. `just_accepted`: the word the user just accepted, as long
    /// as the caret has not moved and no key was typed since (then that word counts as finished).
    pub fn suggest(&self, text_before_caret: &str, mode: Mode, just_accepted: Option<&str>) -> Vec<Candidate> {
        let r = self.resolve(text_before_caret, mode, just_accepted);
        r.cands.iter().map(|c| self.shape(text_before_caret, &r, c)).collect()
    }

    /// Key presses a Khmer-script candidate saves: its code points, minus what was typed, minus one.
    /// Latin candidates (they carry a `roman` spelling) are not counted.
    pub fn keystroke_gain(c: &Candidate) -> usize {
        if c.roman.is_some() {
            return 0;
        }
        (c.word.chars().count() as isize - c.replace.chars().count() as isize - 1).max(0) as usize
    }
}

/// How many UTF-16 units at the end of `raw` correspond to `norm_suffix`, the tail of `normalize(raw)`.
/// Normalization can drop invisible characters and reorder marks, so the lengths may differ.
fn raw_suffix_len_utf16(raw: &str, norm: &str, norm_suffix: &str) -> usize {
    let u16len = |s: &str| s.encode_utf16().count();
    if norm_suffix.is_empty() {
        return 0;
    }
    if raw == norm {
        return u16len(norm_suffix);
    }
    let want = norm_suffix.chars().count();
    for (start, _) in raw.char_indices().rev() {
        let cand = &raw[start..];
        if normalize(cand) == norm_suffix {
            return u16len(cand);
        }
        if cand.chars().count() > want * 3 + 8 {
            break;
        }
    }
    u16len(norm_suffix)
}
