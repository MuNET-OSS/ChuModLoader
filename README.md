# ChuModLoader

CHUNITHM mod 加载框架。通过 `version.dll` 代理劫持，自动加载 `mods/` 目录下的所有 mod DLL。

## 安装

1. 将 `version.dll` 放到游戏目录（`chusanApp.exe` 同级）
2. 在游戏目录创建 `mods/` 文件夹
3. 将 mod DLL 放入 `mods/`
4. 正常启动游戏

## 禁用指定 mod

在游戏目录创建 `mods.ini`：

```ini
[mods]
某个mod.dll=0
```

## Mod API

Mod 可以选择实现 `chumod.h` 中定义的接口，获得生命周期管理和日志能力。**不实现也能加载**——普通 DLL 照常工作。

```c
#include "chumod.h"

CHUMOD_API const char* chumod_name() {
    return "My Mod";
}

CHUMOD_API int chumod_init(const ChuModInfo* info, ChuModLogFunc log) {
    log("hello from my mod! game base=0x%08X", info->game_base);
    return 0; // 0=成功
}

CHUMOD_API void chumod_shutdown() {
    // 清理
}
```

### API 函数（均为可选）

| 导出函数 | 签名 | 说明 |
|----------|------|------|
| `chumod_name` | `const char*()` | 返回 mod 名称 |
| `chumod_init` | `int(ChuModInfo*, ChuModLogFunc)` | 初始化，返回 0 表示成功 |
| `chumod_shutdown` | `void()` | 游戏退出时清理资源 |

### ChuModInfo

| 字段 | 类型 | 说明 |
|------|------|------|
| `api_version` | `uint32_t` | API 版本（当前为 1） |
| `loader_version` | `const char*` | Loader 版本号 |
| `game_module` | `const char*` | 游戏模块名 |
| `game_base` | `uintptr_t` | 游戏模块基址 |
| `game_size` | `uint32_t` | 游戏模块大小 |

## 构建

需要 CMake + MSVC（Win32/x86）：

```bash
cmake -B build -A Win32
cmake --build build --config Release
```

产物在 `build/bin/Release/version.dll`。

## 原理

`version.dll` 是 Windows 系统 DLL，几乎所有程序启动时都会加载。Windows 优先从程序目录搜索，因此放在游戏目录的 `version.dll` 会被优先加载。

Loader 在 `DllMain` 中：
1. 从 `System32` 加载真实的 `version.dll`，转发所有 17 个导出函数
2. 启动后台线程，等待游戏初始化完成后扫描 `mods/` 目录
3. 依次 `LoadLibrary` 每个 mod DLL，调用 Mod API 接口

## 日志

运行时日志输出到游戏目录的 `chusan_loader.log`，同时输出到控制台（如果有）。

## License

MIT
