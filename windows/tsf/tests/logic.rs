use khtsf::logic::*;

fn none() -> Mods {
    Mods::default()
}
fn shift() -> Mods {
    Mods { shift: true, ..Mods::default() }
}

#[test]
fn space_enter_tab() {
    assert_eq!(plan(Mode::Latin, VK_SPACE, 0x39, false, none()), Plan::Ask(Special::Space));
    assert_eq!(plan(Mode::English, VK_SPACE, 0x39, false, none()), Plan::Ask(Special::Space));
    // in Khmer mode a space is an ordinary space that only resets "just accepted"
    assert_eq!(plan(Mode::Khmer, VK_SPACE, 0x39, false, none()), Plan::ForgetAndPassThrough);
    assert_eq!(plan(Mode::Khmer, VK_TAB, 0x0F, false, none()), Plan::Ask(Special::Tab));
    assert_eq!(plan(Mode::Latin, VK_TAB, 0x0F, false, none()), Plan::Ask(Special::Tab));
    assert_eq!(plan(Mode::Khmer, VK_TAB, 0x0F, false, shift()), Plan::PassThrough, "Shift+Tab is the app's");
    assert_eq!(plan(Mode::Latin, VK_RETURN, 0x1C, false, none()), Plan::Ask(Special::Enter));
    assert_eq!(
        plan(Mode::Khmer, VK_RETURN, 0x1C, false, none()),
        Plan::ForgetAndPassThrough,
        "no engine call for Enter in Khmer mode"
    );
}

#[test]
fn shortcuts_are_left_alone() {
    let ctrl = Mods { ctrl: true, ..Mods::default() };
    let alt = Mods { alt: true, ..Mods::default() };
    let win = Mods { win: true, ..Mods::default() };
    for m in [Mode::Khmer, Mode::Latin, Mode::English] {
        assert_eq!(plan(m, 0x43 /* C */, 0x2E, false, ctrl), Plan::PassThrough);
        assert_eq!(plan(m, VK_TAB, 0x0F, false, alt), Plan::PassThrough, "Alt+Tab");
        assert_eq!(plan(m, VK_SPACE, 0x39, false, win), Plan::PassThrough);
        assert_eq!(plan(m, VK_PROCESSKEY, 0, false, none()), Plan::PassThrough);
    }
}

#[test]
fn mode_hot_key() {
    let m = Mods { ctrl: true, alt: true, ..Mods::default() };
    assert_eq!(plan(Mode::Khmer, VK_SPACE, 0x39, false, m), Plan::CycleMode);
    // AltGr + Space (Ctrl is synthesized by Windows) is not the hot key
    let altgr = Mods { ctrl: true, alt: true, right_alt: true, ..Mods::default() };
    assert_ne!(plan(Mode::Khmer, VK_SPACE, 0x39, false, altgr), Plan::CycleMode);
    assert_eq!(Mode::Khmer.next().next().next(), Mode::Khmer);
}

#[test]
fn khmer_mode_types_from_scan_codes() {
    // scan 0x25 is K, 0x30 is B, 0x05 is 4
    assert_eq!(plan(Mode::Khmer, 0x4B, 0x25, false, none()), Plan::Type { scan: 0x25, shift: false, altgr: false });
    assert_eq!(typed(0x25, false, false), Some("ក"));
    assert_eq!(typed(0x25, true, false), Some("គ"));
    assert_eq!(typed(0x30, false, false), Some("ប"));
    assert_eq!(typed(0x05, true, false), Some("៛"));
    // AltGr (Ctrl + right Alt) picks the third level; Ctrl does not count then
    let altgr = Mods { ctrl: true, alt: true, right_alt: true, ..Mods::default() };
    assert_eq!(
        plan(Mode::Khmer, 0x31 /* 1 */, 0x02, false, altgr),
        Plan::Type { scan: 0x02, shift: false, altgr: true }
    );
    assert_eq!(typed(0x02, false, true), Some("\u{200c}"));
    // the layout is by physical key: a numpad slash has the scan code of "/" but is an extended key
    assert!(matches!(plan(Mode::Khmer, 0x6F, 0x35, false, none()), Plan::Type { .. }));
    assert_eq!(plan(Mode::Khmer, 0x6F, 0x35, true, none()), Plan::ForgetAndPassThrough);
    // keys that are not in the layout, and Latin / English letters, are the app's
    assert_eq!(plan(Mode::Khmer, 0x70 /* F1 */, 0x3B, false, none()), Plan::ForgetAndPassThrough);
    assert_eq!(plan(Mode::Latin, 0x4B, 0x25, false, none()), Plan::ForgetAndPassThrough);
    assert_eq!(plan(Mode::English, 0x4B, 0x25, false, shift()), Plan::ForgetAndPassThrough);
}

#[test]
fn editing_and_cursor_keys_forget_the_accepted_word() {
    for vk in [VK_BACK, VK_DELETE, VK_ESCAPE, 0x25, 0x26, 0x27, 0x28, 0x24, 0x23, VK_PRIOR, 0x22] {
        assert_eq!(plan(Mode::Latin, vk, 0, true, none()), Plan::ForgetAndPassThrough, "vk {vk:#x}");
    }
}
