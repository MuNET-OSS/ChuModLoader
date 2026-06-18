# ChuModLoader API 参考（v4.0.0）

本文档描述 ChuModLoader v4.0.0 向 Mod 提供的 C ABI。该 ABI 采用「只追加字段」的兼容策略：旧 Mod 可以继续工作，新 Mod 在使用较新字段前应检查 `ChuModAPI::struct_size`。

C/C++ 项目请包含 `include/chumod.h`。Rust Mod 需要镜像相同的 `#[repr(C)]` 内存布局和 `extern "C"` 函数签名。

## 目录

- [常量](#常量)
- [`ChuModInfo`](#chumodinfo)
- [`ChuModAPI`](#chumodapi)
  - [兼容性检查](#兼容性检查)
- [v1 API](#v1-api)
- [v2 API](#v2-api)
- [v2.5 API](#v25-api)
- [v3 API](#v3-api)
- [v4 API](#v4-api)
- [Mod 导出函数](#mod-导出函数)
- [`CHUMOD_DUAL_MODE`](#chumod_dual_mode)

## 常量

```c
#define CHUMOD_API __declspec(dllexport)
#define CHUMOD_API_VERSION 4
```

- `CHUMOD_API`：标记 Mod DLL 导出的函数。
- `CHUMOD_API_VERSION`：Loader 的 ABI 版本，会通过 `ChuModInfo::api_version` 报告。

## `ChuModInfo`

`chumod_init` 接收的运行时信息。

```c
typedef struct {
    uint32_t api_version;
    const char* loader_version;
    const char* game_module;
    uintptr_t game_base;
    uint32_t game_size;
    uintptr_t text_base;
    uint32_t text_size;
    uintptr_t rdata_base;
    uint32_t rdata_size;
} ChuModInfo;
```

| 字段 | 说明 |
| --- | --- |
| `api_version` | Loader 的 ABI 版本。v4.0.0 为 `CHUMOD_API_VERSION`（`4`）。 |
| `loader_version` | Loader 包版本字符串，例如 `"1.0.0"`。 |
| `game_module` | 游戏主程序模块名，通常为 `"chusanApp.exe"`；可能为 `NULL`。 |
| `game_base` | 游戏镜像基址，找不到模块时为 `0`。 |
| `game_size` | 游戏镜像大小（字节）。 |
| `text_base` | 游戏 `.text` 节基址。 |
| `text_size` | `.text` 节虚拟大小（字节）。 |
| `rdata_base` | 游戏 `.rdata` 节基址。 |
| `rdata_size` | `.rdata` 节虚拟大小（字节）。 |

## `ChuModAPI`

Loader 提供给 Mod 的函数表。Loader 会按自身构建设置 `struct_size = sizeof(ChuModAPI)`。

```c
typedef struct {
    uint32_t struct_size;

    /* v1 */
    ChuModLogFunc log;
    ChuModAobScanFunc aob_scan;
    ChuModMemReadFunc mem_read;
    ChuModMemWriteFunc mem_write;
    ChuModMemFillFunc mem_fill;
    ChuModHookCreateFunc hook_create;
    ChuModHookEnableFunc hook_enable;
    ChuModHookDisableFunc hook_disable;
    ChuModHookRemoveFunc hook_remove;
    ChuModRegisterServiceFunc register_service;
    ChuModGetServiceFunc get_service;
    ChuModPublishFunc publish;
    ChuModSubscribeFunc subscribe;

    /* v2 */
    ChuModRttiFindVtableFunc rtti_find_vtable;
    ChuModConfigGetIntFunc config_get_int;
    ChuModConfigGetFloatFunc config_get_float;
    ChuModConfigGetBoolFunc config_get_bool;
    ChuModConfigGetStringFunc config_get_string;
    ChuModConfigSetIntFunc config_set_int;
    ChuModConfigSetFloatFunc config_set_float;
    ChuModConfigSetBoolFunc config_set_bool;
    ChuModConfigSetStringFunc config_set_string;

    /* v2.5 */
    ChuModLogPlainFunc log_info;
    ChuModLogPlainFunc log_warn;
    ChuModLogPlainFunc log_error;
    const char* log_path;
    ChuModTomlSectionExistsFunc toml_section_exists;
    ChuModTomlGetBoolFunc toml_get_bool;
    ChuModTomlGetIntFunc toml_get_int;
    ChuModTomlGetFloatFunc toml_get_float;
    ChuModTomlGetStringFunc toml_get_string;
    ChuModGetManifestPathFunc get_manifest_path;

    /* v3 */
    ChuModReloadModFunc reload_mod;

    /* v4 */
    ChuModRegisterPresentCallbackFunc register_present_callback;
    ChuModRegisterResetCallbackFunc register_reset_callback;
    ChuModSetFrameLockFunc set_frame_lock;
    ChuModGetD3D9DeviceFunc get_d3d9_device;
    ChuModGetGameHwndFunc get_game_hwnd;
} ChuModAPI;
```

> 函数表中的字段按引入版本分组，且只在末尾追加。这样旧 Mod 看到的前半部分布局始终不变，新增字段不会破坏二进制兼容性。

### 兼容性检查

使用某个版本新增的字段前，先用 `struct_size` 判断 Loader 是否真的提供了它，再检查指针非空：

```c
#include <stddef.h>

if (api->struct_size >= offsetof(ChuModAPI, log_info) + sizeof(api->log_info) && api->log_info) {
    api->log_info("v2.5 日志可用");
}
```

## v1 API

### 日志

```c
typedef void (*ChuModLogFunc)(const char* fmt, ...);
void log(const char* fmt, ...);
```

写入格式化日志到 `chumod_loader.log`，控制台可用时也输出到控制台。

参数：
- `fmt`：`printf` 风格格式串。
- `...`：格式串引用的值。

返回值：无。

示例：

```c
api->log("%s 已加载，基址 0x%08X", "MyMod", (unsigned)info->game_base);
```

### AOB 扫描

```c
typedef uintptr_t (*ChuModAobScanFunc)(
    uintptr_t start,
    uint32_t size,
    const uint8_t* pattern,
    const char* mask
);
uintptr_t aob_scan(uintptr_t start, uint32_t size, const uint8_t* pattern, const char* mask);
```

按字节特征扫描内存。`mask` 中 `x` 表示精确匹配，`?` 表示通配。

参数：
- `start`：起始地址。
- `size`：扫描字节数。
- `pattern`：字节模式。
- `mask`：与 `pattern` 等长的掩码字符串。

返回值：匹配地址；找不到或参数无效时返回 `0`。

示例：

```c
uint8_t pat[] = { 0x8B, 0x45, 0x00, 0x89 };
uintptr_t addr = api->aob_scan(info->text_base, info->text_size, pat, "xx?x");
```

### 内存访问

```c
typedef int (*ChuModMemReadFunc)(uintptr_t addr, void* buf, uint32_t size);
typedef int (*ChuModMemWriteFunc)(uintptr_t addr, const void* buf, uint32_t size);
typedef int (*ChuModMemFillFunc)(uintptr_t addr, uint8_t value, uint32_t size);

int mem_read(uintptr_t addr, void* buf, uint32_t size);
int mem_write(uintptr_t addr, const void* buf, uint32_t size);
int mem_fill(uintptr_t addr, uint8_t value, uint32_t size);
```

读取、写入或填充进程内存。Loader 会按需临时调整页面保护属性。

参数：
- `addr`：目标地址。
- `buf`：读写缓冲区。
- `value`：`mem_fill` 使用的填充值。
- `size`：字节数。

返回值：成功为 `0`，失败为非 `0`。

示例：

```c
uint8_t nop = 0x90;
api->mem_write(address, &nop, 1);
api->mem_fill(address + 1, 0x90, 5);
```

### Inline Hook

```c
typedef int (*ChuModHookCreateFunc)(void* target, void* detour, void** original);
typedef int (*ChuModHookEnableFunc)(void* target);
typedef int (*ChuModHookDisableFunc)(void* target);
typedef int (*ChuModHookRemoveFunc)(void* target);

int hook_create(void* target, void* detour, void** original);
int hook_enable(void* target);
int hook_disable(void* target);
int hook_remove(void* target);
```

创建并控制 inline hook。传入 `original` 时可获得 trampoline 指针，用于调用原始函数。

参数：
- `target`：要 hook 的函数地址。
- `detour`：替换函数。
- `original`：可选输出参数，用于接收 trampoline。

返回值：成功为 `0`，失败为非 `0`。

示例：

```c
typedef int (__stdcall *TargetFn)(int);
static TargetFn real_target = NULL;

int __stdcall my_target(int value) {
    return real_target(value + 1);
}

api->hook_create((void*)target_addr, (void*)my_target, (void**)&real_target);
api->hook_enable((void*)target_addr);
```

### Mod 间通信

```c
typedef int (*ChuModRegisterServiceFunc)(const char* name, void* service_ptr);
typedef void* (*ChuModGetServiceFunc)(const char* name);
typedef void (*ChuModMessageCallback)(const char* topic, void* data, uint32_t size);
typedef int (*ChuModPublishFunc)(const char* topic, void* data, uint32_t size);
typedef int (*ChuModSubscribeFunc)(const char* topic, ChuModMessageCallback callback);

int register_service(const char* name, void* service_ptr);
void* get_service(const char* name);
int publish(const char* topic, void* data, uint32_t size);
int subscribe(const char* topic, ChuModMessageCallback callback);
```

提供简单的命名服务查找，以及基于 topic 的消息发布 / 订阅，用于 Mod 之间通信。

返回值：
- `register_service`、`publish`、`subscribe`：成功为 `0`。
- `get_service`：注册过的指针，不存在时为 `NULL`。

示例：

```c
static void on_msg(const char* topic, void* data, uint32_t size) {
    (void)topic;
    (void)data;
    (void)size;
}

api->register_service("example.counter", &counter_service);
api->subscribe("example.ready", on_msg);
api->publish("example.ready", NULL, 0);
```

## v2 API

### RTTI 辅助

```c
typedef uintptr_t (*ChuModRttiFindVtableFunc)(const char* rtti_class_name);
uintptr_t rtti_find_vtable(const char* rtti_class_name);
```

在游戏镜像中按 MSVC RTTI 类名查找 vtable。

参数：
- `rtti_class_name`：要查找的装饰名或普通类名。

返回值：vtable 地址；找不到时为 `0`。

示例：

```c
uintptr_t vt = api->rtti_find_vtable("SomeGameClass");
```

### INI 配置

```c
int config_get_int(const char* key, int default_val);
float config_get_float(const char* key, float default_val);
int config_get_bool(const char* key, int default_val);
int config_get_string(const char* key, char* buf, uint32_t buf_size, const char* default_val);
int config_set_int(const char* key, int value);
int config_set_float(const char* key, float value);
int config_set_bool(const char* key, int value);
int config_set_string(const char* key, const char* value);
```

读写 `mods/config/<mod_name>.ini` 下的单 Mod INI 配置，使用 `[config]` section。

返回值：
- getter 返回解析值或默认值。
- `config_get_string` 写入 `buf` 并返回写入长度。
- setter 成功返回 `0`。

示例：

```c
int enabled = api->config_get_bool("enabled", 1);
char name[64];
api->config_get_string("profile", name, sizeof(name), "default");
api->config_set_int("launch_count", 42);
```

## v2.5 API

### 分级日志

```c
typedef void (*ChuModLogPlainFunc)(const char* message);
void log_info(const char* message);
void log_warn(const char* message);
void log_error(const char* message);
```

按 INFO / WARN / ERROR 级别写入纯文本日志。跨语言调用时比 variadic 日志更安全。

参数：
- `message`：以 `\0` 结尾的 UTF-8 / ANSI 文本。

返回值：无。

示例：

```c
api->log_info("配置已加载");
api->log_warn("可选特征未找到");
api->log_error("必需的 hook 失败");
```

### 单 Mod 日志路径

```c
const char* log_path;
```

当前 Mod 的日志文件路径，通常为 `mods/log/<mod_name>.log`。不可用时可能为 `NULL`。

示例：

```c
if (api->log_path) {
    api->log_info(api->log_path);
}
```

### TOML 配置

```c
int toml_section_exists(const char* section);
int toml_get_bool(const char* section, const char* key, int default_val);
int toml_get_int(const char* section, const char* key, int default_val);
float toml_get_float(const char* section, const char* key, float default_val);
int toml_get_string(const char* section, const char* key, char* buf, uint32_t buf_size, const char* default_val);
```

从 `mods/config/<mod_name>.toml` 读取单 Mod TOML 配置。若 TOML 文件不存在，旧的配置 API 仍会回退到 INI 路径。

参数：
- `section`：TOML 表名，例如 `"config"`、`"graphics"`、`"features"`。
- `key`：表内键名。
- `default_val`：section / key 缺失或类型无效时返回的默认值。
- `buf` / `buf_size`：字符串输出缓冲区。

返回值：
- `toml_section_exists`：表存在返回 `1`，否则 `0`。
- 其他 getter 返回解析值或默认值。
- `toml_get_string` 返回写入长度。

示例：

```c
if (api->toml_section_exists("graphics")) {
    int fps = api->toml_get_int("graphics", "target_fps", 60);
    float scale = api->toml_get_float("graphics", "scale", 1.0f);
    char preset[32];
    api->toml_get_string("graphics", "preset", preset, sizeof(preset), "default");
}
```

### Manifest 路径

```c
typedef const char* (*ChuModGetManifestPathFunc)(void);
const char* get_manifest_path(void);
```

返回当前 Mod 的 manifest 路径（`mods/manifest/<mod_name>.toml`），没有 manifest 时返回 `NULL`。

示例：

```c
const char* manifest = api->get_manifest_path ? api->get_manifest_path() : NULL;
if (manifest) {
    api->log_info(manifest);
}
```

## v3 API

### 热重载

```c
typedef int (*ChuModReloadModFunc)(const char* mod_name);
int reload_mod(const char* mod_name);
```

按显示名、DLL 文件名或文件 stem 热重载一个当前已加载的 Mod。Loader 会调用目标 Mod 的 `chumod_shutdown`，用 `FreeLibrary` 卸载 DLL，再重新加载，然后调用 `chumod_init` 和可选的 `chumod_on_ready`。整个流程会被 panic guard 包裹并记录日志。

参数：
- `mod_name`：显示名、DLL 文件名（`example.dll`）或文件 stem（`example`）。

返回值：成功为 `0`，失败为非 `0`。

示例：

```c
if (api->reload_mod) {
    int ret = api->reload_mod("Example Mod");
    if (ret != 0) api->log_error("热重载失败");
}
```

外部触发：创建 `mods/reload.flag` 会热重载所有当前已加载的 Mod。Loader 的监视线程处理完后会删除该 flag。

## v4 API

### d3d9 服务

Loader 代理游戏的 Direct3D 9 设备，向 Mod 暴露每帧回调、帧率锁定，以及设备 / 窗口句柄。这些能力在旧版本中是独立的 `D3D9ProxyAPI`；从 v4 起合并进 `ChuModAPI`。

```c
typedef void (*ChuModPresentCallback)(void* device);
typedef void (*ChuModResetCallback)(void* device, uint32_t phase);

int  register_present_callback(ChuModPresentCallback callback);
int  register_reset_callback(ChuModResetCallback callback);
int  set_frame_lock(uint32_t fps);
void* get_d3d9_device(void);
uintptr_t get_game_hwnd(void);
```

- `register_present_callback`：每帧在游戏 Present 前调用，`device` 是原生 `IDirect3DDevice9*`；传 `NULL` 清除。适合做 FPS 叠加层 / ImGui 渲染。
- `register_reset_callback`：在设备 Reset 前后调用。`phase = 0` 为 Reset 前（应释放 `D3DPOOL_DEFAULT` 资源）；`phase = 1` 为 Reset 后（应重建资源）；传 `NULL` 清除。
- `set_frame_lock`：将渲染帧率锁定为 `fps`；`0` 表示解锁。
- `get_d3d9_device`：返回当前 `IDirect3DDevice9*`，设备尚未就绪时返回 `NULL`。
- `get_game_hwnd`：返回游戏窗口 `HWND`（以 `uintptr_t` 表示），不可用时返回 `0`。

返回值：回调注册 / 设置类函数成功返回 `0`，失败返回非零。

示例：

```c
static void on_present(void* device) {
    /* 用 IDirect3DDevice9* device 绘制叠加层 */
}

if (api->register_present_callback) {
    api->register_present_callback(on_present);
    api->set_frame_lock(60);
}
```

## Mod 导出函数

所有导出都是可选的；需要 API 的 Mod 通常会导出 `chumod_init`。只有 `DllMain` 的 Plain DLL 也会被加载。

```c
CHUMOD_API int chumod_init(const ChuModInfo* info, const ChuModAPI* api);
CHUMOD_API void chumod_shutdown(void);
CHUMOD_API const char* chumod_name(void);
CHUMOD_API const char* chumod_depends(void);
CHUMOD_API const char* chumod_version(void);
CHUMOD_API const char* chumod_author(void);
CHUMOD_API const char* chumod_min_loader_version(void);
CHUMOD_API void chumod_on_ready(void);
CHUMOD_API void chumod_on_frame(void);
```

| 导出 | Loader 行为 |
| --- | --- |
| `chumod_init` | DLL 加载且依赖就绪后调用。返回 `0` 表示继续保留；非 `0` 会被跳过并卸载。 |
| `chumod_shutdown` | Loader 卸载期间、`FreeLibrary` 前调用。 |
| `chumod_name` | 返回用于日志和依赖解析的显示名。 |
| `chumod_depends` | 返回逗号分隔的依赖名。 |
| `chumod_version` | 返回 Mod 版本元数据。 |
| `chumod_author` | 返回 Mod 作者元数据。 |
| `chumod_min_loader_version` | 返回最低 Loader 版本要求，例如 `"2.1.0"`。 |
| `chumod_on_ready` | 所有成功的 `chumod_init` 完成后调用。 |
| `chumod_on_frame` | `chumod_on_ready` 之后由 Loader 兜底帧循环调用；当前实现使用 16ms 间隔线程。 |

示例：

```c
static const ChuModAPI* g_api = NULL;

CHUMOD_API const char* chumod_name(void) { return "Example Mod"; }
CHUMOD_API const char* chumod_version(void) { return "1.0.0"; }
CHUMOD_API const char* chumod_author(void) { return "Example Team"; }
CHUMOD_API const char* chumod_min_loader_version(void) { return "2.5.0"; }
CHUMOD_API const char* chumod_depends(void) { return "CoreMod"; }

CHUMOD_API int chumod_init(const ChuModInfo* info, const ChuModAPI* api) {
    (void)info;
    g_api = api;
    g_api->log_info("init");
    return 0;
}

CHUMOD_API void chumod_on_ready(void) {
    g_api->log_info("所有 Mod 就绪");
}

CHUMOD_API void chumod_shutdown(void) {
    if (g_api) g_api->log_info("shutdown");
}
```

## `CHUMOD_DUAL_MODE`

`CHUMOD_DUAL_MODE(init_func)` 让同一个 Mod 同时支持 ChuModLoader 加载和独立注入。Loader 模式会调用 `chumod_init`；独立注入模式会从 `DllMain` 启动一个延迟兜底线程，并用尽力获取的游戏信息调用同一个初始化函数。

```c
static int my_init(const ChuModInfo* info, const ChuModAPI* api) {
    (void)info;
    if (api && api->log_info) api->log_info("dual mode init");
    return 0;
}

CHUMOD_DUAL_MODE(my_init);

BOOL APIENTRY DllMain(HMODULE module, DWORD reason, LPVOID reserved) {
    (void)module;
    (void)reserved;
    if (reason == DLL_PROCESS_ATTACH) {
        CHUMOD_DUAL_MODE_START();
    }
    return TRUE;
}
```

独立兜底模式下，大多数 API 函数指针是 `NULL`；调用前必须检查。
