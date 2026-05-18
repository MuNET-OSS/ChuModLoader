#![allow(non_snake_case, non_camel_case_types)]

mod config;
mod d3d9_wrapper;
mod device_wrapper;
mod mod_api;
mod overlay;

use std::ffi::c_void;
use std::ptr;
use windows_sys::Win32::Foundation::{BOOL, HMODULE, TRUE};
use windows_sys::Win32::System::LibraryLoader::{DisableThreadLibraryCalls, GetProcAddress, LoadLibraryA};

const DLL_PROCESS_ATTACH: u32 = 1;

static mut REAL_D3D9: HMODULE = ptr::null_mut();
static mut REAL_DIRECT3D_CREATE9: usize = 0;
static mut REAL_DIRECT3D_CREATE9_EX: usize = 0;

type Direct3DCreate9Fn = unsafe extern "system" fn(u32) -> *mut c_void;
type Direct3DCreate9ExFn = unsafe extern "system" fn(u32, *mut *mut c_void) -> i32;

unsafe fn load_real_d3d9() -> bool {
    let sys_dir = std::env::var("SYSTEMDRIVE").unwrap_or("C:".to_string());
    let path = format!("{}\\Windows\\SysWOW64\\d3d9.dll\0", sys_dir);
    REAL_D3D9 = LoadLibraryA(path.as_ptr());
    if REAL_D3D9.is_null() {
        return false;
    }
    let proc = GetProcAddress(REAL_D3D9, c"Direct3DCreate9".as_ptr().cast());
    REAL_DIRECT3D_CREATE9 = proc.map_or(0, |f| f as usize);

    let proc_ex = GetProcAddress(REAL_D3D9, c"Direct3DCreate9Ex".as_ptr().cast());
    REAL_DIRECT3D_CREATE9_EX = proc_ex.map_or(0, |f| f as usize);

    REAL_DIRECT3D_CREATE9 != 0
}

/// # Safety
/// Must only be called after `load_real_d3d9` has succeeded.
#[no_mangle]
pub unsafe extern "system" fn Direct3DCreate9(sdk_version: u32) -> *mut c_void {
    if REAL_DIRECT3D_CREATE9 == 0 {
        return ptr::null_mut();
    }

    let real_create: Direct3DCreate9Fn = std::mem::transmute(REAL_DIRECT3D_CREATE9);
    let real_d3d9 = real_create(sdk_version);
    if real_d3d9.is_null() {
        return ptr::null_mut();
    }

    let cfg = config::load();
    d3d9_wrapper::create(real_d3d9, cfg)
}

/// # Safety
/// Must only be called after `load_real_d3d9` has succeeded.
#[no_mangle]
pub unsafe extern "system" fn Direct3DCreate9Ex(sdk_version: u32, ppd3d: *mut *mut c_void) -> i32 {
    if REAL_DIRECT3D_CREATE9_EX == 0 {
        return -1; // D3DERR_NOTAVAILABLE
    }

    let real_create_ex: Direct3DCreate9ExFn = std::mem::transmute(REAL_DIRECT3D_CREATE9_EX);
    let hr = real_create_ex(sdk_version, ppd3d);
    if hr >= 0 && !ppd3d.is_null() && !(*ppd3d).is_null() {
        let cfg = config::load();
        *ppd3d = d3d9_wrapper::create(*ppd3d, cfg);
    }
    hr
}

#[no_mangle]
unsafe extern "system" fn DllMain(h_module: HMODULE, reason: u32, _reserved: *mut c_void) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        DisableThreadLibraryCalls(h_module);
        if !load_real_d3d9() {
            return 0;
        }
    }
    TRUE
}
