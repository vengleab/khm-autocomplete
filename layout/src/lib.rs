//! The Khmer (NiDA) keyboard layout, so Khmer-script mode can type Khmer from physical keys on any platform.
//! The key table is generated from the XKB layout by `layout/tools/gen_layout.py`; shells only pass the key
//! (macOS virtual key code or Windows scan code) and the modifier state, and insert what comes back.
//!
//! Difference from NiDA: the Space key is not part of the layout. In NiDA it types a zero-width space; here the
//! shells let Space through as an ordinary space, because the engine reads a real space as "word finished".

pub struct Key {
    /// the character this key types on a US keyboard without Shift
    pub us: char,
    /// macOS virtual key code (ANSI)
    pub mac: u16,
    /// Windows set-1 scan code
    pub win: u16,
    /// normal, Shift, right Alt, Shift + right Alt ("" = not mapped)
    pub levels: [&'static str; 4],
}

include!("layout_table.rs");

/// How the key was identified by the platform.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyCode {
    /// macOS virtual key code (`NSEvent.keyCode`)
    Mac(u16),
    /// Windows scan code, set 1 (without the 0xE0 prefix)
    WindowsScan(u16),
    /// the character the key types on a US keyboard, without Shift
    Us(char),
}

pub fn key_for(code: KeyCode) -> Option<&'static Key> {
    KEYS.iter().find(|k| match code {
        KeyCode::Mac(c) => k.mac == c,
        KeyCode::WindowsScan(c) => k.win == c,
        KeyCode::Us(c) => k.us == c,
    })
}

/// What a key types in Khmer mode, or `None` when the key is not part of the layout (the shell lets it through).
pub fn map(code: KeyCode, shift: bool, altgr: bool) -> Option<&'static str> {
    let level = usize::from(shift) + 2 * usize::from(altgr);
    key_for(code).map(|k| k.levels[level]).filter(|s| !s.is_empty())
}

/// What the key that types `c` on a US keyboard types on the Khmer layout. Auto mode uses this to read ASCII that
/// was typed with the physical keys of a US keyboard: letters and unshifted symbols use the normal level, capitals
/// and shifted symbols the Shift level (`?` is Shift + the `/` key). `None` for anything that is not a
/// printable ASCII key, or whose key has nothing on that level.
pub fn from_ascii(c: char) -> Option<&'static str> {
    if !c.is_ascii_graphic() {
        return None;
    }
    if c.is_ascii_uppercase() {
        return map(KeyCode::Us(c.to_ascii_lowercase()), true, false);
    }
    match SHIFTED_US.iter().find(|(shifted, _)| *shifted == c) {
        Some(&(_, base)) => map(KeyCode::Us(base), true, false),
        None => map(KeyCode::Us(c), false, false),
    }
}

/// `from_ascii` for a whole run of keys, or `None` if any one of them has no Khmer reading.
pub fn read_ascii(run: &str) -> Option<String> {
    run.chars().map(from_ascii).collect()
}
