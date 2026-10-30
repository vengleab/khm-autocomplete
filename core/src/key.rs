//! Space / Enter / Tab rules (the reference's `commitLatin` and keydown handler), kept here so the
//! shells stay thin: they ask, then either insert the candidate or let the key through.
use crate::engine::{latin_suffix, Candidate, Engine, Kind, Mode};

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
                // Latin, English and Auto only; with no token or no candidate the space is left alone.
                // English converts only a confident match (the word or an inflection of it), never a half-typed word.
                // Auto converts only a confident top (tier < 1: exact Khmerlish, exact English, a NiDA reading made
                // of known words); prefixes, typos, rule guesses and the layout / raw chips leave the text as typed.
                if mode == Mode::Khmer {
                    return KeyAction::PassThrough;
                }
                let r = self.resolve(text_before_caret, mode, None);
                match (r.token.is_empty(), r.cands.first()) {
                    (false, Some(c)) if mode == Mode::Latin || c.tier.is_some_and(|t| t < 1.0) => {
                        KeyAction::Insert(self.shape(text_before_caret, &r, c))
                    }
                    _ => KeyAction::PassThrough,
                }
            }
            Key::Enter => {
                // with a pending Latin token Enter accepts the top candidate instead of inserting a newline
                if mode == Mode::Khmer || just_accepted.is_some() {
                    return KeyAction::PassThrough;
                }
                if mode == Mode::Auto {
                    // Enter takes a real candidate of a pending run, never a layout / raw chip
                    let r = self.resolve(text_before_caret, mode, None);
                    return match r.cands.first() {
                        Some(c) if !r.token.is_empty() && !matches!(c.kind, Kind::Layout | Kind::Raw) => {
                            KeyAction::Insert(self.shape(text_before_caret, &r, c))
                        }
                        _ => KeyAction::PassThrough,
                    };
                }
                if latin_suffix(text_before_caret).is_empty() {
                    return KeyAction::PassThrough;
                }
                let r = self.resolve(text_before_caret, mode, just_accepted);
                match r.cands.first() {
                    Some(c) => KeyAction::Insert(self.shape(text_before_caret, &r, c)),
                    None => KeyAction::PassThrough,
                }
            }
            Key::Tab => {
                // Tab takes the top candidate. With nothing but the layout / raw chips the top is the NiDA reading
                // (an explicit request to convert); a lone raw chip is the text as typed, so there is nothing to do.
                let r = self.resolve(text_before_caret, mode, just_accepted);
                match r.cands.first() {
                    Some(c) if c.kind != Kind::Raw => KeyAction::Insert(self.shape(text_before_caret, &r, c)),
                    _ => KeyAction::PassThrough,
                }
            }
        }
    }
}
