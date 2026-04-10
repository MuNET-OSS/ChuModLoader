use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::Mutex;

use crate::loader::ChuModAPI;
use retour::RawDetour;

const PAGE_EXECUTE_READWRITE: u32 = 0x40;

struct SendPtr(*mut c_void);
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

static mut G_API: ChuModAPI = ChuModAPI {
    struct_size: 0,
    log: None,
    aob_scan: None,
    mem_read: None,
    mem_write: None,
    mem_fill: None,
    hook_create: None,
    hook_enable: None,
    hook_disable: None,
    hook_remove: None,
    register_service: None,
    get_service: None,
    publish: None,
    subscribe: None,
    rtti_find_vtable: None,
    config_get_int: None,
    config_get_float: None,
    config_get_bool: None,
    config_get_string: None,
    config_set_int: None,
    config_set_float: None,
    config_set_bool: None,
    config_set_string: None,
};

static SERVICES: once_cell::sync::Lazy<Mutex<HashMap<String, SendPtr>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

struct Subscriber {
    topic: String,
    cb: unsafe extern "C" fn(*const c_char, *mut c_void, u32),
}

unsafe impl Send for Subscriber {}

static SUBSCRIBERS: once_cell::sync::Lazy<Mutex<Vec<Subscriber>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(Vec::new()));

struct HookEntry {
    detour: RawDetour,
}

unsafe impl Send for HookEntry {}
unsafe impl Sync for HookEntry {}

static HOOKS: once_cell::sync::Lazy<Mutex<HashMap<usize, HookEntry>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

unsafe extern "C" fn api_aob_scan(
    start: usize,
    size: u32,
    pat: *const u8,
    mask: *const c_char,
) -> usize {
    if mask.is_null() || pat.is_null() {
        return 0;
    }
    let mask_str = CStr::from_ptr(mask);
    let mask_bytes = mask_str.to_bytes();
    let len = mask_bytes.len();
    if (size as usize) < len {
        return 0;
    }
    for i in 0..=(size as usize - len) {
        let mem = start + i;
        let mut ok = true;
        for j in 0..len {
            if mask_bytes[j] == b'x' && *((mem + j) as *const u8) != *pat.add(j) {
                ok = false;
                break;
            }
        }
        if ok {
            return mem;
        }
    }
    0
}

unsafe extern "C" fn api_mem_read(addr: usize, buf: *mut c_void, size: u32) -> i32 {
    let mut old_protect = 0u32;
    if windows_sys::Win32::System::Memory::VirtualProtect(
        addr as *const c_void,
        size as usize,
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    ) == 0
    {
        return -1;
    }
    std::ptr::copy_nonoverlapping(addr as *const u8, buf as *mut u8, size as usize);
    windows_sys::Win32::System::Memory::VirtualProtect(
        addr as *const c_void,
        size as usize,
        old_protect,
        &mut old_protect,
    );
    0
}

unsafe extern "C" fn api_mem_write(addr: usize, buf: *const c_void, size: u32) -> i32 {
    let mut old_protect = 0u32;
    if windows_sys::Win32::System::Memory::VirtualProtect(
        addr as *const c_void,
        size as usize,
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    ) == 0
    {
        return -1;
    }
    std::ptr::copy_nonoverlapping(buf as *const u8, addr as *mut u8, size as usize);
    windows_sys::Win32::System::Memory::VirtualProtect(
        addr as *const c_void,
        size as usize,
        old_protect,
        &mut old_protect,
    );
    0
}

unsafe extern "C" fn api_mem_fill(addr: usize, value: u8, size: u32) -> i32 {
    let mut old_protect = 0u32;
    if windows_sys::Win32::System::Memory::VirtualProtect(
        addr as *const c_void,
        size as usize,
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    ) == 0
    {
        return -1;
    }
    std::ptr::write_bytes(addr as *mut u8, value, size as usize);
    windows_sys::Win32::System::Memory::VirtualProtect(
        addr as *const c_void,
        size as usize,
        old_protect,
        &mut old_protect,
    );
    0
}

unsafe extern "C" fn api_hook_create(
    target: *mut c_void,
    detour: *mut c_void,
    original: *mut *mut c_void,
) -> i32 {
    match RawDetour::new(target as *const (), detour as *const ()) {
        Ok(hook) => {
            let trampoline = hook.trampoline() as *const () as *mut c_void;
            if !original.is_null() {
                *original = trampoline;
            }
            let mut hooks = HOOKS.lock().unwrap();
            hooks.insert(target as usize, HookEntry { detour: hook });
            0
        }
        Err(_) => -1,
    }
}

unsafe extern "C" fn api_hook_enable(target: *mut c_void) -> i32 {
    let hooks = HOOKS.lock().unwrap();
    match hooks.get(&(target as usize)) {
        Some(entry) => {
            if entry.detour.enable().is_ok() {
                0
            } else {
                -1
            }
        }
        None => -1,
    }
}

unsafe extern "C" fn api_hook_disable(target: *mut c_void) -> i32 {
    let hooks = HOOKS.lock().unwrap();
    match hooks.get(&(target as usize)) {
        Some(entry) => {
            if entry.detour.disable().is_ok() {
                0
            } else {
                -1
            }
        }
        None => -1,
    }
}

unsafe extern "C" fn api_hook_remove(target: *mut c_void) -> i32 {
    let mut hooks = HOOKS.lock().unwrap();
    if hooks.remove(&(target as usize)).is_some() {
        0
    } else {
        -1
    }
}

unsafe extern "C" fn api_register_service(name: *const c_char, ptr: *mut c_void) -> i32 {
    if name.is_null() {
        return -1;
    }
    let key = CStr::from_ptr(name).to_string_lossy().into_owned();
    let mut services = SERVICES.lock().unwrap();
    services.insert(key, SendPtr(ptr));
    0
}

unsafe extern "C" fn api_get_service(name: *const c_char) -> *mut c_void {
    if name.is_null() {
        return std::ptr::null_mut();
    }
    let key = CStr::from_ptr(name).to_string_lossy();
    let services = SERVICES.lock().unwrap();
    services
        .get(key.as_ref())
        .map_or(std::ptr::null_mut(), |p| p.0)
}

unsafe extern "C" fn api_publish(topic: *const c_char, data: *mut c_void, size: u32) -> i32 {
    if topic.is_null() {
        return -1;
    }
    let topic_str = CStr::from_ptr(topic).to_string_lossy();
    let subs = SUBSCRIBERS.lock().unwrap();
    for sub in subs.iter() {
        if sub.topic == topic_str.as_ref() {
            (sub.cb)(topic, data, size);
        }
    }
    0
}

unsafe extern "C" fn api_subscribe(
    topic: *const c_char,
    callback: Option<unsafe extern "C" fn(*const c_char, *mut c_void, u32)>,
) -> i32 {
    if topic.is_null() {
        return -1;
    }
    let cb = match callback {
        Some(f) => f,
        None => return -1,
    };
    let topic_str = CStr::from_ptr(topic).to_string_lossy().into_owned();
    let mut subs = SUBSCRIBERS.lock().unwrap();
    subs.push(Subscriber {
        topic: topic_str,
        cb,
    });
    0
}

// ---- RTTI vtable 查找 ----

static RTTI_RDATA_BASE: once_cell::sync::Lazy<std::sync::atomic::AtomicUsize> =
    once_cell::sync::Lazy::new(|| std::sync::atomic::AtomicUsize::new(0));
static RTTI_RDATA_SIZE: once_cell::sync::Lazy<std::sync::atomic::AtomicUsize> =
    once_cell::sync::Lazy::new(|| std::sync::atomic::AtomicUsize::new(0));
static RTTI_TEXT_BASE: once_cell::sync::Lazy<std::sync::atomic::AtomicUsize> =
    once_cell::sync::Lazy::new(|| std::sync::atomic::AtomicUsize::new(0));

pub fn set_rtti_info(rdata_base: usize, rdata_size: usize, text_base: usize) {
    RTTI_RDATA_BASE.store(rdata_base, std::sync::atomic::Ordering::Relaxed);
    RTTI_RDATA_SIZE.store(rdata_size, std::sync::atomic::Ordering::Relaxed);
    RTTI_TEXT_BASE.store(text_base, std::sync::atomic::Ordering::Relaxed);
}

unsafe fn find_pattern_u32(base: usize, size: usize, value: u32) -> Vec<usize> {
    let mut results = Vec::new();
    let needle = value.to_le_bytes();
    let data = std::slice::from_raw_parts(base as *const u8, size);
    let mut i = 0;
    while i + 4 <= data.len() {
        if data[i..i + 4] == needle {
            results.push(base + i);
        }
        i += 1;
    }
    results
}

unsafe extern "C" fn api_rtti_find_vtable(rtti_name: *const c_char) -> usize {
    if rtti_name.is_null() {
        return 0;
    }
    let name = CStr::from_ptr(rtti_name);
    let target = name.to_bytes();
    if target.is_empty() {
        return 0;
    }

    let rdata_base = RTTI_RDATA_BASE.load(std::sync::atomic::Ordering::Relaxed);
    let rdata_size = RTTI_RDATA_SIZE.load(std::sync::atomic::Ordering::Relaxed);
    let text_base = RTTI_TEXT_BASE.load(std::sync::atomic::Ordering::Relaxed);
    if rdata_base == 0 || rdata_size == 0 || text_base == 0 {
        return 0;
    }

    let rdata = std::slice::from_raw_parts(rdata_base as *const u8, rdata_size);
    let mut offset = 0;
    while offset + target.len() < rdata.len() {
        if &rdata[offset..offset + target.len()] == target
            && rdata.get(offset + target.len()) == Some(&0)
        {
            let name_va = rdata_base + offset;
            let td_va = name_va - 8;

            let pvf = *(td_va as *const u32);
            if (pvf as usize) < rdata_base {
                offset += 1;
                continue;
            }

            let refs = find_pattern_u32(rdata_base, rdata_size, td_va as u32);
            for ref_va in &refs {
                let col_va = ref_va - 0x0C;
                if col_va < rdata_base {
                    continue;
                }
                if *(col_va as *const u32) != 0 {
                    continue;
                }
                if *((col_va + 0x0C) as *const u32) as usize != td_va {
                    continue;
                }

                let col_refs = find_pattern_u32(rdata_base, rdata_size, col_va as u32);
                for col_ref in &col_refs {
                    let vtable_va = col_ref + 4;
                    let first_entry = *(vtable_va as *const u32);
                    if (first_entry as usize) >= text_base {
                        return vtable_va;
                    }
                }
            }
        }
        offset += 1;
    }
    0
}

// ---- Config (per-mod INI in mods/config/) ----

static CURRENT_CONFIG_PATH: once_cell::sync::Lazy<Mutex<String>> =
    once_cell::sync::Lazy::new(|| Mutex::new(String::new()));

extern "system" {
    fn GetPrivateProfileStringA(
        app: *const u8,
        key: *const u8,
        default: *const u8,
        ret: *mut u8,
        size: u32,
        file: *const u8,
    ) -> u32;
    fn GetPrivateProfileIntA(app: *const u8, key: *const u8, default: i32, file: *const u8) -> u32;
    fn WritePrivateProfileStringA(
        app: *const u8,
        key: *const u8,
        value: *const u8,
        file: *const u8,
    ) -> i32;
}

pub fn set_current_config(path: &str) {
    if let Ok(mut p) = CURRENT_CONFIG_PATH.lock() {
        *p = path.to_string();
    }
}

fn with_config_path<F, R>(f: F) -> R
where
    F: FnOnce(&str) -> R,
    R: Default,
{
    match CURRENT_CONFIG_PATH.lock() {
        Ok(p) if !p.is_empty() => f(&p),
        _ => R::default(),
    }
}

unsafe extern "C" fn api_config_get_int(key: *const c_char, default: i32) -> i32 {
    if key.is_null() {
        return default;
    }
    let key_str = CStr::from_ptr(key);
    with_config_path(|path| {
        let section = b"config\0";
        let key_c = format!("{}\0", key_str.to_string_lossy());
        let file_c = format!("{}\0", path);
        GetPrivateProfileIntA(section.as_ptr(), key_c.as_ptr(), default, file_c.as_ptr()) as i32
    })
}

unsafe extern "C" fn api_config_get_float(key: *const c_char, default: f32) -> f32 {
    if key.is_null() {
        return default;
    }
    let key_str = CStr::from_ptr(key);
    with_config_path(|path| {
        let section = b"config\0";
        let key_c = format!("{}\0", key_str.to_string_lossy());
        let file_c = format!("{}\0", path);
        let mut buf = [0u8; 64];
        let default_str = format!("{}\0", default);
        let len = GetPrivateProfileStringA(
            section.as_ptr(),
            key_c.as_ptr(),
            default_str.as_ptr(),
            buf.as_mut_ptr(),
            buf.len() as u32,
            file_c.as_ptr(),
        );
        if len == 0 {
            return default;
        }
        let s = std::str::from_utf8(&buf[..len as usize]).unwrap_or("");
        s.parse::<f32>().unwrap_or(default)
    })
}

unsafe extern "C" fn api_config_get_bool(key: *const c_char, default: i32) -> i32 {
    if key.is_null() {
        return default;
    }
    let key_str = CStr::from_ptr(key);
    with_config_path(|path| {
        let section = b"config\0";
        let key_c = format!("{}\0", key_str.to_string_lossy());
        let file_c = format!("{}\0", path);
        let mut buf = [0u8; 32];
        let default_str: &[u8] = if default != 0 { b"true\0" } else { b"false\0" };
        let len = GetPrivateProfileStringA(
            section.as_ptr(),
            key_c.as_ptr(),
            default_str.as_ptr(),
            buf.as_mut_ptr(),
            buf.len() as u32,
            file_c.as_ptr(),
        );
        if len == 0 {
            return default;
        }
        let s = std::str::from_utf8(&buf[..len as usize]).unwrap_or("");
        match s {
            "true" | "1" | "yes" => 1,
            "false" | "0" | "no" => 0,
            _ => default,
        }
    })
}

unsafe extern "C" fn api_config_get_string(
    key: *const c_char,
    buf: *mut c_char,
    buf_size: u32,
    default: *const c_char,
) -> i32 {
    if key.is_null() || buf.is_null() || buf_size == 0 {
        return -1;
    }
    let key_str = CStr::from_ptr(key);
    let default_c = if default.is_null() {
        b"\0".as_ptr()
    } else {
        default as *const u8
    };
    with_config_path(|path| {
        let section = b"config\0";
        let key_c = format!("{}\0", key_str.to_string_lossy());
        let file_c = format!("{}\0", path);
        let len = GetPrivateProfileStringA(
            section.as_ptr(),
            key_c.as_ptr(),
            default_c,
            buf as *mut u8,
            buf_size,
            file_c.as_ptr(),
        );
        len as i32
    })
}

unsafe extern "C" fn api_config_set_int(key: *const c_char, value: i32) -> i32 {
    if key.is_null() {
        return -1;
    }
    let key_str = CStr::from_ptr(key);
    with_config_path(|path| {
        let section = b"config\0";
        let key_c = format!("{}\0", key_str.to_string_lossy());
        let val_c = format!("{}\0", value);
        let file_c = format!("{}\0", path);
        WritePrivateProfileStringA(
            section.as_ptr(),
            key_c.as_ptr(),
            val_c.as_ptr(),
            file_c.as_ptr(),
        ) - 1
    })
}

unsafe extern "C" fn api_config_set_float(key: *const c_char, value: f32) -> i32 {
    if key.is_null() {
        return -1;
    }
    let key_str = CStr::from_ptr(key);
    with_config_path(|path| {
        let section = b"config\0";
        let key_c = format!("{}\0", key_str.to_string_lossy());
        let val_c = format!("{}\0", value);
        let file_c = format!("{}\0", path);
        WritePrivateProfileStringA(
            section.as_ptr(),
            key_c.as_ptr(),
            val_c.as_ptr(),
            file_c.as_ptr(),
        ) - 1
    })
}

unsafe extern "C" fn api_config_set_bool(key: *const c_char, value: i32) -> i32 {
    if key.is_null() {
        return -1;
    }
    let key_str = CStr::from_ptr(key);
    with_config_path(|path| {
        let section = b"config\0";
        let key_c = format!("{}\0", key_str.to_string_lossy());
        let val_c: &[u8] = if value != 0 { b"true\0" } else { b"false\0" };
        let file_c = format!("{}\0", path);
        WritePrivateProfileStringA(
            section.as_ptr(),
            key_c.as_ptr(),
            val_c.as_ptr(),
            file_c.as_ptr(),
        ) - 1
    })
}

unsafe extern "C" fn api_config_set_string(key: *const c_char, value: *const c_char) -> i32 {
    if key.is_null() || value.is_null() {
        return -1;
    }
    let key_str = CStr::from_ptr(key);
    let val_str = CStr::from_ptr(value);
    with_config_path(|path| {
        let section = b"config\0";
        let key_c = format!("{}\0", key_str.to_string_lossy());
        let val_c = format!("{}\0", val_str.to_string_lossy());
        let file_c = format!("{}\0", path);
        WritePrivateProfileStringA(
            section.as_ptr(),
            key_c.as_ptr(),
            val_c.as_ptr(),
            file_c.as_ptr(),
        ) - 1
    })
}

fn make_api() -> ChuModAPI {
    ChuModAPI {
        struct_size: std::mem::size_of::<ChuModAPI>() as u32,
        log: None,
        aob_scan: Some(api_aob_scan),
        mem_read: Some(api_mem_read),
        mem_write: Some(api_mem_write),
        mem_fill: Some(api_mem_fill),
        hook_create: Some(api_hook_create),
        hook_enable: Some(api_hook_enable),
        hook_disable: Some(api_hook_disable),
        hook_remove: Some(api_hook_remove),
        register_service: Some(api_register_service),
        get_service: Some(api_get_service),
        publish: Some(api_publish),
        subscribe: Some(api_subscribe),
        rtti_find_vtable: Some(api_rtti_find_vtable),
        config_get_int: Some(api_config_get_int),
        config_get_float: Some(api_config_get_float),
        config_get_bool: Some(api_config_get_bool),
        config_get_string: Some(api_config_get_string),
        config_set_int: Some(api_config_set_int),
        config_set_float: Some(api_config_set_float),
        config_set_bool: Some(api_config_set_bool),
        config_set_string: Some(api_config_set_string),
    }
}

pub unsafe fn init() {
    G_API = make_api();
}

pub unsafe fn shutdown() {
    {
        let mut hooks = HOOKS.lock().unwrap();
        for (_addr, entry) in hooks.drain() {
            let _ = entry.detour.disable();
        }
    }
    SERVICES.lock().unwrap().clear();
    SUBSCRIBERS.lock().unwrap().clear();
}

pub unsafe fn get_api() -> *mut ChuModAPI {
    &raw mut G_API
}
