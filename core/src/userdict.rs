//! Storage interface for an opt-in, on-device user dictionary that learns accepted words (not implemented).
//! Rules for any implementation: opt-in, never leaves the device, never logs typed text, easy to wipe.

pub trait UserDictionary: Send + Sync {
    /// The user accepted `word` after `prev` ("^" at the start of a sentence).
    fn record_accept(&mut self, prev: &str, word: &str);
    /// Extra weight (>= 0) for `word` after `prev`; 0 when nothing was learned.
    fn boost(&self, prev: &str, word: &str) -> f32;
    /// Forget everything.
    fn clear(&mut self);
}

/// The default: learns nothing.
#[derive(Default)]
pub struct NullUserDictionary;

impl UserDictionary for NullUserDictionary {
    fn record_accept(&mut self, _prev: &str, _word: &str) {}
    fn boost(&self, _prev: &str, _word: &str) -> f32 {
        0.0
    }
    fn clear(&mut self) {}
}
