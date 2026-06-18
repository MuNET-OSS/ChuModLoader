#pragma once

/**
 * @file chumod.h
 * @brief ChuModLoader C/C++ Mod API / ChuModLoader C/C++ 模组接口。
 *
 * All newly added exports are optional. Older mods that only export
 * chumod_init/chumod_shutdown/chumod_name continue to work.
 * 所有新增导出均为可选；旧 mod 不导出新函数也能继续工作。
 */

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <Windows.h>

/** Export marker for mod functions / Mod 导出函数标记。 */
#define CHUMOD_API __declspec(dllexport)

/** Current ABI version / 当前 ABI 版本。 */
#define CHUMOD_API_VERSION 4

/**
 * @brief Runtime information passed to chumod_init.
 * @brief 传给 chumod_init 的运行时信息。
 */
typedef struct {
    /** API version supported by loader / Loader 支持的 API 版本。 */
    uint32_t api_version;
    /** Loader version string, e.g. "2.5.0" / Loader 版本字符串。 */
    const char* loader_version;
    /** Game executable module name or NULL / 游戏主程序模块名，可能为 NULL。 */
    const char* game_module;
    /** Game image base address / 游戏镜像基址。 */
    uintptr_t game_base;
    /** Game image size / 游戏镜像大小。 */
    uint32_t game_size;
    /** .text section base address / .text 节基址。 */
    uintptr_t text_base;
    /** .text section virtual size / .text 节虚拟大小。 */
    uint32_t text_size;
    /** .rdata section base address / .rdata 节基址。 */
    uintptr_t rdata_base;
    /** .rdata section virtual size / .rdata 节虚拟大小。 */
    uint32_t rdata_size;
} ChuModInfo;

/** @brief Write formatted text to loader log. / 写入格式化文本到 loader 日志。 */
typedef void (*ChuModLogFunc)(const char* fmt, ...);
/** @brief Write plain INFO/WARN/ERROR text to loader log. / 写入普通分级日志文本。 */
typedef void (*ChuModLogPlainFunc)(const char* message);

/**
 * @brief Scan memory by pattern and mask. Mask uses 'x' for exact byte and '?' for wildcard. Returns address or 0.
 * @brief 按 pattern/mask 扫描内存。mask 中 'x' 表示精确匹配，'?' 表示通配；返回地址或 0。
 */
typedef uintptr_t (*ChuModAobScanFunc)(uintptr_t start, uint32_t size,
                                        const uint8_t* pattern, const char* mask);

/** @brief Read process memory; adjusts page protection automatically. Returns 0 on success. / 读内存，自动处理页保护，0 表示成功。 */
typedef int (*ChuModMemReadFunc)(uintptr_t addr, void* buf, uint32_t size);
/** @brief Write process memory; adjusts page protection automatically. Returns 0 on success. / 写内存，自动处理页保护，0 表示成功。 */
typedef int (*ChuModMemWriteFunc)(uintptr_t addr, const void* buf, uint32_t size);
/** @brief Fill process memory; adjusts page protection automatically. Returns 0 on success. / 填充内存，自动处理页保护，0 表示成功。 */
typedef int (*ChuModMemFillFunc)(uintptr_t addr, uint8_t value, uint32_t size);

/** @brief Create a hook and optionally receive trampoline in original. Returns 0 on success. / 创建 hook，可通过 original 取 trampoline，0 表示成功。 */
typedef int (*ChuModHookCreateFunc)(void* target, void* detour, void** original);
/** @brief Enable hook for target. Returns 0 on success. / 启用 target 对应 hook，0 表示成功。 */
typedef int (*ChuModHookEnableFunc)(void* target);
/** @brief Disable hook for target. Returns 0 on success. / 禁用 target 对应 hook，0 表示成功。 */
typedef int (*ChuModHookDisableFunc)(void* target);
/** @brief Remove hook for target. Returns 0 on success. / 移除 target 对应 hook，0 表示成功。 */
typedef int (*ChuModHookRemoveFunc)(void* target);

/** @brief Register a named service pointer. / 注册命名服务指针。 */
typedef int (*ChuModRegisterServiceFunc)(const char* name, void* service_ptr);
/** @brief Get a named service pointer, or NULL if missing. / 获取命名服务指针，不存在返回 NULL。 */
typedef void* (*ChuModGetServiceFunc)(const char* name);
/** @brief Message callback used by subscribe. / 订阅消息回调。 */
typedef void (*ChuModMessageCallback)(const char* topic, void* data, uint32_t size);
/** @brief Publish a message to topic. / 发布消息到 topic。 */
typedef int (*ChuModPublishFunc)(const char* topic, void* data, uint32_t size);
/** @brief Subscribe a message callback to topic. / 订阅 topic 消息。 */
typedef int (*ChuModSubscribeFunc)(const char* topic, ChuModMessageCallback callback);

/** @brief Find vtable by MSVC RTTI class name. Returns address or 0. / 按 MSVC RTTI 类名查找 vtable，返回地址或 0。 */
typedef uintptr_t (*ChuModRttiFindVtableFunc)(const char* rtti_class_name);

/** @brief Read integer from [config], returns default_val if missing. / 从 [config] 读取整数，缺失返回默认值。 */
typedef int   (*ChuModConfigGetIntFunc)(const char* key, int default_val);
/** @brief Read float from [config], returns default_val if missing. / 从 [config] 读取浮点数，缺失返回默认值。 */
typedef float (*ChuModConfigGetFloatFunc)(const char* key, float default_val);
/** @brief Read bool from [config], returns 0 or 1. / 从 [config] 读取布尔值，返回 0 或 1。 */
typedef int   (*ChuModConfigGetBoolFunc)(const char* key, int default_val);
/** @brief Read string into buf, returns written length. / 读取字符串到 buf，返回实际长度。 */
typedef int   (*ChuModConfigGetStringFunc)(const char* key, char* buf, uint32_t buf_size, const char* default_val);
/** @brief Write integer to [config]. Returns 0 on success. / 写入整数到 [config]，0 表示成功。 */
typedef int   (*ChuModConfigSetIntFunc)(const char* key, int value);
/** @brief Write float to [config]. Returns 0 on success. / 写入浮点数到 [config]，0 表示成功。 */
typedef int   (*ChuModConfigSetFloatFunc)(const char* key, float value);
/** @brief Write bool to [config]. Returns 0 on success. / 写入布尔值到 [config]，0 表示成功。 */
typedef int   (*ChuModConfigSetBoolFunc)(const char* key, int value);
/** @brief Write string to [config]. Returns 0 on success. / 写入字符串到 [config]，0 表示成功。 */
typedef int   (*ChuModConfigSetStringFunc)(const char* key, const char* value);

/** @brief Check whether a TOML section exists. / 检查 TOML section 是否存在。 */
typedef int   (*ChuModTomlSectionExistsFunc)(const char* section);
/** @brief Read bool from TOML section/key. / 从 TOML section/key 读取布尔值。 */
typedef int   (*ChuModTomlGetBoolFunc)(const char* section, const char* key, int default_val);
/** @brief Read integer from TOML section/key. / 从 TOML section/key 读取整数。 */
typedef int   (*ChuModTomlGetIntFunc)(const char* section, const char* key, int default_val);
/** @brief Read float from TOML section/key. / 从 TOML section/key 读取浮点数。 */
typedef float (*ChuModTomlGetFloatFunc)(const char* section, const char* key, float default_val);
/** @brief Read string from TOML section/key into buf. / 从 TOML section/key 读取字符串到 buf。 */
typedef int   (*ChuModTomlGetStringFunc)(const char* section, const char* key, char* buf, uint32_t buf_size, const char* default_val);

/** @brief Return current mod manifest path, or NULL. / 返回当前 mod 的 manifest 路径，不存在则 NULL。 */
typedef const char* (*ChuModGetManifestPathFunc)(void);

/** @brief Reload a loaded mod by display name, file name, or file stem. Returns 0 on success. / 按显示名、文件名或文件 stem 热重载已加载 mod，0 表示成功。 */
typedef int (*ChuModReloadModFunc)(const char* mod_name);

/** @brief Per-frame Present callback; param is native IDirect3DDevice9*. / 每帧 Present 回调，参数为原生 IDirect3DDevice9*。 */
typedef void (*ChuModPresentCallback)(void* device);
/** @brief Device Reset event; phase 0 = before reset (release D3DPOOL_DEFAULT), 1 = after reset (recreate). / 设备 Reset 事件，phase 0 = Reset 前（释放 D3DPOOL_DEFAULT），1 = Reset 后（重建）。 */
typedef void (*ChuModResetCallback)(void* device, uint32_t phase);
/** @brief Register Present callback, NULL to clear. Returns 0 on success. / 注册 Present 回调，传 NULL 清除，0 表示成功。 */
typedef int (*ChuModRegisterPresentCallbackFunc)(ChuModPresentCallback callback);
/** @brief Register Reset callback, NULL to clear. Returns 0 on success. / 注册 Reset 回调，传 NULL 清除，0 表示成功。 */
typedef int (*ChuModRegisterResetCallbackFunc)(ChuModResetCallback callback);
/** @brief Lock render frame rate to fps (0 = unlock). Returns 0 on success. / 锁定渲染帧率为 fps（0 表示解锁），0 表示成功。 */
typedef int (*ChuModSetFrameLockFunc)(uint32_t fps);
/** @brief Get current IDirect3DDevice9*, or NULL if unavailable. / 获取当前 IDirect3DDevice9*，不可用返回 NULL。 */
typedef void* (*ChuModGetD3D9DeviceFunc)(void);
/** @brief Get game window HWND as uintptr_t, or 0 if unavailable. / 获取游戏窗口 HWND（uintptr_t），不可用返回 0。 */
typedef uintptr_t (*ChuModGetGameHwndFunc)(void);

/**
 * @brief API function table provided by loader.
 * @brief Loader 提供给 mod 的 API 函数表。
 *
 * Mods should check struct_size before using fields added after their target API version.
 * Mod 使用较新字段前应检查 struct_size，保证向后兼容。
 */
typedef struct {
    /** Size of this structure / 此结构体大小。 */
    uint32_t struct_size;

    /** Logging API / 日志 API。 */
    ChuModLogFunc log;

    /** Memory APIs / 内存 API。 */
    ChuModAobScanFunc aob_scan;
    ChuModMemReadFunc mem_read;
    ChuModMemWriteFunc mem_write;
    ChuModMemFillFunc mem_fill;

    /** Hook APIs / Hook API。 */
    ChuModHookCreateFunc hook_create;
    ChuModHookEnableFunc hook_enable;
    ChuModHookDisableFunc hook_disable;
    ChuModHookRemoveFunc hook_remove;

    /** IPC APIs / Mod 间通信 API。 */
    ChuModRegisterServiceFunc register_service;
    ChuModGetServiceFunc get_service;
    ChuModPublishFunc publish;
    ChuModSubscribeFunc subscribe;

    /** v2: RTTI helper / v2: RTTI 辅助。 */
    ChuModRttiFindVtableFunc rtti_find_vtable;

    /** v2: per-mod config APIs / v2: 单 Mod 配置 API。 */
    ChuModConfigGetIntFunc config_get_int;
    ChuModConfigGetFloatFunc config_get_float;
    ChuModConfigGetBoolFunc config_get_bool;
    ChuModConfigGetStringFunc config_get_string;
    ChuModConfigSetIntFunc config_set_int;
    ChuModConfigSetFloatFunc config_set_float;
    ChuModConfigSetBoolFunc config_set_bool;
    ChuModConfigSetStringFunc config_set_string;

    /** v2.5: plain leveled logging APIs / v2.5: 普通分级日志 API。 */
    ChuModLogPlainFunc log_info;
    ChuModLogPlainFunc log_warn;
    ChuModLogPlainFunc log_error;

    /** v2.5: per-mod log file path / v2.5: 单 Mod 日志文件路径。 */
    const char* log_path;

    /** v2.5: TOML config APIs / v2.5: TOML 配置 API。 */
    ChuModTomlSectionExistsFunc toml_section_exists;
    ChuModTomlGetBoolFunc toml_get_bool;
    ChuModTomlGetIntFunc toml_get_int;
    ChuModTomlGetFloatFunc toml_get_float;
    ChuModTomlGetStringFunc toml_get_string;

    /** v2.5: manifest path API / v2.5: manifest 路径 API。 */
    ChuModGetManifestPathFunc get_manifest_path;

    /** v3: hot reload API / v3: 热重载 API。 */
    ChuModReloadModFunc reload_mod;

    /** v4: d3d9 service APIs / v4: d3d9 服务 API。 */
    ChuModRegisterPresentCallbackFunc register_present_callback;
    ChuModRegisterResetCallbackFunc register_reset_callback;
    ChuModSetFrameLockFunc set_frame_lock;
    ChuModGetD3D9DeviceFunc get_d3d9_device;
    ChuModGetGameHwndFunc get_game_hwnd;
} ChuModAPI;

/**
 * @brief Initialization function. Loader calls it after dependencies are ready; return 0 on success.
 * @brief 初始化函数。Loader 在依赖就绪后调用；返回 0 表示成功。
 */
typedef int (*ChuModInitFunc)(const ChuModInfo* info, const ChuModAPI* api);
/** @brief Optional ready event called after all chumod_init calls finish. / 所有 chumod_init 完成后调用的可选就绪事件。 */
typedef void (*ChuModReadyFunc)(void);
/** @brief Optional frame event, called from loader fallback frame loop. / 可选帧事件，由 Loader 兜底帧循环调用。 */
typedef void (*ChuModFrameFunc)(void);
/** @brief Shutdown function called during loader unload. / Loader 卸载时调用的清理函数。 */
typedef void (*ChuModShutdownFunc)(void);
/** @brief Optional display name export. / 可选显示名导出。 */
typedef const char* (*ChuModNameFunc)(void);
/** @brief Optional comma-separated dependency list. / 可选逗号分隔依赖列表。 */
typedef const char* (*ChuModDependsFunc)(void);
/** @brief Optional mod version string. / 可选 mod 版本字符串。 */
typedef const char* (*ChuModVersionFunc)(void);
/** @brief Optional mod author string. / 可选 mod 作者字符串。 */
typedef const char* (*ChuModAuthorFunc)(void);
/** @brief Optional minimum loader version requirement, e.g. "2.1.0". / 可选最低 Loader 版本要求。 */
typedef const char* (*ChuModMinLoaderVersionFunc)(void);

#define CHUMOD_INIT_NAME     "chumod_init"
#define CHUMOD_ON_READY_NAME "chumod_on_ready"
#define CHUMOD_ON_FRAME_NAME "chumod_on_frame"
#define CHUMOD_SHUTDOWN_NAME "chumod_shutdown"
#define CHUMOD_NAME_NAME     "chumod_name"
#define CHUMOD_DEPENDS_NAME  "chumod_depends"
#define CHUMOD_VERSION_NAME  "chumod_version"
#define CHUMOD_AUTHOR_NAME   "chumod_author"
#define CHUMOD_MIN_LOADER_VERSION_NAME "chumod_min_loader_version"

/**
 * @brief Dual-mode helper: loader mode uses chumod_init; standalone injection falls back to a delayed thread.
 * @brief 双模式辅助：Loader 加载时走 chumod_init；独立注入时用延迟线程兜底启动。
 */
#define CHUMOD_DUAL_MODE(init_func) \
    static int g_chumod_api_called = 0; \
    static ChuModAPI g_chumod_fallback_api = {}; \
    static ChuModInfo g_chumod_fallback_info = {}; \
    CHUMOD_API int chumod_init(const ChuModInfo* info, const ChuModAPI* api) { \
        g_chumod_api_called = 1; \
        return init_func(info, api); \
    } \
    static DWORD WINAPI chumod_fallback_thread(LPVOID) { \
        Sleep(3000); \
        if (!g_chumod_api_called) { \
            HMODULE game = GetModuleHandleA("chusanApp.exe"); \
            if (game) { \
                g_chumod_fallback_info.api_version = CHUMOD_API_VERSION; \
                g_chumod_fallback_info.loader_version = "standalone"; \
                g_chumod_fallback_info.game_module = "chusanApp.exe"; \
                g_chumod_fallback_info.game_base = (uintptr_t)game; \
                PIMAGE_DOS_HEADER dos = (PIMAGE_DOS_HEADER)game; \
                PIMAGE_NT_HEADERS nt = (PIMAGE_NT_HEADERS)((uintptr_t)game + dos->e_lfanew); \
                g_chumod_fallback_info.game_size = nt->OptionalHeader.SizeOfImage; \
            } \
            g_chumod_fallback_api.struct_size = sizeof(ChuModAPI); \
            init_func(&g_chumod_fallback_info, &g_chumod_fallback_api); \
        } \
        return 0; \
    }

/** @brief Start dual-mode fallback thread from DllMain DLL_PROCESS_ATTACH. / 在 DllMain DLL_PROCESS_ATTACH 中启动双模式兜底线程。 */
#define CHUMOD_DUAL_MODE_START() \
    CreateThread(NULL, 0, chumod_fallback_thread, NULL, 0, NULL)

#ifdef __cplusplus
}
#endif
