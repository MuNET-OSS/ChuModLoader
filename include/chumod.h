#pragma once

// ChuModLoader Mod API v1.0.0

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <Windows.h>

#define CHUMOD_API __declspec(dllexport)
#define CHUMOD_API_VERSION 1

// --- 基础信息 ---

typedef struct {
    uint32_t api_version;
    const char* loader_version;
    const char* game_module;
    uintptr_t game_base;
    uint32_t game_size;
    uintptr_t text_base;
    uint32_t text_size;
} ChuModInfo;

// --- 日志 ---

typedef void (*ChuModLogFunc)(const char* fmt, ...);

// --- 内存 ---

// mask: 'x' 精确匹配, '?' 通配; 返回地址或 0
typedef uintptr_t (*ChuModAobScanFunc)(uintptr_t start, uint32_t size,
                                        const uint8_t* pattern, const char* mask);

// 自动处理页保护, 返回 0 成功
typedef int (*ChuModMemReadFunc)(uintptr_t addr, void* buf, uint32_t size);
typedef int (*ChuModMemWriteFunc)(uintptr_t addr, const void* buf, uint32_t size);
typedef int (*ChuModMemFillFunc)(uintptr_t addr, uint8_t value, uint32_t size);

// --- Hook (MinHook) ---

// 返回 0 成功
typedef int (*ChuModHookCreateFunc)(void* target, void* detour, void** original);
typedef int (*ChuModHookEnableFunc)(void* target);
typedef int (*ChuModHookDisableFunc)(void* target);
typedef int (*ChuModHookRemoveFunc)(void* target);

// --- Mod 间通信 ---

// 命名服务: mod 注册指针, 其他 mod 按名字取
typedef int (*ChuModRegisterServiceFunc)(const char* name, void* service_ptr);
typedef void* (*ChuModGetServiceFunc)(const char* name);

// 消息总线
typedef void (*ChuModMessageCallback)(const char* topic, void* data, uint32_t size);
typedef int (*ChuModPublishFunc)(const char* topic, void* data, uint32_t size);
typedef int (*ChuModSubscribeFunc)(const char* topic, ChuModMessageCallback callback);

// --- API 函数表 (loader -> mod) ---

typedef struct {
    uint32_t struct_size;

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
} ChuModAPI;

// --- Mod 导出函数 ---
// 调用顺序: chumod_init -> (运行) -> chumod_shutdown

// 返回 0 成功, 非 0 则 loader 卸载此 mod
typedef int (*ChuModInitFunc)(const ChuModInfo* info, const ChuModAPI* api);
typedef void (*ChuModShutdownFunc)(void);
// 可选
typedef const char* (*ChuModNameFunc)(void);
// 逗号分隔的依赖 mod 名, loader 保证先加载
typedef const char* (*ChuModDependsFunc)(void);

#define CHUMOD_INIT_NAME     "chumod_init"
#define CHUMOD_SHUTDOWN_NAME "chumod_shutdown"
#define CHUMOD_NAME_NAME     "chumod_name"
#define CHUMOD_DEPENDS_NAME  "chumod_depends"

// --- 双模式兼容 ---
// loader 加载时走 chumod_init, inject -k 时 fallback 自启动

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

// DllMain DLL_PROCESS_ATTACH 里调用
#define CHUMOD_DUAL_MODE_START() \
    CreateThread(NULL, 0, chumod_fallback_thread, NULL, 0, NULL)

#ifdef __cplusplus
}
#endif
