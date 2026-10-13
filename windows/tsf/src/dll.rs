//! DLL entry points: what Windows (COM, regsvr32) calls. Nothing here may panic out, or throw, into the caller.
use crate::guids::CLSID_KHMER_TIP;
use crate::tip::{guard, TextService, LIVE_OBJECTS};
use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, AtomicIsize, Ordering};
use windows::core::{implement, IUnknown, Interface, Ref, Result, BOOL, GUID, HRESULT};
use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_POINTER, HINSTANCE, S_FALSE, S_OK, TRUE,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
use windows::Win32::UI::TextServices::ITfTextInputProcessorEx;

static HINST: AtomicIsize = AtomicIsize::new(0);
static LOCKS: AtomicI32 = AtomicI32::new(0);

pub fn hinstance() -> HINSTANCE {
    HINSTANCE(HINST.load(Ordering::SeqCst) as *mut c_void)
}

#[no_mangle]
pub extern "system" fn DllMain(hinst: HINSTANCE, reason: u32, _reserved: *mut c_void) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        HINST.store(hinst.0 as isize, Ordering::SeqCst);
        unsafe {
            let _ = DisableThreadLibraryCalls(windows::Win32::Foundation::HMODULE(hinst.0));
        }
    }
    TRUE
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(&self, outer: Ref<IUnknown>, riid: *const GUID, ppv: *mut *mut c_void) -> Result<()> {
        guard(|| {
            if ppv.is_null() || riid.is_null() {
                return Err(E_POINTER.into());
            }
            unsafe { *ppv = std::ptr::null_mut() };
            if outer.is_some() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            let obj: ITfTextInputProcessorEx = TextService::new().into();
            unsafe { obj.query(riid, ppv).ok() }
        })
    }

    fn LockServer(&self, lock: BOOL) -> Result<()> {
        if lock.as_bool() {
            LOCKS.fetch_add(1, Ordering::SeqCst);
        } else {
            LOCKS.fetch_sub(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

fn hr(r: Result<()>) -> HRESULT {
    match r {
        Ok(()) => S_OK,
        Err(e) => e.code(),
    }
}

#[no_mangle]
pub extern "system" fn DllGetClassObject(rclsid: *const GUID, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    std::panic::catch_unwind(|| {
        if rclsid.is_null() || riid.is_null() || ppv.is_null() {
            return E_POINTER;
        }
        unsafe {
            *ppv = std::ptr::null_mut();
            if *rclsid != CLSID_KHMER_TIP {
                return CLASS_E_CLASSNOTAVAILABLE;
            }
            let factory: IClassFactory = Factory.into();
            factory.query(riid, ppv)
        }
    })
    .unwrap_or(E_FAIL)
}

#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if LIVE_OBJECTS.load(Ordering::SeqCst) == 0 && LOCKS.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

#[no_mangle]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    std::panic::catch_unwind(|| hr(crate::reg::register())).unwrap_or(E_FAIL)
}

#[no_mangle]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    std::panic::catch_unwind(|| hr(crate::reg::unregister())).unwrap_or(E_FAIL)
}
