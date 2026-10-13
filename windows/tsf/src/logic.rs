//! What a key press means, decided without touching Windows: pure logic, so it is tested on any machine.
//! The text service (`tip.rs`) reads the key state from Windows, calls `plan`, and carries the plan out.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Khmer = 0,
    Latin = 1,
    English = 2,
}

impl Mode {
    pub fn from_u32(v: u32) -> Mode {
        match v {
            1 => Mode::Latin,
            2 => Mode::English,
            _ => Mode::Khmer,
        }
    }
    pub fn next(self) -> Mode {
        match self {
            Mode::Khmer => Mode::Latin,
            Mode::Latin => Mode::English,
            Mode::English => Mode::Khmer,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Mode::Khmer => "Khmer script",
            Mode::Latin => "Latin (Khmerlish)",
            Mode::English => "English → Khmer",
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
    /// change input mode (and swallow the key)
    CycleMode,
    /// ask the engine what to do with Space / Enter / Tab
    Ask(Special),
    /// Khmer mode: type what this physical key types in the Khmer layout
    Type { scan: u16, shift: bool, altgr: bool },
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

/// `vk` is the virtual key, `scan` the set-1 scan code (bits 16..23 of lParam), `extended` bit 24 of lParam.
pub fn plan(mode: Mode, vk: u32, scan: u16, extended: bool, mods: Mods) -> Plan {
    // AltGr arrives as Ctrl + right Alt on many layouts; it is the Khmer third level, not a shortcut
    let altgr = mods.right_alt;
    let ctrl = mods.ctrl && !altgr;
    let alt = mods.alt && !altgr;

    if vk == VK_SPACE && ctrl && alt && !mods.win {
        return Plan::CycleMode; // Ctrl + Alt + Space
    }
    // shortcuts belong to the app; IME-processed keys are not ours either
    if ctrl || alt || mods.win || vk == VK_PROCESSKEY {
        return Plan::PassThrough;
    }
    match vk {
        VK_TAB if !mods.shift => return Plan::Ask(Special::Tab),
        VK_TAB => return Plan::PassThrough,
        VK_RETURN if mode != Mode::Khmer => return Plan::Ask(Special::Enter),
        VK_RETURN => return Plan::ForgetAndPassThrough,
        VK_SPACE if mode != Mode::Khmer => return Plan::Ask(Special::Space),
        // a plain space in Khmer mode finishes the word (the engine reads a real space that way)
        VK_SPACE => return Plan::ForgetAndPassThrough,
        VK_BACK | VK_ESCAPE | VK_DELETE => return Plan::ForgetAndPassThrough,
        // PageUp, PageDown, End, Home, arrows
        0x21..=0x28 => return Plan::ForgetAndPassThrough,
        _ => {}
    }
    if mode == Mode::Khmer
        && !extended
        && khlayout::map(khlayout::KeyCode::WindowsScan(scan), mods.shift, altgr).is_some()
    {
        return Plan::Type { scan, shift: mods.shift, altgr };
    }
    Plan::ForgetAndPassThrough
}

/// The Khmer text a `Plan::Type` key types.
pub fn typed(scan: u16, shift: bool, altgr: bool) -> Option<&'static str> {
    khlayout::map(khlayout::KeyCode::WindowsScan(scan), shift, altgr)
}
