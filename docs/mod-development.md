# Mod Development Guide

## Overview

Mod = Win32 DLL (32-bit), placed in `mods/`. Loader picks them up on startup.

Three approaches:

1. **Rust DLL** — write in Rust, export C ABI interfaces.
2. **C/C++ ChuMod API** — export `chumod_init` etc. to get game info and tool API.
3. **Plain DLL** — `DllMain` only.

Can be mixed.

## Minimal Example (Plain DLL)

### Rust

```rust
use std::ffi::c_void;

const DLL_PROCESS_ATTACH: u32 = 1;

extern "system" {
    fn MessageBoxA(hwnd: *mut c_void, text: *const u8, caption: *const u8, flags: u32) -> i32;
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(_h: *mut c_void, reason: u32, _lp: *const c_void) -> i32 {
    if reason == DLL_PROCESS_ATTACH {
        MessageBoxA(std::ptr::null_mut(), b"Mod loaded!\0".as_ptr(), b"Hello\0".as_ptr(), 0);
    }
    1
}
```

Cargo.toml:

```toml
[package]
name = "my-mod"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]
```

.cargo/config.toml:

```toml
[build]
target = "i686-pc-windows-msvc"
```

Drop the compiled DLL into `mods/`.

### C/C++

```c
#include <Windows.h>

BOOL APIENTRY DllMain(HMODULE h, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        MessageBoxA(NULL, "Mod loaded!", "Hello", MB_OK);
    }
    return TRUE;
}
```

## ChuMod API Example

### Rust

```rust
use std::ffi::{c_char, c_void};

#[repr(C)]
pub struct ChuModInfo {
    pub api_version: u32,
    pub loader_version: *const c_char,
    pub game_module: *const c_char,
    pub game_base: usize,
    pub game_size: u32,
    pub text_base: usize,
    pub text_size: u32,
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
    pub subscribe: Option<unsafe extern "C" fn(*const c_char, Option<unsafe extern "C" fn(*const c_char, *mut c_void, u32)>) -> i32>,
}

static mut G_API: *const ChuModAPI = std::ptr::null();

#[no_mangle]
pub extern "C" fn chumod_name() -> *const c_char {
    b"Example Rust Mod\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "C" fn chumod_init(info: *const ChuModInfo, api: *const ChuModAPI) -> i32 {
    G_API = api;
    if let Some(log) = (*api).log {
        log(b"Rust mod loaded!\0".as_ptr() as *const c_char);
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn chumod_shutdown() {
    if !G_API.is_null() {
        if let Some(log) = (*G_API).log {
            log(b"Rust mod shutdown\0".as_ptr() as *const c_char);
        }
    }
}
```

### C/C++

```c
#include "chumod.h"

static const ChuModAPI* g_api = NULL;

CHUMOD_API const char* chumod_name() { return "Example Mod"; }

CHUMOD_API int chumod_init(const ChuModInfo* info, const ChuModAPI* api) {
    g_api = api;
    api->log("game: %s @ 0x%08X (size 0x%X)",
             info->game_module, info->game_base, info->game_size);
    api->log(".text @ 0x%08X (size 0x%X)", info->text_base, info->text_size);
    return 0;
}

CHUMOD_API void chumod_shutdown() {
    if (g_api) g_api->log("bye");
}
```

### Lifecycle

```
LoadLibrary(mod.dll)
  → DllMain(DLL_PROCESS_ATTACH)
  → chumod_name()        [optional, for log display]
  → chumod_init(info, api)  [optional, return 0 = success]
       ↓
   (game runs)
       ↓
  → chumod_shutdown()    [optional, cleanup]
  → DllMain(DLL_PROCESS_DETACH)
  → FreeLibrary
```

If `chumod_init` returns non-zero, the loader unloads the mod immediately.

## Using the API

The `ChuModAPI*` from `chumod_init` is the tool function table. Store it globally.

### Pattern Scanning

#### Rust

```rust
unsafe fn find_pattern(api: &ChuModAPI, base: usize, size: u32, pattern: &[u8], mask: &str) -> Option<usize> {
    if let Some(aob_scan) = api.aob_scan {
        let mask_c = format!("{}\0", mask);
        let addr = aob_scan(base, size, pattern.as_ptr(), mask_c.as_ptr() as *const c_char);
        if addr != 0 { Some(addr) } else { None }
    } else {
        None
    }
}
```

#### C/C++

```c
const uint8_t sig[] = { 0x55, 0x8B, 0xEC, 0x83, 0xEC };
uintptr_t addr = g_api->aob_scan(info->text_base, info->text_size, sig, "xxxxx");
if (addr) {
    g_api->log("found at 0x%08X", addr);
}
```

### Memory Read/Write

#### Rust

```rust
unsafe fn read_memory<T: Copy>(api: &ChuModAPI, addr: usize) -> Option<T> {
    let mut value: T = std::mem::zeroed();
    if let Some(mem_read) = api.mem_read {
        if mem_read(addr, &mut value as *mut _ as *mut c_void, std::mem::size_of::<T>() as u32) == 0 {
            return Some(value);
        }
    }
    None
}

unsafe fn write_memory<T>(api: &ChuModAPI, addr: usize, value: &T) -> bool {
    if let Some(mem_write) = api.mem_write {
        mem_write(addr, value as *const _ as *const c_void, std::mem::size_of::<T>() as u32) == 0
    } else {
        false
    }
}
```

#### C/C++

```c
uint32_t value;
g_api->mem_read(0x12345678, &value, sizeof(value));

uint8_t nop = 0x90;
g_api->mem_write(0x12345678, &nop, 1);

g_api->mem_fill(0x12345678, 0x90, 5);  // NOP 5 bytes
```

Page protection handled automatically.

### Hooking

#### Rust

```rust
type OrigFunc = unsafe extern "stdcall" fn(i32, i32) -> i32;
static mut ORIG: Option<OrigFunc> = None;

unsafe extern "stdcall" fn my_hook(a: i32, b: i32) -> i32 {
    ORIG.unwrap()(a, b)
}

unsafe fn install_hook(api: &ChuModAPI, target: *mut c_void) {
    let mut orig_ptr: *mut c_void = std::ptr::null_mut();
    if let Some(hook_create) = api.hook_create {
        hook_create(target, my_hook as *mut c_void, &mut orig_ptr);
        ORIG = Some(std::mem::transmute(orig_ptr));
    }
    if let Some(hook_enable) = api.hook_enable {
        hook_enable(target);
    }
}
```

#### C/C++

```c
typedef int (__stdcall *OrigFunc_t)(int a, int b);
static OrigFunc_t orig = NULL;

int __stdcall my_hook(int a, int b) {
    g_api->log("called with %d, %d", a, b);
    return orig(a, b);
}

// in chumod_init:
g_api->hook_create((void*)target_addr, (void*)my_hook, (void**)&orig);
g_api->hook_enable((void*)target_addr);

// to remove later:
g_api->hook_disable((void*)target_addr);
g_api->hook_remove((void*)target_addr);
```

### Inter-Mod Communication

**Services** — named pointer registry:

#### Rust

```rust
#[repr(C)]
pub struct MyService {
    pub version: i32,
    pub do_thing: unsafe extern "C" fn(),
}

static mut SVC: MyService = MyService { version: 1, do_thing: my_func };

unsafe fn register(api: &ChuModAPI) {
    if let Some(register_service) = api.register_service {
        register_service(b"my_service\0".as_ptr() as *const c_char, &mut SVC as *mut _ as *mut c_void);
    }
}

unsafe fn consume(api: &ChuModAPI) {
    if let Some(get_service) = api.get_service {
        let ptr = get_service(b"my_service\0".as_ptr() as *const c_char);
        if !ptr.is_null() {
            let svc = &*(ptr as *const MyService);
            (svc.do_thing)();
        }
    }
}
```

#### C/C++

```c
// Mod A: register
struct MyService { int version; void (*do_thing)(void); };
static struct MyService svc = { 1, my_func };
g_api->register_service("my_service", &svc);

// Mod B: consume
struct MyService* s = (struct MyService*)g_api->get_service("my_service");
if (s) s->do_thing();
```

**Messages** — publish/subscribe:

#### Rust

```rust
unsafe extern "C" fn on_score(_topic: *const c_char, data: *mut c_void, _size: u32) {
    let score = *(data as *const i32);
    // handle score
}

unsafe fn setup_messages(api: &ChuModAPI) {
    if let Some(subscribe) = api.subscribe {
        subscribe(b"score_update\0".as_ptr() as *const c_char, Some(on_score));
    }
}

unsafe fn send_score(api: &ChuModAPI, score: i32) {
    if let Some(publish) = api.publish {
        publish(b"score_update\0".as_ptr() as *const c_char, &score as *const _ as *mut c_void, 4);
    }
}
```

#### C/C++

```c
// subscriber
void on_score(const char* topic, void* data, uint32_t size) {
    int score = *(int*)data;
}
g_api->subscribe("score_update", on_score);

// publisher
int score = 1010000;
g_api->publish("score_update", &score, sizeof(score));
```

## Dual Mode (Loader + inject -k)

Compatible with both loader and `inject -k` injection (C/C++ only, uses chumod.h macros):

```c
#include "chumod.h"

static int my_init(const ChuModInfo* info, const ChuModAPI* api) {
    // your init code
    return 0;
}

CHUMOD_DUAL_MODE(my_init)

BOOL APIENTRY DllMain(HMODULE h, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        DisableThreadLibraryCalls(h);
        CHUMOD_DUAL_MODE_START();
    }
    return TRUE;
}
```

When loaded by the loader, `chumod_init` runs normally. Via `inject -k`, a fallback thread waits 3s then calls init with minimal `ChuModInfo`.

> In standalone mode all `ChuModAPI` function pointers are NULL. Check before calling.

## Dependencies

Declare dependencies, loader ensures load order:

```c
CHUMOD_API const char* chumod_depends() {
    return "base_mod,utility_mod";
}
```

The loader will ensure those mods are loaded before yours.

> Dependencies are matched by filename or `chumod_name()` return value.

## Build Setup

### Rust (recommended)

Cargo.toml:

```toml
[package]
name = "my-mod"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]
```

.cargo/config.toml:

```toml
[build]
target = "i686-pc-windows-msvc"
```

Build:

```bash
cargo build --release
```

Output: `target/i686-pc-windows-msvc/release/my_mod.dll`

### C/C++ (legacy)

Include `chumod.h` from ChuModLoader — copy it or add to include path. CMakeLists.txt example:

```cmake
cmake_minimum_required(VERSION 3.15)
project(my_mod LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)

add_library(my_mod SHARED src/main.cpp)
target_include_directories(my_mod PRIVATE path/to/ChuModLoader/include)

set_target_properties(my_mod PROPERTIES
    OUTPUT_NAME "my_mod"
    SUFFIX ".dll"
)
```

Build with `-A Win32` (game is 32-bit):

```bash
cmake -B build -A Win32
cmake --build build --config Release
```

Copy output DLL to `mods/`.

## Tips

- **Rust mods**: must use `crate-type = ["cdylib"]`, target `i686-pc-windows-msvc`, `#[no_mangle] extern "C"` for exports, `#[repr(C)]` for structs
- **C/C++ mods**: must use `-A Win32`, game is 32-bit
- Don't block `DllMain`, heavy work goes in `chumod_init` or a new thread
- Use AOB scan over hardcoded addresses — they change on game updates
- Clean up all hooks and resources in `chumod_shutdown`
- API pointers may be NULL in dual mode

## See Also

- [API Reference](api-reference.md)
- [chumod.h](../include/chumod.h)
