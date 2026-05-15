# ChuModLoader Mod 开发指南（v3.0.0）

ChuModLoader 是一个 `version.dll` 代理，会在 `chusanApp.exe` 启动时从 `mods/` 加载 Mod DLL。Mod 可以用多种方式编写，并按需接入更丰富的 API。

## 三种 Mod 方式

1. **Rust Mod**：构建 Win32 DLL，导出带 `#[no_mangle]` 的 `extern "C"` 函数。
2. **C/C++ Mod**：包含 `include/chumod.h`，导出 `CHUMOD_API` 函数，并调用 `ChuModAPI` 函数表。
3. **Plain DLL**：只提供 `DllMain`。ChuModLoader 仍会加载 DLL；除非存在匹配导出，否则不会调用 API 回调。

所有新增导出都是可选的。只提供 `chumod_init`、`chumod_shutdown`、`chumod_name` 的旧 Mod 会继续工作。

## Loader 生命周期

对接入 API 的 Mod，生命周期如下：

```text
LoadLibrary(mod.dll)
  -> 读取可选 metadata/dependency 导出
  -> chumod_init(info, api)
  -> 所有成功初始化的 Mod 完成后：chumod_on_ready()
  -> 可选重复帧事件：chumod_on_frame()
  -> 游戏运行中
  -> chumod_shutdown()
  -> FreeLibrary(mod.dll)
```

`chumod_init` 适合本地初始化和 hook 创建。需要等待其他 Mod 服务存在时，使用 `chumod_on_ready`。`chumod_shutdown` 中应禁用 hook 并释放资源。

`chumod_on_frame` 是可选导出。v3.0.0 会在 `chumod_on_ready` 之后，对导出它的 Mod 启动 16ms 间隔的兜底帧循环。

## Quick Start：Rust

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
    pub reload_mod: Option<unsafe extern "C" fn(*const c_char) -> i32>,
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

编译为 32 位 Windows DLL 后放入 `mods/`。

## Quick Start：C++

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

CHUMOD_API void chumod_on_frame() {
    // 由 Loader 兜底帧循环调用
}

CHUMOD_API void chumod_shutdown() {
    if (g_api) g_api->log_info("C++ mod shutdown");
}
```

## TOML 配置

ChuModLoader v2.5 可从以下路径读取单 Mod TOML 文件：

```text
mods/config/<mod_name>.toml
```

示例：

```toml
[config]
enabled = true
profile = "default"

[graphics]
target_fps = 120
scale = 1.25
show_overlay = false
```

使用 TOML API 读取结构化配置：

```c
int fps = api->toml_get_int("graphics", "target_fps", 60);
float scale = api->toml_get_float("graphics", "scale", 1.0f);
int overlay = api->toml_get_bool("graphics", "show_overlay", 0);

char profile[64];
api->toml_get_string("config", "profile", profile, sizeof(profile), "default");
```

旧 INI API 仍使用 `mods/config/<mod_name>.ini` 和 `[config]` section：

```ini
[config]
enabled=true
target_fps=120
```

如果 TOML 文件存在，v2.5 会为 TOML getter 加载它。INI setter 仍写入 INI 文件。

## `manifest.toml` 格式

单 Mod manifest 位于：

```text
mods/manifest/<mod_name>.toml
```

推荐格式：

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

C ABI metadata 导出仍是运行时加载的权威来源。Manifest 更适合工具、启动器和人工打包说明。Mod 可查询自己的 manifest 路径：

```c
const char* path = api->get_manifest_path ? api->get_manifest_path() : NULL;
if (path) api->log_info(path);
```

## 依赖声明

导出 `chumod_depends` 可请求加载顺序。返回依赖名的逗号分隔列表，通常使用显示名或文件 stem。

```c
CHUMOD_API const char* chumod_depends() {
    return "CoreMod,SharedUi";
}
```

Loader 会在调用 `chumod_init` 前排序 Mod。如果依赖无法满足，Loader 会记录问题并以尽力顺序继续。

## 崩溃保护

ChuModLoader 使用 Rust `catch_unwind` 包裹 `chumod_init`、`chumod_on_ready`、`chumod_on_frame` 和 `chumod_shutdown`。v3.0.0 还会安装顶层 SEH 过滤器，把 minidump 和可读 crash log 写到 `mods/crash/`。这**不能**保证任意内存错误安全：native 代码访问冲突仍可能在写出 dump 后终止进程。

建议：

- patch 前验证地址。
- 在 `chumod_shutdown` 中禁用 hook。
- 导出回调保持短小、确定。
- 不要让 C++ 异常跨越 C ABI 边界。

## 分级日志

v2.5 新增普通分级日志：

```c
api->log_info("normal status");
api->log_warn("recoverable problem");
api->log_error("operation failed");
```

需要格式化时可用 `api->log`，但跨语言 Mod 优先使用普通日志 API。在 `chumod_init` 期间，`api->log_path` 指向 `mods/log/` 下的当前 Mod 日志路径。

## Dual Mode

`CHUMOD_DUAL_MODE(init_func)` 让一个 DLL 同时支持 ChuModLoader 和独立注入。

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

独立模式下，兜底 `ChuModAPI` 通常只有 `struct_size`；大多数函数指针为 `NULL`。调用前必须检查。

## 最低 Loader 版本

如果 Mod 需要某个 Loader 版本新增的 API，导出 `chumod_min_loader_version`。

```c
CHUMOD_API const char* chumod_min_loader_version() {
    return "2.5.0";
}
```

面向未知 Loader 版本用户分发二进制时，也要通过 `struct_size` 和空指针检查保护单个字段。

## 热重载

v3.0.0 新增 `api->reload_mod`。它可以按显示名、文件名或文件 stem 热重载一个已加载 Mod。

```c
if (api->reload_mod) {
    api->reload_mod("C++ Example");
}
```

也可以创建 `mods/reload.flag` 触发所有当前已加载 Mod 热重载。Loader 处理完会删除该 flag。
