//! The Khmer layout table: spot checks against facts from the layout, and structural properties.
mod common;
use khcore::layout::{from_ascii, key_for, map, read_ascii, KeyCode, KEYS, SHIFTED_US};
use std::collections::HashSet;

fn us(c: char, shift: bool) -> Option<&'static str> {
    map(KeyCode::Us(c), shift, false)
}

#[test]
fn spot_checks() {
    // facts from the NiDA layout: ប is on B, ៛ on Shift+4, coeng on J, digits are Khmer digits
    assert_eq!(us('b', false), Some("ប"));
    assert_eq!(us('4', true), Some("៛"));
    assert_eq!(us('j', false), Some("្"));
    assert_eq!(us('k', false), Some("ក"));
    assert_eq!(us('k', true), Some("គ"));
    assert_eq!(us('a', false), Some("ា"));
    assert_eq!(us('m', true), Some("ំ"));
    assert_eq!(us('/', true), Some("?"));
    assert_eq!(us('1', false), Some("១"));
    assert_eq!(us('0', false), Some("០"));
    assert_eq!(us('c', true), Some("ជ"));
    // AltGr level
    assert_eq!(map(KeyCode::Us('1'), false, true), Some("\u{200c}"));
    assert_eq!(map(KeyCode::Us('s'), false, true), None);
    // unknown keys are not ours
    assert_eq!(map(KeyCode::Mac(36), false, false), None); // Return
    assert_eq!(map(KeyCode::Mac(49), false, false), None); // Space
    assert_eq!(map(KeyCode::WindowsScan(0x39), false, false), None); // Space
}

#[test]
fn platform_codes_agree_with_us_characters() {
    // macOS ANSI virtual key codes and Windows scan codes for letters, taken from the platforms' documentation
    let mac = [
        ('a', 0),
        ('s', 1),
        ('d', 2),
        ('f', 3),
        ('h', 4),
        ('g', 5),
        ('z', 6),
        ('x', 7),
        ('c', 8),
        ('v', 9),
        ('b', 11),
        ('q', 12),
        ('w', 13),
        ('e', 14),
        ('r', 15),
        ('y', 16),
        ('t', 17),
        ('o', 31),
        ('u', 32),
        ('i', 34),
        ('p', 35),
        ('l', 37),
        ('j', 38),
        ('k', 40),
        ('n', 45),
        ('m', 46),
    ];
    for (c, code) in mac {
        assert_eq!(key_for(KeyCode::Mac(code)).unwrap().us, c, "mac {code}");
    }
    let win = [
        ('q', 0x10),
        ('a', 0x1E),
        ('z', 0x2C),
        ('m', 0x32),
        ('1', 0x02),
        ('0', 0x0B),
        ('`', 0x29),
        ('\\', 0x2B),
        ('/', 0x35),
    ];
    for (c, code) in win {
        assert_eq!(key_for(KeyCode::WindowsScan(code)).unwrap().us, c, "win {code:#x}");
    }
}

#[test]
fn table_is_well_formed() {
    assert_eq!(KEYS.len(), 47); // 13 + 12 + 12 + 10 keys
    let (mut us, mut mac, mut win) = (HashSet::new(), HashSet::new(), HashSet::new());
    for k in KEYS.iter() {
        assert!(us.insert(k.us) && mac.insert(k.mac) && win.insert(k.win), "duplicate key {:?}", k.us);
        assert!(!k.levels[0].is_empty() && !k.levels[1].is_empty(), "normal and Shift must be mapped for {:?}", k.us);
    }
    // within one level no two keys type the same thing (otherwise a character would be ambiguous)
    for level in 0..2 {
        let mut seen = HashSet::new();
        for k in KEYS.iter() {
            assert!(seen.insert(k.levels[level]), "level {level}: {:?} typed twice", k.levels[level]);
        }
    }
    // every Khmer consonant (U+1780..=U+17A2) can be typed
    let typed: HashSet<char> = KEYS.iter().flat_map(|k| k.levels).flat_map(|s| s.chars()).collect();
    for c in '\u{1780}'..='\u{17A2}' {
        assert!(typed.contains(&c), "consonant {c} is not on the keyboard");
    }
}

#[test]
fn from_ascii_reads_us_keys() {
    // letters and unshifted symbols: the normal level; capitals and shifted symbols: the Shift level
    assert_eq!(from_ascii('k'), Some("ក"));
    assert_eq!(from_ascii('K'), Some("គ"));
    assert_eq!(from_ascii('1'), Some("១"));
    assert_eq!(from_ascii('!'), Some("!")); // Shift+1 on the Khmer layout is "!" itself
    assert_eq!(from_ascii('@'), Some("ៗ")); // Shift+2
    assert_eq!(from_ascii('.'), Some("។"));
    assert_eq!(from_ascii('>'), Some("៕")); // Shift+.
    assert_eq!(from_ascii('?'), Some("?")); // Shift+/
    assert_eq!(from_ascii('"'), Some("៉")); // Shift+'
    assert_eq!(from_ascii('#'), Some("\"")); // Shift+3
    assert_eq!(from_ascii('J'), Some("ញ"));
    // not a key: whitespace, control and non-ASCII characters
    assert_eq!(from_ascii(' '), None);
    assert_eq!(from_ascii('\n'), None);
    assert_eq!(from_ascii('ក'), None);
    assert_eq!(read_ascii("xjJuM").as_deref(), Some("ខ្ញុំ"));
    assert_eq!(read_ascii("a b"), None);
    // every Shift pair names a key of the table, and every printable ASCII character has a reading
    for (shifted, base) in SHIFTED_US {
        assert!(key_for(KeyCode::Us(base)).is_some(), "{shifted} -> {base}");
    }
    for c in '!'..='~' {
        assert!(from_ascii(c).is_some(), "{c} has no reading");
    }
}

/// The NIDA constant in web/template.html (what the JS reference reads) is the Rust table.
/// Regenerate it with `cargo run -q -p khlayout --example js_table` and run `node tests/golden/gen.mjs`.
#[test]
fn js_nida_table_equals_the_layout_crate() {
    let g = common::golden("tables.json");
    let js = g["nida"].as_object().expect("tables.json has no nida table: run node tests/golden/gen.mjs");
    for c in '!'..='~' {
        let want = from_ascii(c);
        let got = js.get(&c.to_string()).map(|v| v.as_str().unwrap());
        assert_eq!(got, want, "NIDA[{c:?}] in the JS reference differs from khlayout::from_ascii");
    }
    assert_eq!(js.len(), ('!'..='~').filter(|c| from_ascii(*c).is_some()).count());
}
