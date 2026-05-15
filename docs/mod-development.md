# ChuModLoader Mod Development Guide (v2.5.0)

ChuModLoader is a `version.dll` proxy that loads mod DLLs from `mods/` when `chusanApp.exe` starts. Mods can be written in several ways and can opt into progressively richer APIs.

## Three ways to write a mod

1. **Rust mod**: build a Win32 DLL exporting `extern "C"` functions with `#[no_mangle]`.
2. **C/C++ mod**: include `include/chumod.h`, export `CHUMOD_API` functions, and call the provided `ChuModAPI` table.
3. **Plain DLL**: provide only `DllMain`. ChuModLoader still loads the DLL, but no API callbacks are invoked unless the matching exports exist.

All new exports are optional. Existing mods that only provide `chumod_init`, `chumod_shutdown`, and `chumod_name` continue to work.

## Loader lifecycle

For API-aware mods, the lifecycle is:

```text
LoadLibrary(mod.dll)
  -> optional metadata/dependency exports are read
  -> chumod_init(info, api)
  -> after every successful mod init: chumod_on_ready()
  -> game keeps running
  -> chumod_shutdown()
  -> FreeLibrary(mod.dll)
```

Use `chumod_init` for local setup and hook creation. Use `chumod_on_ready` when you need other mods' services to exist. Use `chumod_shutdown` to disable hooks and release resources.

## Quick Start: Rust

```rust
use std::ffi::{c_char, c_void, CStr};

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
    pub hook_create: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32>,
    pub hook_enable: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub hook_disable: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub hook_remove: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub register_service: Option<unsafe extern "C" fn(*const c_char, *mut c_void) -> i32>,
    pub get_service: Option<unsafe extern "C" fn(*const c_char) -> *mut c_void>,
    pub publish: Option<unsafe extern "C" fn(*const c_char, *mut c_void, u32) -> i32>,
    pub subscribe: usize,
    pub rtti_find_vtable: Option<unsafe extern "C" fn(*const c_char) -> usize>,
    pub config_get_int: Option<unsafe extern "C" fn(*const c_char, i32) -> i32>,
    pub config_get_float: Option<unsafe extern "C" fn(*const c_char, f32) -> f32>,
    pub config_get_bool: Option<unsafe extern "C" fn(*const c_char, i32) -> i32>,
    pub config_get_string: usize,
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
    pub toml_get_string: usize,
    pub get_manifest_path: Option<unsafe extern "C" fn() -> *const c_char>,
}

static mut API: *const ChuModAPI = std::ptr::null();

#[no_mangle]
pub extern "C" fn chumod_name() -> *const c_char {
    b"Rust Example\0".as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn chumod_version() -> *const c_char {
    b"1.0.0\0".as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn chumod_min_loader_version() -> *const c_char {
    b"2.5.0\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "C" fn chumod_init(info: *const ChuModInfo, api: *const ChuModAPI) -> i32 {
    API = api;
    let api_ref = &*api;
    if let Some(log_info) = api_ref.log_info {
        log_info(b"Rust mod initialized\0".as_ptr() as *const c_char);
    }
    if let Some(toml_get_bool) = api_ref.toml_get_bool {
        let enabled = toml_get_bool(
            b"config\0".as_ptr() as *const c_char,
            b"enabled\0".as_ptr() as *const c_char,
            1,
        );
        if enabled == 0 {
            return 1;
        }
    }
    if !(*info).game_version.is_null() {
        let _version = CStr::from_ptr((*info).game_version).to_string_lossy();
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn chumod_on_ready() {
    if !API.is_null() {
        if let Some(log_info) = (*API).log_info {
            log_info(b"All mods are ready\0".as_ptr() as *const c_char);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn chumod_shutdown() {
    if !API.is_null() {
        if let Some(log_info) = (*API).log_info {
            log_info(b"Rust mod shutdown\0".as_ptr() as *const c_char);
        }
    }
}
```

Build as a 32-bit Windows DLL and place it in `mods/`.

## Quick Start: C++

```cpp
#include "chumod.h"

static const ChuModAPI* g_api = nullptr;

CHUMOD_API const char* chumod_name() { return "C++ Example"; }
CHUMOD_API const char* chumod_version() { return "1.0.0"; }
CHUMOD_API const char* chumod_author() { return "Example Team"; }
CHUMOD_API const char* chumod_min_loader_version() { return "2.5.0"; }

CHUMOD_API int chumod_init(const ChuModInfo* info, const ChuModAPI* api) {
    g_api = api;
    api->log_info("C++ mod initialized");
    api->log("game base=0x%08X text=0x%08X size=0x%X version=%s",
             (unsigned)info->game_base,
             (unsigned)info->text_base,
             info->text_size,
             info->game_version ? info->game_version : "");

    if (api->toml_section_exists && api->toml_section_exists("config")) {
        int enabled = api->toml_get_bool("config", "enabled", 1);
        if (!enabled) return 1;
    }

    return 0;
}

CHUMOD_API void chumod_on_ready() {
    if (g_api) g_api->log_info("All mods are ready");
}

CHUMOD_API void chumod_shutdown() {
    if (g_api) g_api->log_info("C++ mod shutdown");
}
```

## TOML configuration

ChuModLoader v2.5 can read per-mod TOML files from:

```text
mods/config/<mod_name>.toml
```

Example:

```toml
[config]
enabled = true
profile = "default"

[graphics]
target_fps = 120
scale = 1.25
show_overlay = false
```

Use the TOML API for structured config:

```c
int fps = api->toml_get_int("graphics", "target_fps", 60);
float scale = api->toml_get_float("graphics", "scale", 1.0f);
int overlay = api->toml_get_bool("graphics", "show_overlay", 0);

char profile[64];
api->toml_get_string("config", "profile", profile, sizeof(profile), "default");
```

Legacy INI APIs still use `mods/config/<mod_name>.ini` and the `[config]` section:

```ini
[config]
enabled=true
target_fps=120
```

If a TOML file exists, v2.5 loads it for TOML getters. INI setters still write INI files.

## `manifest.toml` format

Per-mod manifests live at:

```text
mods/manifest/<mod_name>.toml
```

Recommended format:

```toml
[mod]
name = "Example Mod"
version = "1.0.0"
author = "Example Team"
min_loader_version = "2.5.0"
depends = ["CoreMod", "SharedUi"]

[description]
en = "Example gameplay enhancement."
zh_cn = "示例玩法增强。"
```

The C ABI metadata exports are still authoritative for runtime loading. Manifests are useful for tools, launchers, and human-readable packaging. A mod can query its own manifest path:

```c
const char* path = api->get_manifest_path ? api->get_manifest_path() : NULL;
if (path) api->log_info(path);
```

## Dependency declaration

Export `chumod_depends` to request load ordering. Return comma-separated display names or file stems used by the dependency resolver.

```c
CHUMOD_API const char* chumod_depends() {
    return "CoreMod,SharedUi";
}
```

The loader sorts mods before calling `chumod_init`. If dependencies cannot be satisfied, the loader logs the problem and continues with a best-effort order.

## Crash protection

ChuModLoader wraps `chumod_init`, `chumod_on_ready`, and `chumod_shutdown` with Rust `catch_unwind`. This protects against Rust panics crossing the loader boundary. It does **not** make arbitrary memory faults safe: access violations in native code can still crash the process.

Guidelines:

- Validate addresses before patching.
- Disable hooks in `chumod_shutdown`.
- Keep exported callbacks small and deterministic.
- Do not throw C++ exceptions across the C ABI boundary.

## Leveled logging

v2.5 adds plain leveled logging:

```c
api->log_info("normal status");
api->log_warn("recoverable problem");
api->log_error("operation failed");
```

Use `api->log` when formatting is convenient, but prefer the plain APIs for cross-language mods. During `chumod_init`, `api->log_path` points to the mod-specific log path under `mods/log/`.

## Dual Mode

`CHUMOD_DUAL_MODE(init_func)` lets one DLL work both with ChuModLoader and as a standalone injected DLL.

```cpp
static int my_init(const ChuModInfo* info, const ChuModAPI* api) {
    if (api && api->log_info) api->log_info("init");
    return 0;
}

CHUMOD_DUAL_MODE(my_init);

BOOL APIENTRY DllMain(HMODULE, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        CHUMOD_DUAL_MODE_START();
    }
    return TRUE;
}
```

In standalone mode, the fallback `ChuModAPI` only contains `struct_size`; most function pointers are `NULL`. Always check pointers before calling.

## Minimum loader version

Export `chumod_min_loader_version` if the mod requires APIs added in a specific loader version.

```c
CHUMOD_API const char* chumod_min_loader_version() {
    return "2.5.0";
}
```

Also guard individual fields with `struct_size` and null checks when distributing binaries to users with unknown loader versions.
