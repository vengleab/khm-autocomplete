//! Registry: the user's state setting (Khmer / English), where this DLL lives, and COM / TSF registration (regsvr32 calls
//! `DllRegisterServer`, see dll.rs). Only settings are stored; typed text never is.
use crate::guids::*;
use std::path::PathBuf;
use windows::core::{Result, GUID, PCWSTR};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, HMODULE};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegDeleteValueW, RegGetValueW, RegSetValueExW, HKEY,
    HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_DWORD,
};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, ITfCategoryMgr, ITfInputProcessorProfileMgr,
    GUID_TFCAT_TIP_KEYBOARD,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn guid_string(g: &GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7]
    )
}

/// Path of this DLL.
pub fn dll_path() -> Option<PathBuf> {
    let mut buf = [0u16; 520];
    let n = unsafe { GetModuleFileNameW(Some(HMODULE(crate::dll::hinstance().0)), &mut buf) } as usize;
    (n > 0 && n < buf.len()).then(|| PathBuf::from(String::from_utf16_lossy(&buf[..n])))
}

/// The folder of this DLL; `khengine.exe` and `khdata.bin` are installed next to it.
pub fn dll_dir() -> Option<PathBuf> {
    dll_path().and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

fn read_dword(name: PCWSTR) -> Option<u32> {
    let key = wide(REG_SETTINGS_KEY);
    let mut v = 0u32;
    let mut len = 4u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            name,
            RRF_RT_REG_DWORD,
            None,
            Some((&mut v as *mut u32).cast()),
            Some(&mut len),
        )
    };
    r.is_ok().then_some(v)
}

/// The stored state: 0 English, 1 Khmer (`logic::Mode`). Nothing stored means Khmer.
/// Earlier versions stored one of three modes (0 Khmer, 1 Latin, 2 English) under "Mode"; all of them are the Khmer
/// state now, so that value is dropped (it would read as the wrong thing under the new numbering).
pub fn load_state() -> u32 {
    if let Some(v) = read_dword(windows::core::w!("State")) {
        return v;
    }
    if read_dword(windows::core::w!("Mode")).is_some() {
        unsafe {
            let key = wide(REG_SETTINGS_KEY);
            let mut h = HKEY::default();
            if RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut h,
                None,
            )
            .is_ok()
            {
                let _ = RegDeleteValueW(h, windows::core::w!("Mode"));
                let _ = RegCloseKey(h);
            }
        }
    }
    crate::logic::Mode::Khmer as u32
}

pub fn save_state(state: u32) {
    unsafe {
        let key = wide(REG_SETTINGS_KEY);
        let mut h = HKEY::default();
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut h,
            None,
        )
        .is_ok()
        {
            let _ = RegSetValueExW(h, windows::core::w!("State"), None, REG_DWORD, Some(&state.to_le_bytes()));
            let _ = RegCloseKey(h);
        }
    }
}

fn set_string(root: HKEY, subkey: &str, name: Option<&str>, value: &str) -> Result<()> {
    unsafe {
        let key = wide(subkey);
        let mut h = HKEY::default();
        RegCreateKeyExW(
            root,
            PCWSTR(key.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut h,
            None,
        )
        .ok()?;
        let v = wide(value);
        let bytes: Vec<u8> = v.iter().flat_map(|c| c.to_le_bytes()).collect();
        let n = name.map(wide);
        let r =
            RegSetValueExW(h, n.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr())), None, REG_SZ, Some(&bytes));
        let _ = RegCloseKey(h);
        r.ok()
    }
}

/// Register the COM class and the TSF profile. Needs administrator rights (regsvr32 from an elevated prompt).
pub fn register() -> Result<()> {
    let path = dll_path().ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
    let clsid = guid_string(&CLSID_KHMER_TIP);
    let base = format!("CLSID\\{clsid}");
    set_string(HKEY_CLASSES_ROOT, &base, None, DISPLAY_NAME)?;
    set_string(HKEY_CLASSES_ROOT, &format!("{base}\\InprocServer32"), None, &path.to_string_lossy())?;
    set_string(HKEY_CLASSES_ROOT, &format!("{base}\\InprocServer32"), Some("ThreadingModel"), "Apartment")?;
    unsafe {
        let profiles: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
        let desc: Vec<u16> = DISPLAY_NAME.encode_utf16().collect();
        profiles.RegisterProfile(
            &CLSID_KHMER_TIP,
            LANGID_KHMER,
            &GUID_KHMER_PROFILE,
            &desc,
            &[],
            0,
            HKL::default(),
            0,
            true,
            0,
        )?;
        let cats: ITfCategoryMgr = CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        // a keyboard text service. Deliberately not "secure mode": Windows then turns us off in password fields.
        cats.RegisterCategory(&CLSID_KHMER_TIP, &GUID_TFCAT_TIP_KEYBOARD, &CLSID_KHMER_TIP)?;
    }
    Ok(())
}

pub fn unregister() -> Result<()> {
    unsafe {
        if let Ok(cats) = CoCreateInstance::<_, ITfCategoryMgr>(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER) {
            let _ = cats.UnregisterCategory(&CLSID_KHMER_TIP, &GUID_TFCAT_TIP_KEYBOARD, &CLSID_KHMER_TIP);
        }
        if let Ok(profiles) = CoCreateInstance::<_, ITfInputProcessorProfileMgr>(
            &CLSID_TF_InputProcessorProfiles,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            let _ = profiles.UnregisterProfile(&CLSID_KHMER_TIP, LANGID_KHMER, &GUID_KHMER_PROFILE, 0);
        }
        let key = wide(&format!("CLSID\\{}", guid_string(&CLSID_KHMER_TIP)));
        let r = RegDeleteTreeW(HKEY_CLASSES_ROOT, PCWSTR(key.as_ptr()));
        if r != ERROR_FILE_NOT_FOUND {
            r.ok()?;
        }
    }
    Ok(())
}
