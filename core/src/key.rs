//! Space / Enter / Tab rules (the reference's `commitLatin` and keydown handler), kept here so the
//! shells stay thin: they ask, then either insert the candidate or let the key through.
use crate::engine::{latin_suffix, Candidate, Engine, Mode};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Space,
    Enter,
    Tab,
}

#[derive(Clone, Debug, PartialEq)]
pub enum KeyAction {
    /// not ours: let the host app handle the key
    PassThrough,
    /// swallow the key and replace `candidate.replace_len_utf16` units before the caret with `candidate.word`
    /// (no trailing space is added). Afterwards the shell remembers the word as `just_accepted`.
    Insert(Candidate),
}

impl Engine {
    /// `text_before_caret` is the text before the caret *without* the key being pressed.
    pub fn key_action(&self, text_before_caret: &str, mode: Mode, key: Key, just_accepted: Option<&str>) -> KeyAction {
        match key {
            Key::Space => {
                // Latin mode only; with no token or no candidate the space is left alone
                if mode != Mode::Latin {
                    return KeyAction::PassThrough;
                }
                let r = self.resolve(text_before_caret, Mode::Latin, None);
                match (r.token.is_empty(), r.cands.first()) {
                    (false, Some(c)) => KeyAction::Insert(self.shape(text_before_caret, &r, c)),
                    _ => KeyAction::PassThrough,
                }
            }
            Key::Enter => {
                // with a pending Latin token Enter accepts the top candidate instead of inserting a newline
                if mode != Mode::Latin || latin_suffix(text_before_caret).is_empty() || just_accepted.is_some() {
                    return KeyAction::PassThrough;
                }
                let r = self.resolve(text_before_caret, mode, just_accepted);
                match r.cands.first() {
                    Some(c) => KeyAction::Insert(self.shape(text_before_caret, &r, c)),
                    None => KeyAction::PassThrough,
                }
            }
            Key::Tab => {
                let r = self.resolve(text_before_caret, mode, just_accepted);
                match r.cands.first() {
                    Some(c) => KeyAction::Insert(self.shape(text_before_caret, &r, c)),
                    None => KeyAction::PassThrough,
                }
            }
        }
    }
}
