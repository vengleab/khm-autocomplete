//! The JSON that crosses every boundary (C API, WebAssembly, the Windows engine pipe), shaped in one place.
use crate::{Candidate, Engine, KeyAction};
use serde::Serialize;

/// A candidate as the shells see it: the core's fields plus the key presses it saves.
#[derive(Serialize)]
struct Out<'a> {
    #[serde(flatten)]
    c: &'a Candidate,
    gain: usize,
}

fn out(c: &Candidate) -> Out<'_> {
    Out { c, gain: Engine::keystroke_gain(c) }
}

/// `[{"word","kind","replace","replace_len_utf16","append","prob","roman","why","gain"}, ...]`
pub fn candidates(list: &[Candidate]) -> String {
    serde_json::to_string(&list.iter().map(out).collect::<Vec<_>>()).expect("candidates serialize")
}

/// `{"action":"pass"}` or `{"action":"insert","candidate":{...}}`
pub fn key_action(a: &KeyAction) -> String {
    match a {
        KeyAction::PassThrough => serde_json::json!({"action": "pass"}),
        KeyAction::Insert(c) => serde_json::json!({"action": "insert", "candidate": out(c)}),
    }
    .to_string()
}
