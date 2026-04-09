# API Reference

`chumod.h` — ChuModLoader Mod API v1.0.0. The loader is written in Rust, and mods can be written in Rust, C/C++, or any language that produces a Win32 DLL.

## Constants

| Name | Value | Description |
|------|-------|-------------|
| `CHUMOD_API_VERSION` | `1` | Current API version |
| `CHUMOD_API` | `__declspec(dllexport)` | Export marker for mod functions |

## ChuModInfo

Passed to `chumod_init`. Game process information.

Rust equivalent:

```rust
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
```

```c
typedef struct {
    uint32_t    api_version;     // CHUMOD_API_VERSION
    const char* loader_version;  // e.g. "1.0.0", or "standalone" in dual mode
    const char* game_module;     // "chusanApp.exe"
    uintptr_t   game_base;       // Base address of game module
    uint32_t    game_size;       // SizeOfImage from PE header
    uintptr_t   text_base;       // .text section start
    uint32_t    text_size;       // .text section size
} ChuModInfo;
```

`text_base` and `text_size` are useful for limiting AOB scans to executable code.

## ChuModAPI

Function table from `chumod_init`. Store globally.

Rust equivalent:

```rust
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
```

Return values: `0` on success, non-zero on failure (unless noted).

---

### Rust Types

Full Rust struct definitions for `std::ffi` compatibility:

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
```

---

### Logging

#### `log(const char* fmt, ...)`

Printf-style logging. Output to `chusan_loader.log` and console.

```c
api->log("player score: %d", score);
api->log("hook target: 0x%08X", addr);
```

> `log` is NULL in dual-mode standalone fallback.

---

### Memory — Pattern Scanning

#### `aob_scan(uintptr_t start, uint32_t size, const uint8_t* pattern, const char* mask) → uintptr_t`

Scans for a byte pattern. Returns first match address, or `0` if not found.

- `start` — scan start address (typically `info->text_base`)
- `size` — number of bytes to scan (typically `info->text_size`)
- `pattern` — byte array to match
- `mask` — character mask, same length as pattern:
  - `'x'` — exact match
  - `'?'` — wildcard

```c
const uint8_t sig[] = { 0x55, 0x8B, 0xEC, 0x83, 0xEC };
uintptr_t addr = api->aob_scan(info->text_base, info->text_size, sig, "xxxxx");
```

---

### Memory — Read / Write / Fill

Page protection handled automatically (`VirtualProtect`).

#### `mem_read(uintptr_t addr, void* buf, uint32_t size) → int`

```c
uint32_t value;
api->mem_read(target, &value, sizeof(value));
```

#### `mem_write(uintptr_t addr, const void* buf, uint32_t size) → int`

```c
uint8_t jmp = 0xEB;
api->mem_write(branch_addr, &jmp, 1);
```

#### `mem_fill(uintptr_t addr, uint8_t value, uint32_t size) → int`

```c
api->mem_fill(call_addr, 0x90, 5);
```

---

### Hooking

Based on [retour](https://crates.io/crates/retour). Call `hook_enable` after `hook_create`.

#### `hook_create(void* target, void* detour, void** original) → int`

#### `hook_enable(void* target) → int`

#### `hook_disable(void* target) → int`

#### `hook_remove(void* target) → int`

**Full example:**

```c
typedef int (__stdcall *CheckFunc_t)(void* self);
static CheckFunc_t orig_check = NULL;

int __stdcall hook_check(void* self) {
    return orig_check(self);
}

api->hook_create((void*)target, (void*)hook_check, (void**)&orig_check);
api->hook_enable((void*)target);

// cleanup
api->hook_disable((void*)target);
api->hook_remove((void*)target);
```

---

### Inter-Mod Communication — Services

Named pointer registry. Thread-safe.

#### `register_service(const char* name, void* service_ptr) → int`

#### `get_service(const char* name) → void*`

Returns registered pointer, or `NULL` if not found.

**Example:**

```c
// --- provider mod ---
struct ScoreService {
    int version;
    int (*get_score)(void);
};

static struct ScoreService svc = { 1, my_get_score };
api->register_service("score_service", &svc);

// --- consumer mod ---
struct ScoreService* s = (struct ScoreService*)api->get_service("score_service");
if (s && s->version >= 1) {
    int score = s->get_score();
}
```

> Use `chumod_depends` to ensure load order when depending on another mod's service.

---

### Inter-Mod Communication — Messages

Publish/subscribe message bus. Synchronous dispatch, thread-safe registration.

#### `publish(const char* topic, void* data, uint32_t size) → int`

Sends data to all subscribers of `topic` synchronously.

#### `subscribe(const char* topic, ChuModMessageCallback callback) → int`

**Callback:**

```c
void callback(const char* topic, void* data, uint32_t size);
```

**Example:**

```c
// subscriber
void on_event(const char* topic, void* data, uint32_t size) {
    int value = *(int*)data;
    api->log("received %s: %d", topic, value);
}
api->subscribe("game_event", on_event);

// publisher
int payload = 42;
api->publish("game_event", &payload, sizeof(payload));
```

---

## Mod Export Functions

All optional. Loader checks via `GetProcAddress`.

### `chumod_name() → const char*`

Display name for log output. Falls back to DLL filename.

### `chumod_init(const ChuModInfo* info, const ChuModAPI* api) → int`

Init entry point. Return `0` for success, non-zero to unload.

### `chumod_shutdown()`

Cleanup on exit. Called in reverse load order.

### `chumod_depends() → const char*`

Comma-separated dependency list. Loader ensures they load first.

```c
CHUMOD_API const char* chumod_depends() {
    return "base_mod,utility_mod";
}
```

---

## Dual Mode Macros

For mods that work with both loader and `inject -k`.

### `CHUMOD_DUAL_MODE(init_func)`

Generates `chumod_init` export and fallback thread. In standalone injection, waits 3s then calls `init_func` with `ChuModAPI` fields all NULL.

### `CHUMOD_DUAL_MODE_START()`

Call in `DllMain` `DLL_PROCESS_ATTACH`.

```c
#include "chumod.h"

static int my_init(const ChuModInfo* info, const ChuModAPI* api) {
    if (api->log) api->log("hello");  // check NULL in dual mode
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

---

## Symbol Names

For manual `GetProcAddress` or DEF files:

| Export | String Constant |
|--------|----------------|
| `chumod_init` | `CHUMOD_INIT_NAME` |
| `chumod_shutdown` | `CHUMOD_SHUTDOWN_NAME` |
| `chumod_name` | `CHUMOD_NAME_NAME` |
| `chumod_depends` | `CHUMOD_DEPENDS_NAME` |
