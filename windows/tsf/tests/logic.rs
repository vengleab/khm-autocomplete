use khtsf::logic::*;

fn none() -> Mods {
    Mods::default()
}
fn shift() -> Mods {
    Mods { shift: true, ..Mods::default() }
}

#[test]
fn space_enter_tab_always_go_to_the_engine_in_the_khmer_state() {
    assert_eq!(plan(Mode::Khmer, VK_SPACE, none()), Plan::Ask(Special::Space));
    assert_eq!(plan(Mode::Khmer, VK_RETURN, none()), Plan::Ask(Special::Enter));
    assert_eq!(plan(Mode::Khmer, VK_TAB, none()), Plan::Ask(Special::Tab));
    assert_eq!(plan(Mode::Khmer, VK_TAB, shift()), Plan::PassThrough, "Shift+Tab is the app's");
}

#[test]
fn english_is_pure_pass_through() {
    for vk in [VK_SPACE, VK_RETURN, VK_TAB, VK_BACK, 0x4B, 0x25, 0x70] {
        assert_eq!(plan(Mode::English, vk, none()), Plan::PassThrough, "vk {vk:#x}");
    }
    assert_eq!(Mode::English.engine_mode(), None, "the engine is not called in the English state");
}

#[test]
fn shortcuts_are_left_alone() {
    let ctrl = Mods { ctrl: true, ..Mods::default() };
    let alt = Mods { alt: true, ..Mods::default() };
    let win = Mods { win: true, ..Mods::default() };
    for m in [Mode::Khmer, Mode::English] {
        assert_eq!(plan(m, 0x43 /* C */, ctrl), Plan::PassThrough);
        assert_eq!(plan(m, VK_TAB, alt), Plan::PassThrough, "Alt+Tab");
        assert_eq!(plan(m, VK_SPACE, win), Plan::PassThrough);
        assert_eq!(plan(m, VK_PROCESSKEY, none()), Plan::PassThrough);
    }
}

#[test]
fn hot_key_toggles_the_two_states() {
    let m = Mods { ctrl: true, alt: true, ..Mods::default() };
    assert_eq!(plan(Mode::Khmer, VK_SPACE, m), Plan::CycleMode);
    assert_eq!(plan(Mode::English, VK_SPACE, m), Plan::CycleMode, "the hot key works in the English state too");
    // AltGr + Space (Ctrl is synthesized by Windows) is not the hot key
    let altgr = Mods { ctrl: true, alt: true, right_alt: true, ..Mods::default() };
    assert_ne!(plan(Mode::Khmer, VK_SPACE, altgr), Plan::CycleMode);
    assert_eq!(Mode::Khmer.next(), Mode::English);
    assert_eq!(Mode::Khmer.next().next(), Mode::Khmer);
}

#[test]
fn letters_are_never_remapped() {
    // K (scan 0x25), a digit, a numpad slash, F1: all the app's, in either state
    for vk in [0x4B, 0x31, 0x6F, 0x70] {
        assert_eq!(plan(Mode::Khmer, vk, none()), Plan::ForgetAndPassThrough, "vk {vk:#x}");
        assert_eq!(plan(Mode::Khmer, vk, shift()), Plan::ForgetAndPassThrough, "vk {vk:#x}");
    }
}

#[test]
fn stored_values_migrate_to_the_khmer_state() {
    // an earlier version stored 0 Khmer, 1 Latin, 2 English; now 0 is English and everything else is Khmer
    assert_eq!(Mode::from_u32(0), Mode::English);
    for v in [1, 2, 3, 99] {
        assert_eq!(Mode::from_u32(v), Mode::Khmer);
    }
    assert_eq!(Mode::Khmer.engine_mode(), Some(ENGINE_AUTO));
    assert_eq!(Mode::from_u32(Mode::Khmer as u32), Mode::Khmer);
    assert_eq!(Mode::from_u32(Mode::English as u32), Mode::English);
}

#[test]
fn editing_and_cursor_keys_forget_the_accepted_word() {
    for vk in [VK_BACK, VK_DELETE, VK_ESCAPE, 0x25, 0x26, 0x27, 0x28, 0x24, 0x23, VK_PRIOR, 0x22] {
        assert_eq!(plan(Mode::Khmer, vk, none()), Plan::ForgetAndPassThrough, "vk {vk:#x}");
    }
}
