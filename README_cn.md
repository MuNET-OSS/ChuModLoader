# ChuModLoader

CHUNITHM 的 mod 加载框架。通过代理 `version.dll`，在游戏启动时自动从 `mods/` 目录加载 mod DLL。

> **[English](README.md)**

## 功能

- `version.dll` 代理劫持，转发全部 17 个导出函数
- 启动时自动扫描 `mods/*.dll`，无需配置
- 可选的 Mod API（`chumod_init`），提供内存读写、AOB 扫描、inline hook、mod 间通信；不实现也能加载
- `mods.ini` 按 mod 禁用
- 内置 MinHook

## 安装

1. 构建或下载 `version.dll`
2. 放到游戏目录（和 `chusanApp.exe` 同级）
3. 把 mod DLL 丢进 `mods/`
4. 正常启动游戏

`mods/` 目录和 `mods.ini` 首次运行时自动创建。

## 配置

`mods.ini` 自动生成在游戏目录，禁用 mod 时编辑：

```ini
[mods]
mod_name.dll=0
```

未列出（或设为 `1`）的 mod 默认加载。

## Mod 开发

详见 [Mod 开发指南](docs/mod-development_cn.md) 和 [API 参考](docs/api-reference_cn.md)。

### 快速上手

```c
#include "chumod.h"

CHUMOD_API const char* chumod_name() { return "My Mod"; }

CHUMOD_API int chumod_init(const ChuModInfo* info, const ChuModAPI* api) {
    api->log("game base = 0x%08X, .text = 0x%08X +0x%X",
             info->game_base, info->text_base, info->text_size);
    return 0;
}

CHUMOD_API void chumod_shutdown() { }
```

所有导出函数可选。只用 `DllMain` 的普通 DLL 也能加载。

## 构建

需要 CMake 3.15+ 和 MSVC（Win32/x86）：

```bash
cmake -B build -A Win32
cmake --build build --config Release
```

也可以用 Visual Studio 直接打开文件夹 — 已包含 `CMakePresets.json`。

输出：`build/bin/Release/version.dll`

## 工作原理

利用 Windows DLL 搜索顺序（程序目录优先于 System32）实现劫持：

1. 加载 System32 的真 `version.dll`，naked JMP 转发导出函数
2. 后台线程等 2 秒，扫描 `mods/*.dll`，逐个 `LoadLibrary`
3. 解析 PE 头拿 `.text` 段信息，对导出了 ChuMod API 的 mod 调用 `chumod_init`
4. 退出时逆序调用 `chumod_shutdown`，再 `FreeLibrary`

## 日志

输出到 `chusan_loader.log`，有控制台时同步输出。格式：`[HH:MM:SS.mmm] [loader] message`

## 项目结构

```
ChuModLoader/
├── include/chumod.h           # Mod API 头文件
├── src/
│   ├── main.cpp                # version.dll 代理入口
│   ├── loader.cpp              # mod 扫描加载
│   ├── api_impl.cpp            # API 实现
│   └── version.def             # 导出定义
├── thirdparty/minhook/
├── docs/
│   ├── mod-development_cn.md   # 开发指南
│   └── api-reference_cn.md     # API 参考
├── CMakeLists.txt
└── CMakePresets.json
```

