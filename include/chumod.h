#pragma once

// ChuModLoader Mod API v0.1
// 社区 mod 实现以下导出函数即可被 loader 自动识别和管理
// 所有函数均为可选——不导出则 loader 跳过对应阶段

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>

#define CHUMOD_API __declspec(dllexport)

typedef struct {
    uint32_t api_version;
    const char* loader_version;
    const char* game_module;
    uintptr_t game_base;
    uint32_t game_size;
} ChuModInfo;

typedef void (*ChuModLogFunc)(const char* fmt, ...);

// mod 导出函数签名
// loader 按顺序调用: chumod_init → (游戏运行) → chumod_shutdown

// 初始化：返回 0 表示成功，非 0 表示失败（loader 会卸载该 mod）
typedef int (*ChuModInitFunc)(const ChuModInfo* info, ChuModLogFunc log);

// 关闭：清理资源
typedef void (*ChuModShutdownFunc)(void);

// mod 名称（可选，用于日志显示）
typedef const char* (*ChuModNameFunc)(void);

// 导出函数名
#define CHUMOD_INIT_NAME     "chumod_init"
#define CHUMOD_SHUTDOWN_NAME "chumod_shutdown"
#define CHUMOD_NAME_NAME     "chumod_name"

#ifdef __cplusplus
}
#endif
