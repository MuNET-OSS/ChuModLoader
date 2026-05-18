use std::ffi::c_void;

use crate::config::Config;

pub type PresentCallbackFn = unsafe extern "C" fn(*mut c_void);

#[repr(C)]
pub struct D3D9ProxyAPI {
    pub set_frame_lock: unsafe extern "C" fn(u32),
    pub get_device: unsafe extern "C" fn() -> *mut c_void,
    pub get_hwnd: unsafe extern "C" fn() -> usize,
    pub register_present_callback: unsafe extern "C" fn(PresentCallbackFn),
}

static API_TABLE: D3D9ProxyAPI = D3D9ProxyAPI {
    set_frame_lock: api_set_frame_lock,
    get_device: api_get_device,
    get_hwnd: api_get_hwnd,
    register_present_callback: api_register_present_callback,
};

static mut DEVICE_PTR: *mut c_void = std::ptr::null_mut();
static mut PRESENT_CALLBACKS: [Option<PresentCallbackFn>; 8] = [None; 8];
static mut PENDING: PendingConfig = PendingConfig::new();

struct PendingConfig {
    frame_lock: Option<u32>,
}

impl PendingConfig {
    const fn new() -> Self {
        Self { frame_lock: None }
    }
}

pub fn set_device(device: *mut c_void) {
    unsafe { DEVICE_PTR = device; }
}

pub fn apply_pending(config: &mut Config) {
    unsafe {
        if let Some(fps) = PENDING.frame_lock {
            config.frame_lock = Some(fps);
        }
    }
}

pub fn run_present_callbacks(device: *mut c_void) {
    unsafe {
        for cb in &PRESENT_CALLBACKS {
            if let Some(f) = cb {
                f(device);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn d3d9proxy_get_api() -> *const D3D9ProxyAPI {
    &API_TABLE
}

unsafe extern "C" fn api_set_frame_lock(fps: u32) {
    PENDING.frame_lock = if fps > 0 { Some(fps) } else { None };
    if let Some(cfg) = &mut crate::device_wrapper::DEVICE_CONFIG {
        cfg.frame_lock = if fps > 0 { Some(fps) } else { None };
    }
}

unsafe extern "C" fn api_get_device() -> *mut c_void {
    DEVICE_PTR
}

unsafe extern "C" fn api_get_hwnd() -> usize {
    crate::device_wrapper::GAME_HWND
}

unsafe extern "C" fn api_register_present_callback(cb: PresentCallbackFn) {
    for slot in &mut PRESENT_CALLBACKS {
        if slot.is_none() {
            *slot = Some(cb);
            return;
        }
    }
}
