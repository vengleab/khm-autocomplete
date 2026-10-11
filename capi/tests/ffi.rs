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
