//! What a key press means, decided without touching Windows: pure logic, so it is tested on any machine.
//! The text service (`tip.rs`) reads the key state from Windows, calls `plan`, and carries the plan out.

/// The two states of the keyboard. In `Khmer` nothing is remapped: letters land in the document as typed and the
/// engine reads them (Khmerlish, English, keys of the Khmer layout, Khmer script). `English` is pure pass-through.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    English = 0,
    Khmer = 1,
}

/// The engine mode behind the Khmer state (`KH_MODE_AUTO`, the wire value of `khipc`).
pub const ENGINE_AUTO: u8 = 3;

impl Mode {
    /// The stored value: 0 is English; anything else is Khmer (so every state an earlier version stored reads as Khmer).
    pub fn from_u32(v: u32) -> Mode {
        if v == 0 {
            Mode::English
        } else {
            Mode::Khmer
        }
    }
    /// Ctrl + Alt + Space toggles between the two states.
    pub fn next(self) -> Mode {
        match self {
            Mode::Khmer => Mode::English,
            Mode::English => Mode::Khmer,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Mode::Khmer => "Khmer",
            Mode::English => "English (off)",
        }
    }
    /// The engine mode to ask for, or `None` in the English state: the engine is not called at all.
    pub fn engine_mode(self) -> Option<u8> {
        match self {
            Mode::Khmer => Some(ENGINE_AUTO),
            Mode::English => None,
        }
    }
}

/// Modifier state when the key went down.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    /// either Alt key
    pub alt: bool,
    /// the right Alt key (AltGr)
    pub right_alt: bool,
    pub win: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Special {
    Space = 0,
    Enter = 1,
    Tab = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plan {
    /// not ours: leave the key alone
    PassThrough,
    /// the app handles the key, but whatever was just accepted is no longer "just accepted"
    ForgetAndPassThrough,
    /// switch between the Khmer and English states (and swallow the key)
    CycleMode,
    /// ask the engine what to do with Space / Enter / Tab
    Ask(Special),
}

// virtual-key codes (winuser.h); kept here so this file has no dependency on Windows headers
pub const VK_BACK: u32 = 0x08;
pub const VK_TAB: u32 = 0x09;
pub const VK_RETURN: u32 = 0x0D;
pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_SPACE: u32 = 0x20;
pub const VK_PRIOR: u32 = 0x21;
pub const VK_DELETE: u32 = 0x2E;
pub const VK_PROCESSKEY: u32 = 0xE5;

/// `vk` is the virtual key. Keys are never remapped: in the Khmer state only Space, Enter and Tab are ours, and they
/// always go to the engine (which says whether there is anything to convert); every other key is the app's.
pub fn plan(mode: Mode, vk: u32, mods: Mods) -> Plan {
    // AltGr arrives as Ctrl + right Alt on many layouts; it is a character level, not a shortcut
    let altgr = mods.right_alt;
    let ctrl = mods.ctrl && !altgr;
    let alt = mods.alt && !altgr;

    if vk == VK_SPACE && ctrl && alt && !mods.win {
        return Plan::CycleMode; // Ctrl + Alt + Space
    }
    // English: the keyboard is off, every key is the app's
    if mode == Mode::English {
        return Plan::PassThrough;
    }
    // shortcuts belong to the app; IME-processed keys are not ours either
    if ctrl || alt || mods.win || vk == VK_PROCESSKEY {
        return Plan::PassThrough;
    }
    match vk {
        VK_TAB if !mods.shift => Plan::Ask(Special::Tab),
        VK_TAB => Plan::PassThrough,
        VK_RETURN => Plan::Ask(Special::Enter),
        VK_SPACE => Plan::Ask(Special::Space),
        VK_BACK | VK_ESCAPE | VK_DELETE => Plan::ForgetAndPassThrough,
        // letters, digits, punctuation, PageUp, PageDown, End, Home, arrows: the app inserts or moves; "just accepted" ends
        _ => Plan::ForgetAndPassThrough,
    }
}
