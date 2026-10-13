//! C ABI over `khcore`. All data crosses as UTF-8 C strings; results are JSON, so the ABI stays small.
//!
//! Rules for callers
//! - Every `char*` returned by this library must be released with `kh_string_free`.
//! - A function that can fail returns NULL; `kh_last_error()` then describes why (per thread, valid until
//!   the next call on that thread).
//! - An engine is immutable after opening: any number of threads may call `kh_suggest` / `kh_key_action`
//!   on the same engine at once. `kh_engine_free` must not race with other calls.
//! - Text passed in must be valid UTF-8. Lengths in results (`replace_len_utf16`) count UTF-16 code units
//!   of the text the caller sent, which is what Cocoa (`NSRange`) and Windows (TSF) both use.
//! - No panic crosses the boundary; no network; typed text is never logged or stored.
#![allow(clippy::missing_safety_doc)]

use khcore::{Engine, Key, Mode};
use std::cell::RefCell;
use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Version of this C interface. Bumped on any incompatible change.
pub const KH_ABI_VERSION: u32 = 1;

/// Input mode: Khmer script.
pub const KH_MODE_KHMER: i32 = 0;
/// Input mode: Latin letters (Khmerlish).
pub const KH_MODE_LATIN: i32 = 1;

/// Input mode: English words, Khmer suggestions (needs an engine opened with an English glossary).
pub const KH_MODE_ENGLISH: i32 = 2;

/// Key: Space.
pub const KH_KEY_SPACE: i32 = 0;
/// Key: Enter / Return.
pub const KH_KEY_ENTER: i32 = 1;
/// Key: Tab.
pub const KH_KEY_TAB: i32 = 2;

/// Key code kind: macOS virtual key code (`NSEvent.keyCode`).
pub const KH_KEYCODE_MAC: i32 = 0;
/// Key code kind: Windows scan code, set 1, without the 0xE0 prefix.
pub const KH_KEYCODE_WINDOWS_SCAN: i32 = 1;

/// An opened engine (opaque).
pub struct KhEngine(Engine);

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_error(msg: impl AsRef<str>) {
    let c = CString::new(msg.as_ref().replace('\0', " ")).unwrap_or_default();
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(c));
}

fn clear_error() {
    LAST_ERROR.with(|e| *e.borrow_mut() = None);
}

/// Run `f`, turning panics and `Err` into NULL + last error.
fn guard<T>(f: impl FnOnce() -> Result<*mut T, String>) -> *mut T {
    clear_error();
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(p)) => p,
        Ok(Err(m)) => {
            set_error(m);
            std::ptr::null_mut()
        }
        Err(_) => {
            set_error("internal error (panic caught at the C boundary)");
            std::ptr::null_mut()
        }
    }
}

unsafe fn str_arg<'a>(p: *const c_char, what: &str) -> Result<&'a str, String> {
    if p.is_null() {
        return Err(format!("{what} is NULL"));
    }
    CStr::from_ptr(p).to_str().map_err(|_| format!("{what} is not valid UTF-8"))
}

unsafe fn opt_str_arg<'a>(p: *const c_char, what: &str) -> Result<Option<&'a str>, String> {
    if p.is_null() {
        Ok(None)
    } else {
        str_arg(p, what).map(Some)
    }
}

fn mode_of(m: i32) -> Result<Mode, String> {
    match m {
        KH_MODE_KHMER => Ok(Mode::Khmer),
        KH_MODE_LATIN => Ok(Mode::Latin),
        KH_MODE_ENGLISH => Ok(Mode::English),
        _ => Err(format!("unknown mode {m}")),
    }
}

fn to_c(s: String) -> Result<*mut c_char, String> {
    CString::new(s).map(CString::into_raw).map_err(|_| "result contained a NUL byte".to_string())
}

fn engine_ptr(e: Result<Engine, khcore::model::Error>) -> Result<*mut KhEngine, String> {
    e.map(|e| Box::into_raw(Box::new(KhEngine(e)))).map_err(|e| e.to_string())
}

/// The interface version this library was built with (compare with `KH_ABI_VERSION` from the header).
#[no_mangle]
pub extern "C" fn kh_abi_version() -> u32 {
    KH_ABI_VERSION
}

/// Open an engine from `data.json` and `lexicon.json` (slow start; use the compiled format in products).
/// Returns NULL on failure.
#[no_mangle]
pub unsafe extern "C" fn kh_engine_open(data_path: *const c_char, lexicon_path: *const c_char) -> *mut KhEngine {
    guard(|| engine_ptr(Engine::load(str_arg(data_path, "data_path")?, str_arg(lexicon_path, "lexicon_path")?)))
}

/// Like `kh_engine_open`, plus `english.json` (Khmer word -> English glosses) for `KH_MODE_ENGLISH`.
/// Returns NULL on failure.
#[no_mangle]
pub unsafe extern "C" fn kh_engine_open_with_english(
    data_path: *const c_char,
    lexicon_path: *const c_char,
    english_path: *const c_char,
) -> *mut KhEngine {
    guard(|| {
        engine_ptr(Engine::load_with_english(
            str_arg(data_path, "data_path")?,
            str_arg(lexicon_path, "lexicon_path")?,
            str_arg(english_path, "english_path")?,
        ))
    })
}

/// Open an engine from a compiled `khdata.bin` made by `khbuild`. Returns NULL on failure.
#[no_mangle]
pub unsafe extern "C" fn kh_engine_open_compiled(path: *const c_char) -> *mut KhEngine {
    guard(|| engine_ptr(Engine::load_compiled(str_arg(path, "path")?)))
}

/// Open an engine from compiled data already in memory (the bytes are copied). Returns NULL on failure.
#[no_mangle]
pub unsafe extern "C" fn kh_engine_open_compiled_bytes(bytes: *const u8, len: usize) -> *mut KhEngine {
    guard(|| {
        if bytes.is_null() {
            return Err("bytes is NULL".into());
        }
        engine_ptr(khcore::Model::from_bytes(std::slice::from_raw_parts(bytes, len)).map(Engine::from_model))
    })
}

/// Release an engine. NULL is allowed.
#[no_mangle]
pub unsafe extern "C" fn kh_engine_free(engine: *mut KhEngine) {
    if !engine.is_null() {
        drop(Box::from_raw(engine));
    }
}

/// Candidates for the text before the caret, as a JSON array (best first, at most 8):
/// `[{"word","kind","replace","replace_len_utf16","append","prob","roman","why","gain"}, ...]`.
/// In Latin and English modes `roman` is the Latin spelling / the matched English gloss.
/// `mode` is `KH_MODE_*`. `just_accepted` is the word the user just accepted, or NULL (see Engine::suggest).
#[no_mangle]
pub unsafe extern "C" fn kh_suggest(
    engine: *const KhEngine,
    text_before_caret: *const c_char,
    mode: i32,
    just_accepted: *const c_char,
) -> *mut c_char {
    guard(|| {
        let e = engine.as_ref().ok_or("engine is NULL")?;
        let text = str_arg(text_before_caret, "text_before_caret")?;
        let c = e.0.suggest(text, mode_of(mode)?, opt_str_arg(just_accepted, "just_accepted")?);
        to_c(khcore::api_json::candidates(&c))
    })
}

/// What to do with a Space / Enter / Tab key press. `text_before_caret` excludes the key being pressed.
/// Returns `{"action":"pass"}` (let the host app handle the key) or
/// `{"action":"insert","candidate":{...}}` (swallow the key, replace `replace_len_utf16` UTF-16 units before
/// the caret with `candidate.word`, add nothing else; then pass the word as `just_accepted` until the next key).
#[no_mangle]
pub unsafe extern "C" fn kh_key_action(
    engine: *const KhEngine,
    text_before_caret: *const c_char,
    mode: i32,
    key: i32,
    just_accepted: *const c_char,
) -> *mut c_char {
    guard(|| {
        let e = engine.as_ref().ok_or("engine is NULL")?;
        let text = str_arg(text_before_caret, "text_before_caret")?;
        let key = match key {
            KH_KEY_SPACE => Key::Space,
            KH_KEY_ENTER => Key::Enter,
            KH_KEY_TAB => Key::Tab,
            k => return Err(format!("unknown key {k}")),
        };
        let action = e.0.key_action(text, mode_of(mode)?, key, opt_str_arg(just_accepted, "just_accepted")?);
        to_c(khcore::api_json::key_action(&action))
    })
}

/// What a physical key types in Khmer-script mode (Khmer NiDA layout), as a UTF-8 string, or NULL when the key
/// is not part of the layout (not an error: let the key through). `kind` is `KH_KEYCODE_*`; `shift` and `altgr`
/// (right Alt on Windows, Option on macOS) are 0 or 1. Space is not part of the layout; see core/src/layout.rs.
#[no_mangle]
pub extern "C" fn kh_layout_map(kind: i32, code: u32, shift: i32, altgr: i32) -> *mut c_char {
    clear_error();
    let code = match (kind, u16::try_from(code)) {
        (KH_KEYCODE_MAC, Ok(c)) => khcore::layout::KeyCode::Mac(c),
        (KH_KEYCODE_WINDOWS_SCAN, Ok(c)) => khcore::layout::KeyCode::WindowsScan(c),
        _ => return std::ptr::null_mut(),
    };
    match khcore::layout::map(code, shift != 0, altgr != 0) {
        Some(s) => CString::new(s).map_or(std::ptr::null_mut(), CString::into_raw),
        None => std::ptr::null_mut(),
    }
}

/// Release a string returned by this library. NULL is allowed.
#[no_mangle]
pub unsafe extern "C" fn kh_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// Description of the last failure on this thread, or NULL. Do not free; valid until the next call on this thread.
#[no_mangle]
pub extern "C" fn kh_last_error() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ref().map_or(std::ptr::null(), |c| c.as_ptr()))
}
