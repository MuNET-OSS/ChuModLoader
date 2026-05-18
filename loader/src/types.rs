use std::ffi::{c_char, c_void};

pub const CHUMOD_API_VERSION: u32 = 3;
pub const LOADER_VERSION: &str = "1.0.0";

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
    pub game_version: *const c_char,
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
    pub log_info: Option<unsafe extern "C" fn(*const c_char)>,
    pub log_warn: Option<unsafe extern "C" fn(*const c_char)>,
    pub log_error: Option<unsafe extern "C" fn(*const c_char)>,
    pub log_path: *const c_char,
    pub toml_section_exists: Option<unsafe extern "C" fn(*const c_char) -> i32>,
    pub toml_get_bool: Option<unsafe extern "C" fn(*const c_char, *const c_char, i32) -> i32>,
    pub toml_get_int: Option<unsafe extern "C" fn(*const c_char, *const c_char, i32) -> i32>,
    pub toml_get_float: Option<unsafe extern "C" fn(*const c_char, *const c_char, f32) -> f32>,
    pub toml_get_string: Option<
        unsafe extern "C" fn(*const c_char, *const c_char, *mut c_char, u32, *const c_char) -> i32,
    >,
    pub get_manifest_path: Option<unsafe extern "C" fn() -> *const c_char>,
    pub reload_mod: Option<unsafe extern "C" fn(*const c_char) -> i32>,
}

pub type ChuModInitFunc = unsafe extern "C" fn(*const ChuModInfo, *const ChuModAPI) -> i32;
pub type ChuModReadyFunc = unsafe extern "C" fn();
pub type ChuModFrameFunc = unsafe extern "C" fn();
pub type ChuModShutdownFunc = unsafe extern "C" fn();
pub type ChuModNameFunc = unsafe extern "C" fn() -> *const c_char;
pub type ChuModDependsFunc = unsafe extern "C" fn() -> *const c_char;
pub type ChuModStringFunc = unsafe extern "C" fn() -> *const c_char;
