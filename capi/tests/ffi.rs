//! Calls the C ABI exactly like a foreign caller would (raw pointers, C strings, JSON results).
use khcore_capi::*;
use serde_json::Value;
use std::ffi::{c_char, CStr, CString};

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}
unsafe fn take(p: *mut c_char) -> String {
    assert!(!p.is_null(), "call failed: {:?}", err());
    let s = CStr::from_ptr(p).to_str().unwrap().to_string();
    kh_string_free(p);
    s
}
fn err() -> Option<String> {
    let p = kh_last_error();
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
}
fn open() -> *mut KhEngine {
    let r = root();
    let (d, l) = (c(r.join("data/data.json").to_str().unwrap()), c(r.join("data/lexicon.json").to_str().unwrap()));
    let e = unsafe { kh_engine_open(d.as_ptr(), l.as_ptr()) };
    assert!(!e.is_null(), "{:?}", err());
    e
}

#[test]
fn layout_maps_physical_keys() {
    unsafe {
        let k = |kind, code, shift, altgr| {
            let p = kh_layout_map(kind, code, shift, altgr);
            (!p.is_null()).then(|| take(p))
        };
        assert_eq!(k(KH_KEYCODE_MAC, 40, 0, 0).as_deref(), Some("ក")); // K
        assert_eq!(k(KH_KEYCODE_MAC, 40, 1, 0).as_deref(), Some("គ")); // Shift+K
        assert_eq!(k(KH_KEYCODE_WINDOWS_SCAN, 0x30, 0, 0).as_deref(), Some("ប")); // B
        assert_eq!(k(KH_KEYCODE_WINDOWS_SCAN, 0x05, 1, 0).as_deref(), Some("៛")); // Shift+4
        assert_eq!(k(KH_KEYCODE_MAC, 49, 0, 0), None); // Space is not ours
        assert_eq!(k(99, 40, 0, 0), None);
        assert_eq!(k(KH_KEYCODE_MAC, 70000, 0, 0), None);
    }
}

#[test]
fn version_and_constants() {
    assert_eq!(kh_abi_version(), KH_ABI_VERSION);
}

#[test]
fn suggest_returns_json_candidates() {
    unsafe {
        let e = open();
        let v: Value =
            serde_json::from_str(&take(kh_suggest(e, c("ខ្ញុំ ស្រ").as_ptr(), KH_MODE_KHMER, std::ptr::null()))).unwrap();
        let a = v.as_array().unwrap();
        assert!(!a.is_empty() && a.len() <= 8);
        let first = &a[0];
        assert_eq!(first["word"], "ស្រលាញ់");
        assert_eq!(first["kind"], "complete");
        assert_eq!(first["replace"], "ស្រ");
        assert_eq!(first["replace_len_utf16"], 3); // ស + coeng + រ
        assert!(first["gain"].as_u64().unwrap() > 0);
        kh_engine_free(e);
    }
}

#[test]
fn key_action_space_converts_the_latin_word() {
    unsafe {
        let e = open();
        let v: Value = serde_json::from_str(&take(kh_key_action(
            e,
            c("knyom").as_ptr(),
            KH_MODE_LATIN,
            KH_KEY_SPACE,
            std::ptr::null(),
        )))
        .unwrap();
        assert_eq!(v["action"], "insert");
        assert_eq!(v["candidate"]["word"], "ខ្ញុំ");
        assert_eq!(v["candidate"]["replace_len_utf16"], 5);
        // no token: the space is left to the host
        let v: Value = serde_json::from_str(&take(kh_key_action(
            e,
            c("").as_ptr(),
            KH_MODE_LATIN,
            KH_KEY_SPACE,
            std::ptr::null(),
        )))
        .unwrap();
        assert_eq!(v["action"], "pass");
        // Khmer mode never takes Space
        let v: Value = serde_json::from_str(&take(kh_key_action(
            e,
            c("ខ្ញុំ").as_ptr(),
            KH_MODE_KHMER,
            KH_KEY_SPACE,
            std::ptr::null(),
        )))
        .unwrap();
        assert_eq!(v["action"], "pass");
        kh_engine_free(e);
    }
}

fn open_en() -> *mut KhEngine {
    let r = root();
    let (d, l, e) = (
        c(r.join("data/data.json").to_str().unwrap()),
        c(r.join("data/lexicon.json").to_str().unwrap()),
        c(r.join("data/english.json").to_str().unwrap()),
    );
    let e = unsafe { kh_engine_open_with_english(d.as_ptr(), l.as_ptr(), e.as_ptr()) };
    assert!(!e.is_null(), "{:?}", err());
    e
}

#[test]
fn english_mode_suggests_khmer_for_english_words() {
    unsafe {
        let e = open_en();
        let v: Value =
            serde_json::from_str(&take(kh_suggest(e, c("water").as_ptr(), KH_MODE_ENGLISH, std::ptr::null()))).unwrap();
        assert_eq!(v[0]["word"], "ទឹក");
        assert_eq!(v[0]["roman"], "water");
        assert_eq!(v[0]["replace_len_utf16"], 5);
        // Space converts a confident match, and swallows the space
        let a: Value = serde_json::from_str(&take(kh_key_action(
            e,
            c("water").as_ptr(),
            KH_MODE_ENGLISH,
            KH_KEY_SPACE,
            std::ptr::null(),
        )))
        .unwrap();
        assert_eq!(a["action"], "insert");
        assert_eq!(a["candidate"]["word"], "ទឹក");
        // ...but never a half-typed word: "wat" only prefix-matches, so the space is left alone
        let a: Value = serde_json::from_str(&take(kh_key_action(
            e,
            c("wat").as_ptr(),
            KH_MODE_ENGLISH,
            KH_KEY_SPACE,
            std::ptr::null(),
        )))
        .unwrap();
        assert_eq!(a["action"], "pass");
        // Tab takes the top candidate even then
        let a: Value = serde_json::from_str(&take(kh_key_action(
            e,
            c("wat").as_ptr(),
            KH_MODE_ENGLISH,
            KH_KEY_TAB,
            std::ptr::null(),
        )))
        .unwrap();
        assert_eq!(a["candidate"]["word"], "ទឹក");
        kh_engine_free(e);
        // an engine opened without a glossary has no English candidates (and does not fail)
        let e = open();
        let v: Value =
            serde_json::from_str(&take(kh_suggest(e, c("water").as_ptr(), KH_MODE_ENGLISH, std::ptr::null()))).unwrap();
        assert!(v.as_array().unwrap().is_empty());
        kh_engine_free(e);
    }
}

#[test]
fn just_accepted_is_honoured() {
    unsafe {
        let e = open();
        let v: Value =
            serde_json::from_str(&take(kh_suggest(e, c("ខ្ញុំ ចូលចិត្ត ញ៉ាំ").as_ptr(), KH_MODE_KHMER, c("ញ៉ាំ").as_ptr())))
                .unwrap();
        assert!(v.as_array().unwrap().iter().any(|x| x["word"] == "បាយ"));
        kh_engine_free(e);
    }
}

#[test]
fn bad_calls_fail_cleanly() {
    unsafe {
        // NULL engine, NULL text, bad mode, bad key, invalid UTF-8, missing file: NULL + a message, never a crash
        assert!(kh_suggest(std::ptr::null(), c("x").as_ptr(), 0, std::ptr::null()).is_null());
        assert!(err().unwrap().contains("engine"));
        let e = open();
        assert!(kh_suggest(e, std::ptr::null(), 0, std::ptr::null()).is_null());
        assert!(kh_suggest(e, c("x").as_ptr(), 9, std::ptr::null()).is_null());
        assert!(err().unwrap().contains("mode"));
        assert!(kh_key_action(e, c("x").as_ptr(), 0, 9, std::ptr::null()).is_null());
        let bad = [0xffu8, 0xfe, 0];
        assert!(kh_suggest(e, bad.as_ptr() as *const c_char, 0, std::ptr::null()).is_null());
        assert!(err().unwrap().contains("UTF-8"));
        // a successful call clears the previous error
        take(kh_suggest(e, c("x").as_ptr(), 0, std::ptr::null()));
        assert!(err().is_none());
        kh_engine_free(e);
        assert!(kh_engine_open(c("/no/such/file").as_ptr(), c("/no/such").as_ptr()).is_null());
        assert!(err().is_some());
        assert!(kh_engine_open_compiled_bytes(b"junk".as_ptr(), 4).is_null());
        assert!(kh_engine_open_compiled_bytes(std::ptr::null(), 0).is_null());
        kh_engine_free(std::ptr::null_mut());
        kh_string_free(std::ptr::null_mut());
    }
}

#[test]
fn compiled_bytes_round_trip() {
    unsafe {
        let r = root();
        let model = khcore::Model::from_json(
            &std::fs::read_to_string(r.join("data/data.json")).unwrap(),
            &std::fs::read_to_string(r.join("data/lexicon.json")).unwrap(),
        )
        .unwrap();
        let bytes = model.to_bytes();
        let e = kh_engine_open_compiled_bytes(bytes.as_ptr(), bytes.len());
        assert!(!e.is_null(), "{:?}", err());
        let v: Value =
            serde_json::from_str(&take(kh_suggest(e, c("knyom").as_ptr(), KH_MODE_LATIN, std::ptr::null()))).unwrap();
        assert_eq!(v[0]["word"], "ខ្ញុំ");
        kh_engine_free(e);
    }
}

#[test]
fn many_threads_share_one_engine() {
    unsafe {
        let e = open();
        let addr = e as usize;
        let hs: Vec<_> = (0..8)
            .map(|i| {
                std::thread::spawn(move || {
                    let e = addr as *const KhEngine;
                    for _ in 0..50 {
                        let t = if i % 2 == 0 { c("knyom") } else { c("ខ្ញុំ ស្រ") };
                        let m = if i % 2 == 0 { KH_MODE_LATIN } else { KH_MODE_KHMER };
                        let s = take(kh_suggest(e, t.as_ptr(), m, std::ptr::null()));
                        assert!(s.starts_with('['));
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        kh_engine_free(e);
    }
}

#[test]
fn auto_mode_reads_the_run_three_ways() {
    unsafe {
        let r = root();
        let (d, l, en) = (
            c(r.join("data/data.json").to_str().unwrap()),
            c(r.join("data/lexicon.json").to_str().unwrap()),
            c(r.join("data/english.json").to_str().unwrap()),
        );
        let e = kh_engine_open_with_english(d.as_ptr(), l.as_ptr(), en.as_ptr());
        assert!(!e.is_null(), "{:?}", err());
        assert_eq!(KH_MODE_AUTO, 3);
        let action = |text: &str, key| -> Value {
            serde_json::from_str(&take(kh_key_action(e, c(text).as_ptr(), KH_MODE_AUTO, key, std::ptr::null())))
                .unwrap()
        };
        // Khmerlish, an English word with punctuation before it, and keys of the Khmer layout
        let a = action("knhom", KH_KEY_SPACE);
        assert_eq!(
            (a["candidate"]["word"].as_str(), a["candidate"]["source"].as_str()),
            (Some("ខ្ញុំ"), Some("khmerlish"))
        );
        let a = action("hello,water", KH_KEY_SPACE);
        assert_eq!(a["candidate"]["word"], "ទឹក");
        assert_eq!(a["candidate"]["replace_len_utf16"], 5);
        let a = action("xjJuM", KH_KEY_SPACE);
        assert_eq!((a["candidate"]["word"].as_str(), a["candidate"]["source"].as_str()), (Some("ខ្ញុំ"), Some("nida")));
        // unknown text stays as typed, and the list ends in the layout and raw chips
        assert_eq!(action("windows", KH_KEY_SPACE)["action"], "pass");
        let v: Value =
            serde_json::from_str(&take(kh_suggest(e, c("windows").as_ptr(), KH_MODE_AUTO, std::ptr::null()))).unwrap();
        let kinds: Vec<&str> = v.as_array().unwrap().iter().map(|x| x["kind"].as_str().unwrap()).collect();
        assert_eq!(kinds, ["layout", "raw"]);
        kh_engine_free(e);
    }
}

#[test]
fn open_with_mapping_reads_the_charts_from_a_file() {
    unsafe {
        let r = root();
        let p = |f: &str| c(r.join(f).to_str().unwrap());
        let (d, l, en) = (p("data/data.json"), p("data/lexicon.json"), p("data/english.json"));
        let m = p("tests/golden/mapping_custom.json");
        let e = kh_engine_open_with_mapping(d.as_ptr(), l.as_ptr(), en.as_ptr(), m.as_ptr());
        assert!(!e.is_null(), "{:?}", err());
        // the custom charts write a final ប as B, so `leab` is a rule guess for លាប
        let v: Value =
            serde_json::from_str(&take(kh_suggest(e, c("leab").as_ptr(), KH_MODE_LATIN, std::ptr::null()))).unwrap();
        assert_eq!(v[0]["word"], "លាប");
        kh_engine_free(e);
        // a missing or invalid mapping file is an error with a message, not a crash
        let bad = p("data/data.json");
        assert!(kh_engine_open_with_mapping(d.as_ptr(), l.as_ptr(), en.as_ptr(), bad.as_ptr()).is_null());
        assert!(err().is_some());
        let none = c("/no/such/mapping.json");
        assert!(kh_engine_open_with_mapping(d.as_ptr(), l.as_ptr(), en.as_ptr(), none.as_ptr()).is_null());
    }
}
