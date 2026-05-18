use std::ffi::c_void;

use crate::config::Config;
use crate::device_wrapper;

// IDirect3D9 vtable index
const CREATE_DEVICE_INDEX: usize = 16;

static mut ORIG_CREATE_DEVICE: usize = 0;
static mut D3D9_CONFIG: Option<Config> = None;

pub unsafe fn create(real: *mut c_void, config: Config) -> *mut c_void {
    D3D9_CONFIG = Some(config);
    patch_vtable(real);
    OutputDebugStringA(b"[d3d9proxy] create() called, vtable patched\0".as_ptr());
    real
}

unsafe fn patch_vtable(obj: *mut c_void) {
    let vtable_ptr = *(obj as *mut *mut usize);
    let slot = vtable_ptr.add(CREATE_DEVICE_INDEX);

    ORIG_CREATE_DEVICE = *slot;

    let mut old_protect = 0u32;
    VirtualProtect(
        slot.cast(),
        std::mem::size_of::<usize>(),
        0x40, // PAGE_EXECUTE_READWRITE
        &mut old_protect,
    );
    *slot = hooked_create_device as usize;
    let mut ignored = 0u32;
    VirtualProtect(
        slot.cast(),
        std::mem::size_of::<usize>(),
        old_protect,
        &mut ignored,
    );
}

// IDirect3D9::CreateDevice(UINT, D3DDEVTYPE, HWND, DWORD, D3DPRESENT_PARAMETERS*, IDirect3DDevice9**)
unsafe extern "system" fn hooked_create_device(
    this: *mut c_void,
    adapter: u32,
    device_type: u32,
    focus_window: usize,
    behavior_flags: u32,
    present_params: *mut c_void,
    returned_device: *mut *mut c_void,
) -> i32 {
    OutputDebugStringA(b"[d3d9proxy] hooked_create_device ENTERED\0".as_ptr());

    let orig: unsafe extern "system" fn(
        *mut c_void, u32, u32, usize, u32, *mut c_void, *mut *mut c_void,
    ) -> i32 = std::mem::transmute(ORIG_CREATE_DEVICE);

    let hr = orig(this, adapter, device_type, focus_window, behavior_flags, present_params, returned_device);

    if hr >= 0 && !returned_device.is_null() && !(*returned_device).is_null() {
        OutputDebugStringA(b"[d3d9proxy] CreateDevice OK, patching device\0".as_ptr());
        device_wrapper::GAME_HWND = focus_window;
        if let Some(cfg) = &D3D9_CONFIG {
            device_wrapper::patch(*returned_device, cfg.clone());
        }
    } else {
        OutputDebugStringA(b"[d3d9proxy] CreateDevice FAILED\0".as_ptr());
    }

    hr
}

#[link(name = "kernel32")]
extern "system" {
    fn VirtualProtect(addr: *mut c_void, size: usize, new_protect: u32, old_protect: *mut u32) -> i32;
    fn OutputDebugStringA(s: *const u8);
}
