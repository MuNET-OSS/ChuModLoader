use std::sync::atomic::{AtomicBool, Ordering};

use windows_sys::Win32::System::Threading::Sleep;

use super::log::{log_info, log_warn};
use super::seh::call_mod_on_frame;
use super::state::STATE;

static FRAME_THREAD_STARTED: AtomicBool = AtomicBool::new(false);
static FRAME_THREAD_RUNNING: AtomicBool = AtomicBool::new(false);

extern "system" {
    fn CreateThread(
        attrs: *const std::ffi::c_void,
        stack_size: usize,
        start: Option<unsafe extern "system" fn(*mut std::ffi::c_void) -> u32>,
        param: *mut std::ffi::c_void,
        flags: u32,
        id: *mut u32,
    ) -> *mut std::ffi::c_void;
}

pub fn start_if_needed() {
    let has_frame_mod = STATE
        .lock()
        .map(|state| state.mods.iter().any(|m| m.on_frame.is_some()))
        .unwrap_or(false);
    if !has_frame_mod {
        return;
    }

    if FRAME_THREAD_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    FRAME_THREAD_RUNNING.store(true, Ordering::SeqCst);
    unsafe {
        let handle = CreateThread(
            std::ptr::null(),
            0,
            Some(frame_thread),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
        );
        if handle.is_null() {
            FRAME_THREAD_RUNNING.store(false, Ordering::SeqCst);
            FRAME_THREAD_STARTED.store(false, Ordering::SeqCst);
            log_warn("failed to start chumod_on_frame fallback thread");
            return;
        }
    }
    log_info("chumod_on_frame fallback thread started (16ms interval)");
}

pub fn stop() {
    FRAME_THREAD_RUNNING.store(false, Ordering::SeqCst);
    FRAME_THREAD_STARTED.store(false, Ordering::SeqCst);
}

unsafe extern "system" fn frame_thread(_param: *mut std::ffi::c_void) -> u32 {
    while FRAME_THREAD_RUNNING.load(Ordering::SeqCst) {
        tick();
        Sleep(16);
    }
    0
}

pub unsafe fn tick() {
    if super::hot_reload::is_reloading() {
        return;
    }

    // 复制回调后释放锁，避免 Mod 回调里调用 Loader API 时发生锁重入。
    let frame_mods: Vec<_> = STATE
        .lock()
        .map(|state| {
            state
                .mods
                .iter()
                .filter_map(|m| m.on_frame.map(|on_frame| (m.name.clone(), on_frame)))
                .collect()
        })
        .unwrap_or_default();

    for (name, on_frame) in frame_mods {
        call_mod_on_frame(&name, on_frame);
    }
}
