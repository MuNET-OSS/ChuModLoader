# ChuModLoader API Reference (v3.0.0)

This document describes the C ABI exposed by ChuModLoader v3.0.0. The ABI is intentionally append-only: older mods continue to work, and newer mods should check `ChuModAPI::struct_size` before using fields added after their target version.

For C/C++ projects, include `include/chumod.h`. Rust or other languages should mirror the same `#[repr(C)]` layouts and `extern "C"` function signatures.

## Constants

```c
#define CHUMOD_API __declspec(dllexport)
#define CHUMOD_API_VERSION 3
```

- `CHUMOD_API` marks functions exported by a mod DLL.
- `CHUMOD_API_VERSION` is the loader ABI version reported in `ChuModInfo::api_version`.

## `ChuModInfo`

Runtime information passed to `chumod_init`.

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
    const char* game_version;
} ChuModInfo;
```

| Field | Description |
| --- | --- |
| `api_version` | Loader ABI version. v3.0.0 reports `CHUMOD_API_VERSION` (`3`). |
| `loader_version` | Loader package version string, e.g. `"3.0.0"`. |
| `game_module` | Game executable module name, usually `"chusanApp.exe"`; may be `NULL`. |
| `game_base` | Base address of the game image, or `0` if the module was not found. |
| `game_size` | Size of the game image in bytes. |
| `text_base` | Base address of the game `.text` section. |
| `text_size` | Virtual size of the `.text` section in bytes. |
| `rdata_base` | Base address of the game `.rdata` section. |
| `rdata_size` | Virtual size of the `.rdata` section in bytes. |
| `game_version` | FileVersion/ProductVersion read from the game PE resources; empty string if unavailable. |

## `ChuModAPI`

Function table provided to mods. The loader sets `struct_size = sizeof(ChuModAPI)` for its build.

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
} ChuModAPI;
```

### Compatibility check

```c
#include <stddef.h>

if (api->struct_size >= offsetof(ChuModAPI, log_info) + sizeof(api->log_info) && api->log_info) {
    api->log_info("v2.5 logging is available");
}
```

## v1 API

### Logging

```c
typedef void (*ChuModLogFunc)(const char* fmt, ...);
void log(const char* fmt, ...);
```

Writes a formatted message to `chusan_loader.log` and the console if attached.

Parameters:
- `fmt`: `printf`-style format string.
- `...`: values referenced by `fmt`.

Return value: none.

Example:

```c
api->log("%s loaded at 0x%08X", "MyMod", (unsigned)info->game_base);
```

### AOB scan

```c
typedef uintptr_t (*ChuModAobScanFunc)(
    uintptr_t start,
    uint32_t size,
    const uint8_t* pattern,
    const char* mask
);
uintptr_t aob_scan(uintptr_t start, uint32_t size, const uint8_t* pattern, const char* mask);
```

Scans memory for a byte pattern. In `mask`, `x` means exact match and `?` means wildcard.

Parameters:
- `start`: start address.
- `size`: bytes to scan.
- `pattern`: byte pattern.
- `mask`: mask string with the same length as `pattern`.

Return value: matched address, or `0` if not found or arguments are invalid.

Example:

```c
uint8_t pat[] = { 0x8B, 0x45, 0x00, 0x89 };
uintptr_t addr = api->aob_scan(info->text_base, info->text_size, pat, "xx?x");
```

### Memory access

```c
typedef int (*ChuModMemReadFunc)(uintptr_t addr, void* buf, uint32_t size);
typedef int (*ChuModMemWriteFunc)(uintptr_t addr, const void* buf, uint32_t size);
typedef int (*ChuModMemFillFunc)(uintptr_t addr, uint8_t value, uint32_t size);

int mem_read(uintptr_t addr, void* buf, uint32_t size);
int mem_write(uintptr_t addr, const void* buf, uint32_t size);
int mem_fill(uintptr_t addr, uint8_t value, uint32_t size);
```

Reads, writes, or fills process memory. The loader temporarily adjusts page protection where needed.

Parameters:
- `addr`: target address.
- `buf`: source/destination buffer for read/write.
- `value`: byte used by `mem_fill`.
- `size`: number of bytes.

Return value: `0` on success, non-zero on failure.

Example:

```c
uint8_t nop = 0x90;
api->mem_write(address, &nop, 1);
api->mem_fill(address + 1, 0x90, 5);
```

### Inline hooks

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

Creates and controls inline hooks. `original` receives a trampoline pointer when provided.

Parameters:
- `target`: function address to hook.
- `detour`: replacement function.
- `original`: optional out-parameter for trampoline.

Return value: `0` on success, non-zero on failure.

Example:

```c
typedef int (__stdcall *TargetFn)(int);
static TargetFn real_target = NULL;

int __stdcall my_target(int value) {
    return real_target(value + 1);
}

api->hook_create((void*)target_addr, (void*)my_target, (void**)&real_target);
api->hook_enable((void*)target_addr);
```

### Inter-mod IPC

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

Provides simple service lookup and topic-based messaging between loaded mods.

Return values:
- `register_service`, `publish`, `subscribe`: `0` on success.
- `get_service`: registered pointer, or `NULL` if missing.

Example:

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

### RTTI helper

```c
typedef uintptr_t (*ChuModRttiFindVtableFunc)(const char* rtti_class_name);
uintptr_t rtti_find_vtable(const char* rtti_class_name);
```

Finds an MSVC RTTI vtable by class name inside the game image.

Parameters:
- `rtti_class_name`: decorated or plain class name to search for.

Return value: vtable address, or `0` if not found.

Example:

```c
uintptr_t vt = api->rtti_find_vtable("SomeGameClass");
```

### INI configuration

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

Reads and writes per-mod INI configuration under `mods/config/<mod_name>.ini`, using the `[config]` section.

Return values:
- Getters return the parsed value or the default.
- `config_get_string` writes into `buf` and returns written length.
- Setters return `0` on success.

Example:

```c
int enabled = api->config_get_bool("enabled", 1);
char name[64];
api->config_get_string("profile", name, sizeof(name), "default");
api->config_set_int("launch_count", 42);
```

## v2.5 API

### Leveled logging

```c
typedef void (*ChuModLogPlainFunc)(const char* message);
void log_info(const char* message);
void log_warn(const char* message);
void log_error(const char* message);
```

Writes plain text at INFO/WARN/ERROR levels. These are safer than variadic logging when crossing languages.

Parameters:
- `message`: null-terminated UTF-8/ANSI text.

Return value: none.

Example:

```c
api->log_info("configuration loaded");
api->log_warn("optional pattern not found");
api->log_error("required hook failed");
```

### Per-mod log path

```c
const char* log_path;
```

Path to the current mod log file, usually `mods/log/<mod_name>.log`. It may be `NULL` when unavailable.

Example:

```c
if (api->log_path) {
    api->log_info(api->log_path);
}
```

### TOML configuration

```c
int toml_section_exists(const char* section);
int toml_get_bool(const char* section, const char* key, int default_val);
int toml_get_int(const char* section, const char* key, int default_val);
float toml_get_float(const char* section, const char* key, float default_val);
int toml_get_string(const char* section, const char* key, char* buf, uint32_t buf_size, const char* default_val);
```

Reads per-mod TOML configuration from `mods/config/<mod_name>.toml`. If the TOML file does not exist, the loader falls back to the INI path for legacy config APIs.

Parameters:
- `section`: TOML table name, for example `"config"`, `"graphics"`, or `"features"`.
- `key`: key inside the table.
- `default_val`: value returned when the section/key is missing or invalid.
- `buf` / `buf_size`: destination buffer for strings.

Return values:
- `toml_section_exists`: `1` if the table exists, otherwise `0`.
- Other getters return parsed value or the default.
- `toml_get_string` returns written length.

Example:

```c
if (api->toml_section_exists("graphics")) {
    int fps = api->toml_get_int("graphics", "target_fps", 60);
    float scale = api->toml_get_float("graphics", "scale", 1.0f);
    char preset[32];
    api->toml_get_string("graphics", "preset", preset, sizeof(preset), "default");
}
```

### Manifest path

```c
typedef const char* (*ChuModGetManifestPathFunc)(void);
const char* get_manifest_path(void);
```

Returns the current mod manifest path (`mods/manifest/<mod_name>.toml`) or `NULL` if no manifest exists.

Example:

```c
const char* manifest = api->get_manifest_path ? api->get_manifest_path() : NULL;
if (manifest) {
    api->log_info(manifest);
}
```

## v3 API

### Hot reload

```c
typedef int (*ChuModReloadModFunc)(const char* mod_name);
int reload_mod(const char* mod_name);
```

Reloads a currently loaded mod by display name, file name, or file stem. The loader calls the target mod's `chumod_shutdown`, unloads the DLL with `FreeLibrary`, loads it again, then calls `chumod_init` and `chumod_on_ready` if present. The whole flow is panic-guarded and logged.

Parameters:
- `mod_name`: display name, DLL file name (`example.dll`), or file stem (`example`).

Return value: `0` on success, non-zero on failure.

Example:

```c
if (api->reload_mod) {
    int ret = api->reload_mod("Example Mod");
    if (ret != 0) api->log_error("reload failed");
}
```

External trigger: create `mods/reload.flag` to reload all currently loaded mods. The loader monitor thread deletes the flag after processing.

## Mod exports

All exports are optional except that mods needing the API should export `chumod_init`. Plain DLLs with only `DllMain` are still loaded.

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

| Export | Loader behavior |
| --- | --- |
| `chumod_init` | Called after the DLL is loaded and dependencies are ready. Return `0` to keep the mod loaded; non-zero skips/unloads it. |
| `chumod_shutdown` | Called during loader unload before `FreeLibrary`. |
| `chumod_name` | Returns display name used in logs and dependency resolution. |
| `chumod_depends` | Returns comma-separated dependency names. |
| `chumod_version` | Returns mod version metadata. |
| `chumod_author` | Returns mod author metadata. |
| `chumod_min_loader_version` | Returns minimum required loader version, e.g. `"2.1.0"`. |
| `chumod_on_ready` | Called after all successful `chumod_init` calls finish. |
| `chumod_on_frame` | Called by the loader fallback frame loop after `chumod_on_ready`; current implementation uses a 16 ms interval thread. |

Example:

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
    g_api->log_info("all mods are ready");
}

CHUMOD_API void chumod_shutdown(void) {
    if (g_api) g_api->log_info("shutdown");
}
```

## `CHUMOD_DUAL_MODE`

`CHUMOD_DUAL_MODE(init_func)` helps a mod support both ChuModLoader and standalone injection. Loader mode calls `chumod_init`; standalone mode starts a delayed fallback thread from `DllMain` and calls the same initializer with best-effort game information.

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

In standalone fallback, many API function pointers are `NULL`; always check before use.
