#![allow(non_snake_case)]
#![feature(c_variadic)]

mod api_impl;
mod loader;
mod types;

use std::ffi::c_void;
use windows_sys::Win32::Foundation::{BOOL, HMODULE, TRUE};
use windows_sys::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows_sys::Win32::System::Threading::Sleep;

const DLL_PROCESS_ATTACH: u32 = 1;
const DLL_PROCESS_DETACH: u32 = 0;

extern "system" {
    fn CreateThread(
        attrs: *const c_void,
        stack_size: usize,
        start: Option<unsafe extern "system" fn(*mut c_void) -> u32>,
        param: *mut c_void,
        flags: u32,
        id: *mut u32,
    ) -> *mut c_void;
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
        }
        _ => {}
    }
    TRUE
}
