use crate::api_impl;
use std::ffi::{c_char, c_void, CStr};
use std::fs::File;
use std::io::Write;
use std::sync::Mutex;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    FindClose, FindFirstFileA, FindNextFileA, GetFileAttributesA, FILE_ATTRIBUTE_DIRECTORY,
    INVALID_FILE_ATTRIBUTES, WIN32_FIND_DATAA,
};
use windows_sys::Win32::System::Console::{
    AttachConsole, GetStdHandle, WriteConsoleA, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleFileNameA, GetModuleHandleA, GetModuleHandleExA, GetProcAddress, LoadLibraryA,
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
};

extern "system" {
    fn CreateFileA(
        name: *const u8,
        access: u32,
        share: u32,
        security: *const c_void,
        disposition: u32,
        flags: u32,
        template: *mut c_void,
    ) -> HANDLE;
    fn WriteFile(
        file: HANDLE,
        buf: *const u8,
        len: u32,
        written: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn CreateDirectoryA(path: *const u8, security: *const c_void) -> i32;
    fn GetLocalTime(st: *mut SYSTEMTIME);
    fn FreeLibrary(module: *mut c_void) -> i32;
}

#[repr(C)]
struct SYSTEMTIME {
    w_year: u16,
    w_month: u16,
    w_day_of_week: u16,
    w_day: u16,
    w_hour: u16,
    w_minute: u16,
    w_second: u16,
    w_milliseconds: u16,
}

const GENERIC_WRITE: u32 = 0x40000000;
const CREATE_NEW: u32 = 1;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const MAX_PATH: usize = 260;
const CHUMOD_API_VERSION: u32 = 2;

#[repr(C)]
pub struct ChuModInfo {
    pub api_version: u32,
    pub loader_version: *const c_char,
    pub game_module: *const c_char,
    pub game_base: usize,
    pub game_size: u32,
    pub text_base: usize,
    pub text_size: u32,
    pub rdata_base: usize,
    pub rdata_size: u32,
}

#[repr(C)]
pub struct ChuModAPI {
    pub struct_size: u32,
    pub log: Option<unsafe extern "C" fn(*const c_char, ...)>,
    pub aob_scan: Option<unsafe extern "C" fn(usize, u32, *const u8, *const c_char) -> usize>,
    pub mem_read: Option<unsafe extern "C" fn(usize, *mut c_void, u32) -> i32>,
    pub mem_write: Option<unsafe extern "C" fn(usize, *const c_void, u32) -> i32>,
    pub mem_fill: Option<unsafe extern "C" fn(usize, u8, u32) -> i32>,
    pub hook_create:
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32>,
    pub hook_enable: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub hook_disable: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub hook_remove: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub register_service: Option<unsafe extern "C" fn(*const c_char, *mut c_void) -> i32>,
    pub get_service: Option<unsafe extern "C" fn(*const c_char) -> *mut c_void>,
    pub publish: Option<unsafe extern "C" fn(*const c_char, *mut c_void, u32) -> i32>,
    pub subscribe: Option<
        unsafe extern "C" fn(
            *const c_char,
            Option<unsafe extern "C" fn(*const c_char, *mut c_void, u32)>,
        ) -> i32,
    >,
    pub rtti_find_vtable: Option<unsafe extern "C" fn(*const c_char) -> usize>,
    pub config_get_int: Option<unsafe extern "C" fn(*const c_char, i32) -> i32>,
    pub config_get_float: Option<unsafe extern "C" fn(*const c_char, f32) -> f32>,
    pub config_get_bool: Option<unsafe extern "C" fn(*const c_char, i32) -> i32>,
    pub config_get_string:
        Option<unsafe extern "C" fn(*const c_char, *mut c_char, u32, *const c_char) -> i32>,
    pub config_set_int: Option<unsafe extern "C" fn(*const c_char, i32) -> i32>,
    pub config_set_float: Option<unsafe extern "C" fn(*const c_char, f32) -> i32>,
    pub config_set_bool: Option<unsafe extern "C" fn(*const c_char, i32) -> i32>,
    pub config_set_string: Option<unsafe extern "C" fn(*const c_char, *const c_char) -> i32>,
}

type ChuModInitFunc = unsafe extern "C" fn(*const ChuModInfo, *const ChuModAPI) -> i32;
type ChuModShutdownFunc = unsafe extern "C" fn();
type ChuModNameFunc = unsafe extern "C" fn() -> *const c_char;

type HMODULE = *mut c_void;

struct LoadedMod {
    handle: HMODULE,
    shutdown: Option<ChuModShutdownFunc>,
    name: String,
}

unsafe impl Send for LoadedMod {}

struct LoaderState {
    loaded: bool,
    mods: Vec<LoadedMod>,
    base_dir: String,
    log_file: Option<File>,
    console: HANDLE,
}

unsafe impl Send for LoaderState {}

impl Default for LoaderState {
    fn default() -> Self {
        Self {
            loaded: false,
            mods: Vec::new(),
            base_dir: String::new(),
            log_file: None,
            console: INVALID_HANDLE_VALUE,
        }
    }
}

static STATE: once_cell::sync::Lazy<Mutex<LoaderState>> =
    once_cell::sync::Lazy::new(|| Mutex::new(LoaderState::default()));

fn get_self_base_dir() -> Option<String> {
    unsafe {
        let mut self_module: HMODULE = std::ptr::null_mut();
        let dummy_addr = get_self_base_dir as *const () as *const u8;
        GetModuleHandleExA(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            dummy_addr,
            &mut self_module,
        );
        if self_module.is_null() {
            return None;
        }

        let mut buf = [0u8; MAX_PATH];
        let len = GetModuleFileNameA(self_module, buf.as_mut_ptr(), buf.len() as u32);
        if len == 0 {
            return None;
        }

        let path = String::from_utf8_lossy(&buf[..len as usize]).into_owned();
        path.rfind('\\')
            .or_else(|| path.rfind('/'))
            .map(|pos| path[..pos].to_string())
    }
}

fn write_log_inner(state: &mut LoaderState, msg: &str) {
    unsafe {
        let mut st: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut st);

        let formatted = format!(
            "[{:02}:{:02}:{:02}.{:03}] [loader] {}\n",
            st.w_hour, st.w_minute, st.w_second, st.w_milliseconds, msg
        );

        if let Some(ref mut f) = state.log_file {
            let _ = f.write_all(formatted.as_bytes());
            let _ = f.flush();
        }

        if state.console != INVALID_HANDLE_VALUE && !state.console.is_null() {
            let mut written = 0u32;
            WriteConsoleA(
                state.console,
                formatted.as_ptr(),
                formatted.len() as u32,
                &mut written,
                std::ptr::null(),
            );
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn write_log_variadic(fmt: *const c_char, args: ...) {
    extern "C" {
        fn vsnprintf(buf: *mut u8, size: usize, fmt: *const c_char, args: *const c_void) -> i32;
    }
    let mut buf = [0u8; 480];
    let len = vsnprintf(
        buf.as_mut_ptr(),
        buf.len(),
        fmt,
        &args as *const _ as *const c_void,
    );
    let len = if len < 0 {
        0
    } else {
        len.min(buf.len() as i32 - 1)
    } as usize;
    let text = String::from_utf8_lossy(&buf[..len]);
    if let Ok(mut state) = STATE.lock() {
        write_log_inner(&mut state, &text);
    }
}

fn is_mod_enabled(ini_path: &str, mod_name: &str) -> bool {
    extern "system" {
        fn GetPrivateProfileStringA(
            app: *const u8,
            key: *const u8,
            default: *const u8,
            ret: *mut u8,
            size: u32,
            file: *const u8,
        ) -> u32;
    }
    unsafe {
        let mut value = [0u8; 32];
        let section = b"mods\0";
        let ini_cstr = format!("{}\0", ini_path);
        let mod_cstr = format!("{}\0", mod_name);
        let empty = b"\0";

        GetPrivateProfileStringA(
            section.as_ptr(),
            mod_cstr.as_ptr(),
            empty.as_ptr(),
            value.as_mut_ptr(),
            value.len() as u32,
            ini_cstr.as_ptr(),
        );

        if value[0] == 0 {
            return true;
        }

        let s = CStr::from_ptr(value.as_ptr() as *const c_char);
        s.to_str()
            .map_or(true, |v| v.parse::<i32>().unwrap_or(1) != 0)
    }
}

#[repr(C)]
struct ImageDosHeader {
    e_magic: u16,
    _pad: [u16; 29],
    e_lfanew: i32,
}

#[repr(C)]
struct ImageFileHeader {
    machine: u16,
    number_of_sections: u16,
    _pad: [u32; 3],
    size_of_optional_header: u16,
    characteristics: u16,
}

#[repr(C)]
struct ImageOptionalHeader32 {
    _pad: [u8; 56],
    size_of_image: u32,
}

#[repr(C)]
struct ImageNtHeaders32 {
    signature: u32,
    file_header: ImageFileHeader,
    optional_header: ImageOptionalHeader32,
}

#[repr(C)]
struct ImageSectionHeader {
    name: [u8; 8],
    virtual_size: u32,
    virtual_address: u32,
    _rest: [u32; 4],
    _characteristics: u32,
}

fn parse_game_info(game: HMODULE) -> (u32, usize, u32, usize, u32) {
    unsafe {
        let base = game as usize;
        let dos = &*(base as *const ImageDosHeader);
        let nt = &*((base + dos.e_lfanew as usize) as *const ImageNtHeaders32);
        let game_size = nt.optional_header.size_of_image;

        let first_section = (nt as *const ImageNtHeaders32 as usize)
            + std::mem::size_of::<u32>()
            + std::mem::size_of::<ImageFileHeader>()
            + nt.file_header.size_of_optional_header as usize;

        let num_sections = nt.file_header.number_of_sections;
        let mut text_base = 0usize;
        let mut text_size = 0u32;
        let mut rdata_base = 0usize;
        let mut rdata_size = 0u32;

        for i in 0..num_sections as usize {
            let sec = &*((first_section + i * std::mem::size_of::<ImageSectionHeader>())
                as *const ImageSectionHeader);
            if &sec.name[..5] == b".text" {
                text_base = base + sec.virtual_address as usize;
                text_size = sec.virtual_size;
            }
            if &sec.name[..6] == b".rdata" {
                rdata_base = base + sec.virtual_address as usize;
                rdata_size = sec.virtual_size;
            }
        }

        (game_size, text_base, text_size, rdata_base, rdata_size)
    }
}

pub unsafe fn load_mods() {
    let mut state = STATE.lock().unwrap();
    if state.loaded {
        return;
    }
    state.loaded = true;

    state.console = GetStdHandle(STD_OUTPUT_HANDLE);
    if state.console.is_null() || state.console == INVALID_HANDLE_VALUE {
        AttachConsole(ATTACH_PARENT_PROCESS);
        state.console = GetStdHandle(STD_OUTPUT_HANDLE);
    }

    let base_dir = match get_self_base_dir() {
        Some(d) => d,
        None => return,
    };
    state.base_dir = base_dir.clone();

    let log_path = format!("{}\\chusan_loader.log", base_dir);
    state.log_file = File::create(&log_path).ok();

    let mods_dir = format!("{}\\mods", base_dir);
    let ini_path = format!("{}\\mods.ini", base_dir);
    let pattern = format!("{}\\*.dll\0", mods_dir);

    write_log_inner(&mut state, &format!("loader start: base={}", base_dir));
    drop(state);

    api_impl::init();

    let game = GetModuleHandleA(b"chusanApp.exe\0".as_ptr());
    let (
        game_size_cached,
        text_base_cached,
        text_size_cached,
        rdata_base_cached,
        rdata_size_cached,
    ) = if !game.is_null() {
        parse_game_info(game)
    } else {
        (0u32, 0usize, 0u32, 0usize, 0u32)
    };
    api_impl::set_rtti_info(
        rdata_base_cached,
        rdata_size_cached as usize,
        text_base_cached,
    );

    let mods_dir_c = format!("{}\0", mods_dir);
    let attrs = GetFileAttributesA(mods_dir_c.as_ptr());
    if attrs == INVALID_FILE_ATTRIBUTES {
        CreateDirectoryA(mods_dir_c.as_ptr(), std::ptr::null());
        let mut s = STATE.lock().unwrap();
        write_log_inner(&mut s, &format!("created mods dir: {}", mods_dir));
        drop(s);
    }

    let ini_path_c = format!("{}\0", ini_path);
    let ini_attrs = GetFileAttributesA(ini_path_c.as_ptr());
    if ini_attrs == INVALID_FILE_ATTRIBUTES {
        let hf = CreateFileA(
            ini_path_c.as_ptr(),
            GENERIC_WRITE,
            0,
            std::ptr::null(),
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        );
        if hf != INVALID_HANDLE_VALUE {
            let default_ini = b"[mods]\r\n; mod_name.dll=0\r\n";
            let mut written = 0u32;
            WriteFile(
                hf,
                default_ini.as_ptr(),
                default_ini.len() as u32,
                &mut written,
                std::ptr::null_mut(),
            );
            CloseHandle(hf);
            let mut s = STATE.lock().unwrap();
            write_log_inner(&mut s, "created default mods.ini");
            drop(s);
        }
    }

    {
        let mut s = STATE.lock().unwrap();
        write_log_inner(&mut s, &format!("scan mods dir: {}", mods_dir));
        write_log_inner(&mut s, &format!("config file: {}", ini_path));
    }

    let mut find_data: WIN32_FIND_DATAA = std::mem::zeroed();
    let find_handle = FindFirstFileA(pattern.as_ptr(), &mut find_data);
    if find_handle == INVALID_HANDLE_VALUE {
        let mut s = STATE.lock().unwrap();
        write_log_inner(&mut s, "no mods found or cannot open directory");
        return;
    }

    loop {
        if (find_data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0 {
            if FindNextFileA(find_handle, &mut find_data) == 0 {
                break;
            }
            continue;
        }

        let mod_name_cstr = CStr::from_ptr(find_data.cFileName.as_ptr() as *const c_char);
        let mod_name = mod_name_cstr.to_string_lossy().into_owned();
        let full_path = format!("{}\\{}", mods_dir, mod_name);

        if !is_mod_enabled(&ini_path, &mod_name) {
            let mut s = STATE.lock().unwrap();
            write_log_inner(&mut s, &format!("mod disabled: {}", full_path));
            drop(s);
            if FindNextFileA(find_handle, &mut find_data) == 0 {
                break;
            }
            continue;
        }

        let full_path_c = format!("{}\0", full_path);
        let mod_handle = LoadLibraryA(full_path_c.as_ptr());

        if mod_handle.is_null() {
            let err = GetLastError();
            let mut s = STATE.lock().unwrap();
            write_log_inner(
                &mut s,
                &format!("failed to load mod: {} (err={})", full_path, err),
            );
            drop(s);
            if FindNextFileA(find_handle, &mut find_data) == 0 {
                break;
            }
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

        let init_fn_ptr = GetProcAddress(mod_handle, b"chumod_init\0".as_ptr());
        if let Some(init_fn) = init_fn_ptr {
            let init_fn: ChuModInitFunc = std::mem::transmute(init_fn);

            let game_module_str: *const c_char = if !game.is_null() {
                b"chusanApp.exe\0".as_ptr() as *const c_char
            } else {
                std::ptr::null()
            };

            let loader_ver = b"2.0.0\0".as_ptr() as *const c_char;
            let info = ChuModInfo {
                api_version: CHUMOD_API_VERSION,
                loader_version: loader_ver,
                game_module: game_module_str,
                game_base: if !game.is_null() { game as usize } else { 0 },
                game_size: game_size_cached,
                text_base: text_base_cached,
                text_size: text_size_cached,
                rdata_base: rdata_base_cached,
                rdata_size: rdata_size_cached,
            };

            let api = api_impl::get_api();
            (*api).log = Some(write_log_variadic);

            let config_dir = format!("{}\\mods\\config", base_dir);
            let config_dir_c = format!("{}\0", config_dir);
            CreateDirectoryA(config_dir_c.as_ptr(), std::ptr::null());

            let mod_stem = mod_name
                .strip_suffix(".dll")
                .or_else(|| mod_name.strip_suffix(".DLL"))
                .unwrap_or(&mod_name);
            let config_path = format!("{}\\{}.ini", config_dir, mod_stem);
            api_impl::set_current_config(&config_path);

            let ret = init_fn(&info, api);
            if ret != 0 {
                let mut s = STATE.lock().unwrap();
                write_log_inner(
                    &mut s,
                    &format!("mod init failed (ret={}): {}", ret, display_name),
                );
                drop(s);
                FreeLibrary(mod_handle);
                if FindNextFileA(find_handle, &mut find_data) == 0 {
                    break;
                }
                continue;
            }
        }

        let shutdown_ptr = GetProcAddress(mod_handle, b"chumod_shutdown\0".as_ptr());
        let shutdown: Option<ChuModShutdownFunc> = shutdown_ptr.map(|f| std::mem::transmute(f));

        {
            let mut s = STATE.lock().unwrap();
            s.mods.push(LoadedMod {
                handle: mod_handle,
                shutdown,
                name: display_name.clone(),
            });
            write_log_inner(&mut s, &format!("loaded mod: {}", display_name));
        }

        if FindNextFileA(find_handle, &mut find_data) == 0 {
            break;
        }
    }

    FindClose(find_handle);

    let mut s = STATE.lock().unwrap();
    let count = s.mods.len();
    write_log_inner(&mut s, &format!("mods loaded: {}", count));
}

pub unsafe fn unload_mods() {
    let mut state = STATE.lock().unwrap();
    for m in state.mods.iter().rev() {
        if let Some(shutdown) = m.shutdown {
            shutdown();
        }
        if !m.handle.is_null() {
            FreeLibrary(m.handle);
        }
    }
    state.mods.clear();
    state.loaded = false;
    drop(state);

    api_impl::shutdown();

    let mut state = STATE.lock().unwrap();
    write_log_inner(&mut state, "loader shutdown");
    state.log_file = None;
}
