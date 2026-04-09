# Mod 开发指南

## 概述

Mod = Win32 DLL（32 位），放到 `mods/` 目录。Loader 启动时自动加载。

三种写法：

1. **Rust DLL** — 用 Rust 编写，导出 C ABI 接口。
2. **C/C++ ChuMod API** — 导出 `chumod_init` 等函数，拿到游戏信息和工具 API。
3. **普通 DLL** — 只用 `DllMain`。

可以混用。

## 最简示例（普通 DLL）

### Rust

```rust
use std::ffi::c_void;
use windows::Win32::Foundation::{BOOL, HMODULE, TRUE};
use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxA, MB_OK};

#[no_mangle]
pub extern "stdcall" fn DllMain(_h: HMODULE, reason: u32, _lp: *const c_void) -> BOOL {
    if reason == 1 { // DLL_PROCESS_ATTACH
        unsafe {
            MessageBoxA(None, "Mod loaded!", "Hello", MB_OK);
        }
    }
    TRUE
}
```

Cargo.toml：

```toml
[package]
name = "my-mod"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
windows = { version = "0.52", features = ["Win32_Foundation", "Win32_System_LibraryLoader", "Win32_UI_WindowsAndMessaging"] }
```

.cargo/config.toml：

```toml
[build]
target = "i686-pc-windows-msvc"
```

编译后丢到 `mods/`。

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

## ChuMod API 示例

### Rust

```rust
use std::ffi::{c_char, c_void};
use std::slice;

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
    pub mem_read: Option<unsafe extern "C" fn(usize, *mut c_void, u32)>,
    pub mem_write: Option<unsafe extern "C" fn(usize, *const c_void, u32)>,
    pub mem_fill: Option<unsafe extern "C" fn(usize, u8, u32)>,
    pub aob_scan: Option<unsafe extern "C" fn(usize, u32, *const u8, *const c_char) -> usize>,
    pub hook_create: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32>,
    pub hook_remove: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub hook_enable: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub hook_disable: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub register_service: Option<unsafe extern "C" fn(*const c_char, *mut c_void)>,
    pub get_service: Option<unsafe extern "C" fn(*const c_char) -> *mut c_void>,
    pub subscribe: Option<unsafe extern "C" fn(*const c_char, *mut c_void)>,
    pub publish: Option<unsafe extern "C" fn(*const c_char, *const c_void, u32)>,
}

static mut G_API: Option<&'static ChuModAPI> = None;

#[no_mangle]
pub extern "C" fn chumod_name() -> *const c_char {
    b"Example Rust Mod\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "C" fn chumod_init(info: *const ChuModInfo, api: *const ChuModAPI) -> i32 {
    G_API = Some(&*api);
    
    let info = &*info;
    if let Some(log) = api.as_ref().unwrap().log {
        log("Rust mod loaded!\0".as_ptr() as *const c_char);
    }
    
    0
}

#[no_mangle]
pub unsafe extern "C" fn chumod_shutdown() {
    if let Some(api) = G_API {
        if let Some(log) = api.log {
            log("Rust mod shutdown\0".as_ptr() as *const c_char);
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

### 生命周期

```
LoadLibrary(mod.dll)
  → DllMain(DLL_PROCESS_ATTACH)
  → chumod_name()           [可选，用于日志显示]
  → chumod_init(info, api)  [可选，返回 0 = 成功]
       ↓
    （游戏运行中）
       ↓
   → chumod_shutdown()       [可选，清理资源]
   → DllMain(DLL_PROCESS_DETACH)
   → FreeLibrary
```

如果 `chumod_init` 返回非零值，loader 会立即卸载该 mod。

## 使用 API

`chumod_init` 的 `ChuModAPI*` 参数是工具函数表，全局保存即可。

### 特征码扫描

#### Rust

```rust
use std::ffi::CString;

unsafe fn find_pattern(api: &ChuModAPI, base: usize, size: u32, pattern: &[u8], mask: &str) -> Option<usize> {
    let mask_cstr = CString::new(mask).unwrap();
    if let Some(aob_scan) = api.aob_scan {
        let addr = aob_scan(base, size, pattern.as_ptr(), mask_cstr.as_ptr());
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

### 内存读写

#### Rust

```rust
unsafe fn read_memory<T>(api: &ChuModAPI, addr: usize) -> T {
    let mut value: T = std::mem::zeroed();
    if let Some(mem_read) = api.mem_read {
        mem_read(addr, &mut value as *mut _ as *mut c_void, std::mem::size_of::<T>() as u32);
    }
    value
}

unsafe fn write_memory<T>(api: &ChuModAPI, addr: usize, value: &T) {
    if let Some(mem_write) = api.mem_write {
        mem_write(addr, value as *const _ as *const c_void, std::mem::size_of::<T>() as u32);
    }
}

unsafe fn fill_memory(api: &ChuModAPI, addr: usize, value: u8, len: u32) {
    if let Some(mem_fill) = api.mem_fill {
        mem_fill(addr, value, len);
    }
}
```

#### C/C++

```c
uint32_t value;
g_api->mem_read(0x12345678, &value, sizeof(value));

uint8_t nop = 0x90;
g_api->mem_write(0x12345678, &nop, 1);

g_api->mem_fill(0x12345678, 0x90, 5);  // NOP 5 字节
```

页保护自动处理。

### Hook

#### Rust

```rust
type OrigFunc = unsafe extern "C" fn(i32, i32) -> i32;
static mut ORIG: Option<OrigFunc> = None;

unsafe extern "C" fn my_hook(a: i32, b: i32) -> i32 {
    if let Some(api) = G_API {
        if let Some(log) = api.log {
            log("hook called\0".as_ptr() as *const c_char);
        }
    }
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

// 在 chumod_init 中:
g_api->hook_create((void*)target_addr, (void*)my_hook, (void**)&orig);
g_api->hook_enable((void*)target_addr);

// 卸载:
g_api->hook_disable((void*)target_addr);
g_api->hook_remove((void*)target_addr);
```

### Mod 间通信

**服务** — 注册命名指针，其他 mod 按名字查找：

#### Rust

```rust
#[repr(C)]
pub struct MyService {
    pub version: i32,
    pub do_thing: unsafe extern "C" fn(),
}

unsafe fn register_my_service(api: &ChuModAPI) {
    static mut SVC: MyService = MyService {
        version: 1,
        do_thing: my_func,
    };
    if let Some(register_service) = api.register_service {
        let name = CString::new("my_service").unwrap();
        register_service(name.as_ptr(), &mut SVC as *mut _ as *mut c_void);
    }
}

unsafe fn use_service(api: &ChuModAPI) {
    if let Some(get_service) = api.get_service {
        let name = CString::new("my_service").unwrap();
        let ptr = get_service(name.as_ptr());
        if !ptr.is_null() {
            let svc = &*(ptr as *const MyService);
            svc.do_thing();
        }
    }
}
```

#### C/C++

```c
// Mod A: 注册
struct MyService { int version; void (*do_thing)(void); };
static struct MyService svc = { 1, my_func };
g_api->register_service("my_service", &svc);

// Mod B: 使用
struct MyService* s = (struct MyService*)g_api->get_service("my_service");
if (s) s->do_thing();
```

**消息** — 发布/订阅：

#### Rust

```rust
type TopicCallback = unsafe extern "C" fn(*const c_char, *const c_void, u32);

unsafe extern "C" fn on_score(topic: *const c_char, data: *const c_void, size: u32) {
    let score = *(data as *const i32);
    // 处理分数
}

unsafe fn subscribe_to_topic(api: &ChuModAPI) {
    if let Some(subscribe) = api.subscribe {
        let topic = CString::new("score_update").unwrap();
        subscribe(topic.as_ptr(), on_score as *mut c_void);
    }
}

unsafe fn publish_score(api: &ChuModAPI, score: i32) {
    if let Some(publish) = api.publish {
        let topic = CString::new("score_update").unwrap();
        publish(topic.as_ptr(), &score as *const _ as *const c_void, 4);
    }
}
```

#### C/C++

```c
// 订阅
void on_score(const char* topic, void* data, uint32_t size) {
    int score = *(int*)data;
}
g_api->subscribe("score_update", on_score);

// 发布
int score = 1010000;
g_api->publish("score_update", &score, sizeof(score));
```

## 双模式（Loader + inject -k）

同时兼容 loader 加载和 `inject -k` 注入：

```c
#include "chumod.h"

static int my_init(const ChuModInfo* info, const ChuModAPI* api) {
    // 初始化代码
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

Loader 加载时走 `chumod_init`；`inject -k` 注入时后备线程等 3 秒后调 init，此时 `ChuModInfo` 只有基础字段。

> 独立模式下 `ChuModAPI` 所有函数指针为 NULL，调用前检查。

## 依赖

声明依赖，loader 保证加载顺序：

```c
CHUMOD_API const char* chumod_depends() {
    return "base_mod,utility_mod";
}
```

Loader 会保证那些 mod 在你之前加载。

> 依赖通过文件名或 `chumod_name()` 返回值匹配。

## 构建配置

### Rust（推荐）

Cargo.toml：

```toml
[package]
name = "my-mod"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
windows = { version = "0.52", features = ["Win32_Foundation", "Win32_System_LibraryLoader", "Win32_System_Memory", "Win32_UI_WindowsAndMessaging"] }
```

.cargo/config.toml：

```toml
[build]
target = "i686-pc-windows-msvc"
```

构建命令：

```bash
cargo build --release
```

产物：`target/i686-pc-windows-msvc/release/my_mod.dll`

### C/C++（传统）

CMakeLists.txt 示例：

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

用 `-A Win32` 构建（游戏 32 位）：

```bash
cmake -B build -A Win32
cmake --build build --config Release
```

编译产物复制到 `mods/`。

## 注意事项

- **Rust mod**：必须指定 `crate-type = ["cdylib"]`，目标平台为 `i686-pc-windows-msvc`
- **C/C++ mod**：必须 `-A Win32`，游戏是 32 位
- 不要阻塞 `DllMain`，耗时操作放 `chumod_init` 或新线程
- 用 AOB 扫描而不是硬编码地址，游戏更新会变
- `chumod_shutdown` 里清理所有 hook 和资源
- 双模式下 API 指针可能为 NULL

## 参考

- [API 参考](api-reference_cn.md)
- [chumod.h](../include/chumod.h)
