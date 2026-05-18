use std::ffi::c_void;

use retour::GenericDetour;

use crate::config::Config;
use crate::mod_api;
use crate::overlay;

// IDirect3DDevice9 vtable index
const PRESENT_INDEX: usize = 17;
const END_SCENE_INDEX: usize = 42;

type PresentFn = unsafe extern "system" fn(
    *mut c_void, *const c_void, *const c_void, usize, *const c_void,
) -> i32;
type EndSceneFn = unsafe extern "system" fn(*mut c_void) -> i32;

pub static mut DEVICE_CONFIG: Option<Config> = None;
pub static mut FPS_STATE: Option<overlay::FpsState> = None;
pub static mut GAME_HWND: usize = 0;

static mut PRESENT_HOOK: Option<GenericDetour<PresentFn>> = None;
static mut END_SCENE_HOOK: Option<GenericDetour<EndSceneFn>> = None;

pub unsafe fn patch(device: *mut c_void, mut config: Config) {
    mod_api::apply_pending(&mut config);
    let frame_lock = config.frame_lock;
    DEVICE_CONFIG = Some(config);
    if frame_lock.is_some() {
        FPS_STATE = Some(overlay::FpsState::new());
    }
    mod_api::set_device(device);

    let vtable = *(device as *const *const usize);
    let present_addr = *vtable.add(PRESENT_INDEX);
    let end_scene_addr = *vtable.add(END_SCENE_INDEX);

    let present_fn: PresentFn = std::mem::transmute(present_addr);
    if let Ok(hook) = GenericDetour::<PresentFn>::new(present_fn, hooked_present) {
        let _ = hook.enable();
        PRESENT_HOOK = Some(hook);
    }

    let end_scene_fn: EndSceneFn = std::mem::transmute(end_scene_addr);
    if let Ok(hook) = GenericDetour::<EndSceneFn>::new(end_scene_fn, hooked_end_scene) {
        let _ = hook.enable();
        END_SCENE_HOOK = Some(hook);
    }
}

// IDirect3DDevice9::Present(RECT*, RECT*, HWND, RGNDATA*)
unsafe extern "system" fn hooked_present(
    this: *mut c_void,
    source_rect: *const c_void,
    dest_rect: *const c_void,
    dest_window_override: usize,
    dirty_region: *const c_void,
) -> i32 {
    if let Some(state) = &mut FPS_STATE {
        if let Some(cfg) = &DEVICE_CONFIG {
            if let Some(target_fps) = cfg.frame_lock {
                state.frame_lock(target_fps);
            }
        }
    }

    PRESENT_HOOK.as_ref().unwrap().call(this, source_rect, dest_rect, dest_window_override, dirty_region)
}

// IDirect3DDevice9::EndScene()
unsafe extern "system" fn hooked_end_scene(this: *mut c_void) -> i32 {
    mod_api::run_present_callbacks(this);

    END_SCENE_HOOK.as_ref().unwrap().call(this)
}
