# Mod 开发指南

## 概述

Mod = Win32 DLL（32 位），放到 `mods/` 目录。Loader 启动时自动加载。

两种写法：

1. **普通 DLL** — 只用 `DllMain`。
2. **ChuMod API** — 导出 `chumod_init` 等函数，拿到游戏信息和工具 API。

可以混用。

## 最简示例（普通 DLL）

```c
#include <Windows.h>

BOOL APIENTRY DllMain(HMODULE h, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        MessageBoxA(NULL, "Mod loaded!", "Hello", MB_OK);
    }
    return TRUE;
}
```

编译后丢到 `mods/`。

## ChuMod API 示例

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

```c
const uint8_t sig[] = { 0x55, 0x8B, 0xEC, 0x83, 0xEC };
uintptr_t addr = g_api->aob_scan(info->text_base, info->text_size, sig, "xxxxx");
if (addr) {
    g_api->log("found at 0x%08X", addr);
}
```

### 内存读写

```c
uint32_t value;
g_api->mem_read(0x12345678, &value, sizeof(value));

uint8_t nop = 0x90;
g_api->mem_write(0x12345678, &nop, 1);

g_api->mem_fill(0x12345678, 0x90, 5);  // NOP 5 字节
```

页保护自动处理。
### Hook

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

引用 `chumod.h`，复制到项目或加 include path。CMakeLists.txt 示例：

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

- 必须 `-A Win32`，游戏是 32 位
- 不要阻塞 `DllMain`，耗时操作放 `chumod_init` 或新线程
- 用 AOB 扫描而不是硬编码地址，游戏更新会变
- `chumod_shutdown` 里清理所有 hook 和资源
- 双模式下 API 指针可能为 NULL

## 参考

- [API 参考](api-reference_cn.md)
- [chumod.h](../include/chumod.h)
