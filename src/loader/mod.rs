pub mod dependency;
pub mod log;
pub mod metadata;
pub mod pe;
pub mod scanner;
pub mod seh;
pub mod state;

use std::ffi::{c_char, CStr};
use std::fs::File;

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Console::{
    AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleA, GetProcAddress, LoadLibraryA,
};

use crate::api_impl;
use crate::types::{
    ChuModInfo, ChuModInitFunc, ChuModNameFunc, ChuModShutdownFunc, CHUMOD_API_VERSION,
};

use self::dependency::{read_dependencies, sort_mods, PendingMod};
use self::log::{log_info, write_log_inner, write_log_variadic};
use self::metadata::{read_metadata, should_load_metadata};
use self::pe::{get_self_base_dir, parse_game_info};
use self::scanner::{ensure_mods_layout, scan_mod_files};
use self::seh::{call_mod_init, call_mod_shutdown};
use self::state::{LoadedMod, STATE};

extern "system" {
    fn CreateDirectoryA(path: *const u8, security: *const std::ffi::c_void) -> i32;
    fn FreeLibrary(module: *mut std::ffi::c_void) -> i32;
}

pub unsafe fn load_mods() {
    let mut state = STATE.lock().unwrap();
    if state.loaded {
        return;
    }
    state.loaded = true;

    state.console = GetStdHandle(STD_OUTPUT_HANDLE);
    if state.console.is_null() || state.console == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        AttachConsole(ATTACH_PARENT_PROCESS);
        state.console = GetStdHandle(STD_OUTPUT_HANDLE);
    }

    let base_dir = match get_self_base_dir() {
        Some(d) => d,
        None => return,
    };
    state.base_dir = base_dir.clone();
    state.log_file = File::create(format!("{}\\chusan_loader.log", base_dir)).ok();
    write_log_inner(&mut state, &format!("loader start: base={}", base_dir));
    drop(state);

    api_impl::init();

    let game = GetModuleHandleA(b"chusanApp.exe\0".as_ptr());
    let (game_size, text_base, text_size, rdata_base, rdata_size) = if !game.is_null() {
        parse_game_info(game)
    } else {
        (0, 0, 0, 0, 0)
    };
    api_impl::set_rtti_info(rdata_base, rdata_size as usize, text_base);

    let (mods_dir, ini_path) = ensure_mods_layout(&base_dir);
    log_info(&format!("scan mods dir: {}", mods_dir));
    log_info(&format!("config file: {}", ini_path));

    let mut pending_mods = Vec::new();
    for (mod_name, full_path) in scan_mod_files(&mods_dir, &ini_path) {
        let full_path_c = format!("{}\0", full_path);
        let mod_handle = LoadLibraryA(full_path_c.as_ptr());
        if mod_handle.is_null() {
            log_info(&format!(
                "failed to load mod: {} (err={})",
                full_path,
                GetLastError()
            ));
            continue;
        }

        let mut display_name = mod_name.clone();
        let name_fn_ptr = GetProcAddress(mod_handle, b"chumod_name\0".as_ptr());
        if let Some(name_fn) = name_fn_ptr {
            let name_fn: ChuModNameFunc = std::mem::transmute(name_fn);
            let n = name_fn();
            if !n.is_null() {
                display_name = CStr::from_ptr(n).to_string_lossy().into_owned();
            }
        }

        let metadata = read_metadata(mod_handle);
        if !should_load_metadata(&display_name, &metadata) {
            FreeLibrary(mod_handle);
            continue;
        }
        if metadata.version.is_some() || metadata.author.is_some() {
            log_info(&format!(
                "mod metadata: {} version={} author={}",
                display_name,
                metadata.version.as_deref().unwrap_or("unknown"),
                metadata.author.as_deref().unwrap_or("unknown")
            ));
        }

        let dependencies = read_dependencies(mod_handle);
        pending_mods.push(PendingMod {
            file_name: mod_name,
            handle: mod_handle,
            display_name,
            dependencies,
            metadata,
        });
    }

    for pending in sort_mods(pending_mods) {
        let mod_name = pending.file_name;
        let mod_handle = pending.handle;
        let display_name = pending.display_name;

        let init_fn_ptr = GetProcAddress(mod_handle, b"chumod_init\0".as_ptr());
        if let Some(init_fn) = init_fn_ptr {
            let init_fn: ChuModInitFunc = std::mem::transmute(init_fn);
            let game_module_str: *const c_char = if !game.is_null() {
                b"chusanApp.exe\0".as_ptr() as *const c_char
            } else {
                std::ptr::null()
            };
            let loader_ver = concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char;
            let info = ChuModInfo {
                api_version: CHUMOD_API_VERSION,
                loader_version: loader_ver,
                game_module: game_module_str,
                game_base: if !game.is_null() { game as usize } else { 0 },
                game_size,
                text_base,
                text_size,
                rdata_base,
                rdata_size,
            };

            let api = api_impl::get_api();
            (*api).log = Some(write_log_variadic);
            let config_dir = format!("{}\\mods\\config", base_dir);
            CreateDirectoryA(format!("{}\0", config_dir).as_ptr(), std::ptr::null());
            let mod_stem = mod_name
                .strip_suffix(".dll")
                .or_else(|| mod_name.strip_suffix(".DLL"))
                .unwrap_or(&mod_name);
            api_impl::set_current_config(&format!("{}\\{}.ini", config_dir, mod_stem));

            let ret = call_mod_init(&display_name, init_fn, &info, api);
            if ret != Some(0) {
                if let Some(ret) = ret {
                    log_info(&format!("mod init failed (ret={}): {}", ret, display_name));
                }
                FreeLibrary(mod_handle);
                continue;
            }
        }

        let shutdown_ptr = GetProcAddress(mod_handle, b"chumod_shutdown\0".as_ptr());
        let shutdown: Option<ChuModShutdownFunc> = shutdown_ptr.map(|f| std::mem::transmute(f));

        let mut state = STATE.lock().unwrap();
        state.mods.push(LoadedMod {
            handle: mod_handle,
            shutdown,
            name: display_name.clone(),
        });
        write_log_inner(&mut state, &format!("loaded mod: {}", display_name));
    }

    let mut state = STATE.lock().unwrap();
    let count = state.mods.len();
    write_log_inner(&mut state, &format!("mods loaded: {}", count));
}

pub unsafe fn unload_mods() {
    let mut state = STATE.lock().unwrap();
    while let Some(m) = state.mods.pop() {
        write_log_inner(&mut state, &format!("unloading mod: {}", m.name));
        if let Some(shutdown) = m.shutdown {
            call_mod_shutdown(&m.name, shutdown);
        }
        if !m.handle.is_null() {
            FreeLibrary(m.handle);
        }
    }
    state.loaded = false;
    drop(state);

    api_impl::shutdown();

    let mut state = STATE.lock().unwrap();
    write_log_inner(&mut state, "loader shutdown");
    state.log_file = None;
}
