//! WebAssembly wrapper over `khcore`. Results are JSON strings, so the binding stays tiny and the JS side
//! just calls `JSON.parse`. The same engine code runs here, in the C API and in the platform shells.
use khcore::suggest::{Cand, Ctx};
use khcore::{Engine, Key, Mode, Model};
use serde::Serialize;
use wasm_bindgen::prelude::*;

fn err(e: impl ToString) -> JsError {
    JsError::new(&e.to_string())
}

fn mode_of(m: u8) -> Result<Mode, JsError> {
    match m {
        0 => Ok(Mode::Khmer),
        1 => Ok(Mode::Latin),
        2 => Ok(Mode::English),
        _ => Err(err(format!("unknown mode {m}"))),
    }
}

/// A candidate in the shape the demo UI uses (`rom` = Latin spelling, `p` = full-precision score).
#[derive(Serialize)]
struct JsCand<'a> {
    word: &'a str,
    kind: khcore::Kind,
    p: Option<f64>,
    append: bool,
    rom: Option<&'a str>,
    why: Option<&'a str>,
    tier: Option<f64>,
}

fn js_cands(c: &[Cand]) -> String {
    let v: Vec<JsCand> = c
        .iter()
        .map(|x| JsCand {
            word: &x.word,
            kind: x.kind,
            p: x.p,
            append: x.append,
            rom: x.roman.as_deref(),
            why: x.why.as_deref(),
            tier: x.tier,
        })
        .collect();
    serde_json::to_string(&v).unwrap()
}

#[wasm_bindgen]
pub struct WasmEngine {
    e: Engine,
}

#[wasm_bindgen]
impl WasmEngine {
    /// Build from the text of `data.json` and `lexicon.json`, and optionally `english.json` (English -> Khmer).
    #[wasm_bindgen(constructor)]
    pub fn new(data_json: &str, lexicon_json: &str, english_json: Option<String>) -> Result<WasmEngine, JsError> {
        let m = Model::from_json_with_english(data_json, lexicon_json, english_json.as_deref()).map_err(err)?;
        Ok(WasmEngine { e: Engine::from_model(m) })
    }

    /// Build from compiled `khdata.bin` bytes.
    #[wasm_bindgen(js_name = fromCompiled)]
    pub fn from_compiled(bytes: &[u8]) -> Result<WasmEngine, JsError> {
        Ok(WasmEngine { e: Engine::from_model(Model::from_bytes(bytes).map_err(err)?) })
    }

    pub fn vocab(&self) -> usize {
        self.e.model().vocab()
    }
    pub fn tokens(&self) -> f64 {
        self.e.model().tokens as f64
    }
    pub fn sentences(&self) -> f64 {
        self.e.model().sentences as f64
    }
    /// Sum of word frequencies (the reference's `N`).
    pub fn total(&self) -> f64 {
        self.e.model().n
    }
    pub fn source(&self) -> String {
        self.e.model().source.clone()
    }

    /// `{tokens, partial, prev}` for text that is already normalized.
    pub fn analyze(&self, text: &str) -> String {
        let c = self.e.analyze(text);
        serde_json::json!({"tokens": c.tokens, "partial": c.partial, "prev": c.prev}).to_string()
    }

    /// Khmer-script candidates after `prev` ("^" at sentence start) for the unfinished word `partial`.
    #[wasm_bindgen(js_name = khmerCandidates)]
    pub fn khmer_candidates(&self, prev: &str, partial: &str) -> String {
        js_cands(&self.e.khmer_candidates(&Ctx {
            tokens: vec![],
            partial: partial.to_string(),
            prev: prev.to_string(),
        }))
    }

    /// Latin candidates for a pending token after `prev`.
    #[wasm_bindgen(js_name = latinCandidates)]
    pub fn latin_candidates(&self, token: &str, prev: &str) -> String {
        js_cands(&self.e.latin_candidates(token, prev))
    }

    /// English -> Khmer candidates for a pending English token after `prev`; `rom` is the matched English gloss.
    #[wasm_bindgen(js_name = englishCandidates)]
    pub fn english_candidates(&self, token: &str, prev: &str) -> String {
        js_cands(&self.e.english_candidates(token, prev))
    }

    /// Rule-made spellings for a Latin token (`compose`), `[{word, why, score}]`.
    pub fn compose(&self, token: &str) -> String {
        let v: Vec<_> = khcore::compose::compose(token, &self.e.model().prior)
            .into_iter()
            .map(|c| serde_json::json!({"word": c.word, "why": c.why, "score": c.score}))
            .collect();
        serde_json::Value::Array(v).to_string()
    }

    /// The public API: candidates for the text before the caret. `mode`: 0 Khmer, 1 Latin, 2 English.
    pub fn suggest(&self, text_before_caret: &str, mode: u8, just_accepted: Option<String>) -> Result<String, JsError> {
        let c = self.e.suggest(text_before_caret, mode_of(mode)?, just_accepted.as_deref());
        Ok(khcore::api_json::candidates(&c))
    }

    /// Space (0) / Enter (1) / Tab (2): `{"action":"pass"}` or `{"action":"insert","candidate":{...}}`.
    #[wasm_bindgen(js_name = keyAction)]
    pub fn key_action(
        &self,
        text_before_caret: &str,
        mode: u8,
        key: u8,
        just_accepted: Option<String>,
    ) -> Result<String, JsError> {
        let key = match key {
            0 => Key::Space,
            1 => Key::Enter,
            2 => Key::Tab,
            k => return Err(err(format!("unknown key {k}"))),
        };
        Ok(khcore::api_json::key_action(&self.e.key_action(
            text_before_caret,
            mode_of(mode)?,
            key,
            just_accepted.as_deref(),
        )))
    }
}

/// Khmer normalization (NFC, invisible characters, mark order).
#[wasm_bindgen]
pub fn normalize(text: &str) -> String {
    khcore::norm::normalize(text)
}

/// Latin sound key.
#[wasm_bindgen(js_name = romKey)]
pub fn rom_key(text: &str) -> String {
    khcore::romkey::rom_key(text)
}

/// Rough Latin spelling of a Khmer word.
#[wasm_bindgen]
pub fn romanize(word: &str) -> String {
    khcore::romanize::romanize(word)
}
