mod iat_hook;

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use chu_abi::{ChuModPresentCallback, ChuModResetCallback};

use crate::loader::log::{log_info, log_warn};

use self::iat_hook::{hook_iat, patch_slot, vtable_slot};

const D3D9_DLL_NAME: &str = "d3d9.dll";
const DIRECT3D_CREATE9_NAME: &str = "Direct3DCreate9";

const D3D9_CREATE_DEVICE_INDEX: usize = 16;
const DEVICE_TEST_COOPERATIVE_LEVEL_INDEX: usize = 3;
const DEVICE_RESET_INDEX: usize = 16;
const DEVICE_PRESENT_INDEX: usize = 17;
const DEVICE_END_SCENE_INDEX: usize = 42;

const D3DERR_DEVICELOST: i32 = 0x8876_0868u32 as i32;
const D3DERR_DEVICENOTRESET: i32 = 0x8876_0869u32 as i32;
const D3D_OK: i32 = 0;
const MAX_RECOVERY_ATTEMPTS: usize = 600;
const RECOVERY_WAIT_MS: u64 = 50;

const MAX_PRESENT_CALLBACKS: usize = 8;
const MAX_RESET_CALLBACKS: usize = 8;

type Direct3DCreate9Fn = unsafe extern "system" fn(u32) -> *mut c_void;
type CreateDeviceFn = unsafe extern "system" fn(
    *mut c_void, u32, u32, usize, u32, *mut c_void, *mut *mut c_void,
) -> i32;
type PresentFn = unsafe extern "system" fn(
    *mut c_void, *const c_void, *const c_void, usize, *const c_void,
) -> i32;
type ResetFn = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;
type EndSceneFn = unsafe extern "system" fn(*mut c_void) -> i32;
type TestCooperativeLevelFn = unsafe extern "system" fn(*mut c_void) -> i32;

static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
static ORIGINAL_DIRECT3D_CREATE9: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_CREATE_DEVICE: AtomicUsize = AtomicUsize::new(0);
static DEVICE_HOOKED: AtomicBool = AtomicBool::new(false);

static ORIGINAL_PRESENT: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_RESET: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_END_SCENE: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_TEST_COOPERATIVE_LEVEL: AtomicUsize = AtomicUsize::new(0);

static DEVICE_PTR: AtomicUsize = AtomicUsize::new(0);
static GAME_HWND: AtomicUsize = AtomicUsize::new(0);
static PRESENTATION_PARAMETERS: AtomicUsize = AtomicUsize::new(0);
static RECOVERING: AtomicBool = AtomicBool::new(false);

static FRAME_LOCK_FPS: AtomicU32 = AtomicU32::new(0);
static QPC_FREQ: AtomicUsize = AtomicUsize::new(0);
static LAST_FRAME_QPC: AtomicUsize = AtomicUsize::new(0);

static PRESENT_CALLBACKS: [AtomicUsize; MAX_PRESENT_CALLBACKS] = [
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
];
static RESET_CALLBACKS: [AtomicUsize; MAX_RESET_CALLBACKS] = [
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
];

#[link(name = "kernel32")]
extern "system" {
    fn QueryPerformanceCounter(count: *mut i64) -> i32;
    fn QueryPerformanceFrequency(freq: *mut i64) -> i32;
}

pub unsafe fn install_early(game_base: usize) {
    if HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }

    let mut freq = 0i64;
    QueryPerformanceFrequency(&mut freq);
    QPC_FREQ.store(freq as usize, Ordering::SeqCst);

    match hook_iat(
        game_base,
        D3D9_DLL_NAME,
        DIRECT3D_CREATE9_NAME,
        hooked_direct3d_create9 as *const (),
    ) {
        Some(original) => {
            ORIGINAL_DIRECT3D_CREATE9.store(original as usize, Ordering::SeqCst);
            log_info("d3d9: Direct3DCreate9 IAT hook installed");
        }
        None => {
            HOOK_INSTALLED.store(false, Ordering::SeqCst);
            log_warn("d3d9: Direct3DCreate9 import not found (IAT hook skipped)");
        }
    }
}

unsafe extern "system" fn hooked_direct3d_create9(sdk_version: u32) -> *mut c_void {
    let addr = ORIGINAL_DIRECT3D_CREATE9.load(Ordering::SeqCst);
    if addr == 0 {
        return std::ptr::null_mut();
    }
    let original: Direct3DCreate9Fn = std::mem::transmute(addr);
    let d3d = original(sdk_version);
    if !d3d.is_null() {
        hook_create_device(d3d);
    }
    d3d
}

unsafe fn hook_create_device(d3d: *mut c_void) {
    if ORIGINAL_CREATE_DEVICE.load(Ordering::SeqCst) != 0 {
        return;
    }
    let slot = vtable_slot(d3d, D3D9_CREATE_DEVICE_INDEX);
    if slot.is_null() {
        return;
    }
    let current = *slot;
    let detour = hooked_create_device as *const () as usize;
    if current == detour {
        return;
    }
    if patch_slot(slot, detour) {
        ORIGINAL_CREATE_DEVICE.store(current, Ordering::SeqCst);
        log_info("d3d9: IDirect3D9::CreateDevice vtable hook installed");
    } else {
        log_warn("d3d9: CreateDevice vtable write failed");
    }
}

unsafe extern "system" fn hooked_create_device(
    this: *mut c_void,
    adapter: u32,
    device_type: u32,
    focus_window: usize,
    behavior_flags: u32,
    presentation_parameters: *mut c_void,
    returned_device_interface: *mut *mut c_void,
) -> i32 {
    let addr = ORIGINAL_CREATE_DEVICE.load(Ordering::SeqCst);
    if addr == 0 {
        return -1;
    }
    let original: CreateDeviceFn = std::mem::transmute(addr);
    let result = original(
        this,
        adapter,
        device_type,
        focus_window,
        behavior_flags,
        presentation_parameters,
        returned_device_interface,
    );

    if result >= 0 && !returned_device_interface.is_null() {
        let device = *returned_device_interface;
        if !device.is_null() {
            GAME_HWND.store(focus_window, Ordering::SeqCst);
            hook_device(device, presentation_parameters);
        }
    }
    result
}

unsafe fn hook_device(device: *mut c_void, presentation_parameters: *mut c_void) {
    if DEVICE_HOOKED.swap(true, Ordering::SeqCst) {
        return;
    }
    DEVICE_PTR.store(device as usize, Ordering::SeqCst);
    PRESENTATION_PARAMETERS.store(presentation_parameters as usize, Ordering::SeqCst);

    remember_slot(device, DEVICE_TEST_COOPERATIVE_LEVEL_INDEX, &ORIGINAL_TEST_COOPERATIVE_LEVEL);

    hook_slot(device, DEVICE_RESET_INDEX, hooked_reset as *const () as usize, &ORIGINAL_RESET, "Reset");
    hook_slot(device, DEVICE_PRESENT_INDEX, hooked_present as *const () as usize, &ORIGINAL_PRESENT, "Present");
    hook_slot(device, DEVICE_END_SCENE_INDEX, hooked_end_scene as *const () as usize, &ORIGINAL_END_SCENE, "EndScene");
}

unsafe fn remember_slot(device: *mut c_void, index: usize, original: &AtomicUsize) {
    if original.load(Ordering::SeqCst) != 0 {
        return;
    }
    let slot = vtable_slot(device, index);
    if !slot.is_null() {
        original.store(*slot, Ordering::SeqCst);
    }
}

unsafe fn hook_slot(device: *mut c_void, index: usize, detour: usize, original: &AtomicUsize, name: &str) {
    let slot = vtable_slot(device, index);
    if slot.is_null() {
        return;
    }
    let current = *slot;
    if current == detour || original.load(Ordering::SeqCst) != 0 {
        return;
    }
    if patch_slot(slot, detour) {
        original.store(current, Ordering::SeqCst);
        log_info(&format!("d3d9: IDirect3DDevice9::{name} vtable hook installed"));
    } else {
        log_warn(&format!("d3d9: {name} vtable write failed"));
    }
}

unsafe extern "system" fn hooked_present(
    this: *mut c_void,
    source_rect: *const c_void,
    dest_rect: *const c_void,
    dest_window_override: usize,
    dirty_region: *const c_void,
) -> i32 {
    frame_lock_wait();
    run_present_callbacks(this);

    let addr = ORIGINAL_PRESENT.load(Ordering::SeqCst);
    if addr == 0 {
        return D3DERR_DEVICELOST;
    }
    let original: PresentFn = std::mem::transmute(addr);
    let result = original(this, source_rect, dest_rect, dest_window_override, dirty_region);
    if result == D3DERR_DEVICELOST {
        recover_device(this);
    }
    result
}

unsafe extern "system" fn hooked_end_scene(this: *mut c_void) -> i32 {
    let addr = ORIGINAL_END_SCENE.load(Ordering::SeqCst);
    if addr == 0 {
        return D3D_OK;
    }
    let original: EndSceneFn = std::mem::transmute(addr);
    original(this)
}

unsafe extern "system" fn hooked_reset(this: *mut c_void, presentation_parameters: *mut c_void) -> i32 {
    if !presentation_parameters.is_null() {
        PRESENTATION_PARAMETERS.store(presentation_parameters as usize, Ordering::SeqCst);
    }
    run_reset_callbacks(this, 0);

    let addr = ORIGINAL_RESET.load(Ordering::SeqCst);
    if addr == 0 {
        return D3DERR_DEVICELOST;
    }
    let original: ResetFn = std::mem::transmute(addr);
    let result = original(this, presentation_parameters);
    if result == D3D_OK {
        run_reset_callbacks(this, 1);
    }
    result
}

unsafe fn recover_device(device: *mut c_void) {
    if device.is_null() || RECOVERING.swap(true, Ordering::SeqCst) {
        return;
    }
    log_info("d3d9: D3DERR_DEVICELOST detected, waiting for Reset");

    let test_addr = ORIGINAL_TEST_COOPERATIVE_LEVEL.load(Ordering::SeqCst);
    let reset_addr = ORIGINAL_RESET.load(Ordering::SeqCst);

    for _ in 0..MAX_RECOVERY_ATTEMPTS {
        let state = if test_addr != 0 {
            let test: TestCooperativeLevelFn = std::mem::transmute(test_addr);
            test(device)
        } else {
            D3DERR_DEVICELOST
        };

        if state == D3DERR_DEVICENOTRESET {
            let params = PRESENTATION_PARAMETERS.load(Ordering::SeqCst) as *mut c_void;
            if !params.is_null() && reset_addr != 0 {
                run_reset_callbacks(device, 0);
                let reset: ResetFn = std::mem::transmute(reset_addr);
                if reset(device, params) == D3D_OK {
                    run_reset_callbacks(device, 1);
                    log_info("d3d9: device recovered via Reset");
                } else {
                    log_warn("d3d9: Reset called but device not recovered");
                }
            }
            break;
        }

        if state == D3D_OK {
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(RECOVERY_WAIT_MS));
    }

    RECOVERING.store(false, Ordering::SeqCst);
}

unsafe fn run_present_callbacks(device: *mut c_void) {
    for slot in &PRESENT_CALLBACKS {
        let addr = slot.load(Ordering::SeqCst);
        if addr != 0 {
            let cb: ChuModPresentCallback = std::mem::transmute(addr);
            cb(device);
        }
    }
}

unsafe fn run_reset_callbacks(device: *mut c_void, phase: u32) {
    for slot in &RESET_CALLBACKS {
        let addr = slot.load(Ordering::SeqCst);
        if addr != 0 {
            let cb: ChuModResetCallback = std::mem::transmute(addr);
            cb(device, phase);
        }
    }
}

unsafe fn frame_lock_wait() {
    let fps = FRAME_LOCK_FPS.load(Ordering::SeqCst);
    let freq = QPC_FREQ.load(Ordering::SeqCst) as u64;
    if fps == 0 || freq == 0 {
        return;
    }
    let target_interval = freq / fps as u64;
    let last = LAST_FRAME_QPC.load(Ordering::SeqCst) as u64;
    loop {
        let mut now = 0i64;
        QueryPerformanceCounter(&mut now);
        let now = now as u64;
        if now.wrapping_sub(last) >= target_interval {
            LAST_FRAME_QPC.store(now as usize, Ordering::SeqCst);
            return;
        }
        std::hint::spin_loop();
    }
}

pub unsafe extern "C" fn api_register_present_callback(callback: Option<ChuModPresentCallback>) -> i32 {
    let Some(cb) = callback else { return -1 };
    let value = cb as usize;
    for slot in &PRESENT_CALLBACKS {
        if slot
            .compare_exchange(0, value, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return 0;
        }
    }
    -1
}

pub unsafe extern "C" fn api_register_reset_callback(callback: Option<ChuModResetCallback>) -> i32 {
    let Some(cb) = callback else { return -1 };
    let value = cb as usize;
    for slot in &RESET_CALLBACKS {
        if slot
            .compare_exchange(0, value, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return 0;
        }
    }
    -1
}

pub unsafe extern "C" fn api_set_frame_lock(fps: u32) -> i32 {
    FRAME_LOCK_FPS.store(fps, Ordering::SeqCst);
    0
}

pub unsafe extern "C" fn api_get_d3d9_device() -> *mut c_void {
    DEVICE_PTR.load(Ordering::SeqCst) as *mut c_void
}

pub unsafe extern "C" fn api_get_game_hwnd() -> usize {
    GAME_HWND.load(Ordering::SeqCst)
}
