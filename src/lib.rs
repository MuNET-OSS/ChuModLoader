#![allow(non_snake_case)]
#![feature(c_variadic)]

mod api_impl;
mod loader;

use std::ffi::c_void;
use windows_sys::Win32::Foundation::{BOOL, HMODULE, TRUE};
use windows_sys::Win32::System::LibraryLoader::{
    DisableThreadLibraryCalls, GetProcAddress, LoadLibraryA,
};
use windows_sys::Win32::System::Threading::Sleep;

const DLL_PROCESS_ATTACH: u32 = 1;
const DLL_PROCESS_DETACH: u32 = 0;

extern "system" {
    fn GetSystemDirectoryA(buf: *mut u8, size: u32) -> u32;
    fn CreateThread(
        attrs: *const c_void,
        stack_size: usize,
        start: Option<unsafe extern "system" fn(*mut c_void) -> u32>,
        param: *mut c_void,
        flags: u32,
        id: *mut u32,
    ) -> *mut c_void;
    fn FreeLibrary(module: *mut c_void) -> i32;
}

static mut REAL_VERSION: HMODULE = std::ptr::null_mut();
static mut REAL_FP: [usize; 17] = [0; 17];

static EXPORT_NAMES: [&[u8]; 17] = [
    b"GetFileVersionInfoA\0",
    b"GetFileVersionInfoByHandle\0",
    b"GetFileVersionInfoExA\0",
    b"GetFileVersionInfoExW\0",
    b"GetFileVersionInfoSizeA\0",
    b"GetFileVersionInfoSizeExA\0",
    b"GetFileVersionInfoSizeExW\0",
    b"GetFileVersionInfoSizeW\0",
    b"GetFileVersionInfoW\0",
    b"VerFindFileA\0",
    b"VerFindFileW\0",
    b"VerInstallFileA\0",
    b"VerInstallFileW\0",
    b"VerLanguageNameA\0",
    b"VerLanguageNameW\0",
    b"VerQueryValueA\0",
    b"VerQueryValueW\0",
];

macro_rules! proxy_fn {
    ($name:ident, $idx:expr) => {
        #[unsafe(naked)]
        #[no_mangle]
        unsafe extern "C" fn $name() {
            core::arch::naked_asm!(
                "jmp dword ptr [{fp} + {off}]",
                fp = sym REAL_FP,
                off = const $idx * 4,
            );
        }
    };
}

proxy_fn!(p_GetFileVersionInfoA, 0);
proxy_fn!(p_GetFileVersionInfoByHandle, 1);
proxy_fn!(p_GetFileVersionInfoExA, 2);
proxy_fn!(p_GetFileVersionInfoExW, 3);
proxy_fn!(p_GetFileVersionInfoSizeA, 4);
proxy_fn!(p_GetFileVersionInfoSizeExA, 5);
proxy_fn!(p_GetFileVersionInfoSizeExW, 6);
proxy_fn!(p_GetFileVersionInfoSizeW, 7);
proxy_fn!(p_GetFileVersionInfoW, 8);
proxy_fn!(p_VerFindFileA, 9);
proxy_fn!(p_VerFindFileW, 10);
proxy_fn!(p_VerInstallFileA, 11);
proxy_fn!(p_VerInstallFileW, 12);
proxy_fn!(p_VerLanguageNameA, 13);
proxy_fn!(p_VerLanguageNameW, 14);
proxy_fn!(p_VerQueryValueA, 15);
proxy_fn!(p_VerQueryValueW, 16);

unsafe fn load_real_version() {
    let mut sys_dir = [0u8; 260];
    let len = GetSystemDirectoryA(sys_dir.as_mut_ptr(), sys_dir.len() as u32);
    if len == 0 {
        return;
    }

    let mut real_path = [0u8; 260];
    let dir = &sys_dir[..len as usize];
    real_path[..dir.len()].copy_from_slice(dir);
    let suffix = b"\\version.dll\0";
    real_path[dir.len()..dir.len() + suffix.len()].copy_from_slice(suffix);

    REAL_VERSION = LoadLibraryA(real_path.as_ptr());
    if REAL_VERSION.is_null() {
        return;
    }

    for i in 0..17 {
        let addr = GetProcAddress(REAL_VERSION, EXPORT_NAMES[i].as_ptr());
        REAL_FP[i] = addr.map_or(0, |f| f as usize);
    }
}

unsafe extern "system" fn loader_thread(_param: *mut c_void) -> u32 {
    Sleep(2000);
    loader::load_mods();
    0
}

#[no_mangle]
unsafe extern "system" fn DllMain(h_module: HMODULE, reason: u32, _reserved: *mut c_void) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => {
            DisableThreadLibraryCalls(h_module);
            load_real_version();
            CreateThread(
                std::ptr::null(),
                0,
                Some(loader_thread),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
            );
        }
        DLL_PROCESS_DETACH => {
            loader::unload_mods();
            if !REAL_VERSION.is_null() {
                FreeLibrary(REAL_VERSION);
            }
        }
        _ => {}
    }
    TRUE
}
