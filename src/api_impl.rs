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
