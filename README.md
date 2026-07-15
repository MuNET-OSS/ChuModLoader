# ChuModLoader

> [!WARNING]
> ### DEPRECATED
>
> 现在，ChuModLoader 的全部功能已经移动到 [AppleChu](https://github.com/MuNET-OSS/AppleChu)，请直接安装 AppleChu 即可使用全部功能。

<p align="center">
  <strong>CHUNITHM Mod 加载框架</strong>
</p>

<p align="center">
  <a href="#许可证"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/platform-Windows%20x86-lightgrey.svg" alt="Platform">
  <img src="https://img.shields.io/badge/rust-nightly-orange.svg" alt="Rust">
  <img src="https://img.shields.io/badge/ABI-v4-green.svg" alt="ABI Version">
</p>

通过代理 `winhttp.dll`，在游戏启动时自动从 `mods/` 目录加载 Mod DLL。框架使用 Rust 编写，Mod 可用 Rust 或 C/C++ 开发

---

## 目录

- [ChuModLoader](#chumodloader)
  - [目录](#目录)
  - [功能特性](#功能特性)
  - [安装](#安装)
  - [构建](#构建)
  - [Mod 开发](#mod-开发)
  - [文档](#文档)
  - [许可证](#许可证)

## 功能特性

**DLL 代理** 代理 `winhttp.dll`，可以不需要注入脚本直接启动游戏
**自动加载** 启动时自动扫描并加载 `mods/*.dll`

## 安装

1. 构建或下载 `winhttp.dll`
2. 放到游戏目录（和 `chusanApp.exe` 同级）
3. 把 Mod DLL 放进 `mods/` 目录
4. 正常启动游戏

加载日志会写入 `chumod_loader.log`，单 Mod 日志位于 `mods/log/`，崩溃报告位于 `mods/crash/`。

## 构建

需要 Rust nightly 工具链 + `i686-pc-windows-msvc` target（仓库已通过 `rust-toolchain.toml` 固定）：

```bash
cargo build --release
```

输出文件：

```text
target/i686-pc-windows-msvc/release/winhttp.dll
```

> [!NOTE]
> 游戏为 32 位程序，必须使用 `i686-pc-windows-msvc` target，不能用默认的 64 位 target。

## Mod 开发

最小的 C/C++ Mod 示例：

```c
#include "chumod.h"

static const ChuModAPI* g_api = NULL;

CHUMOD_API const char* chumod_name(void)    { return "Example Mod"; }
CHUMOD_API const char* chumod_version(void) { return "1.0.0"; }

CHUMOD_API int chumod_init(const ChuModInfo* info, const ChuModAPI* api) {
    (void)info;
    g_api = api;
    g_api->log_info("Example Mod 已初始化");
    return 0; // 返回 0 保留加载，非 0 跳过卸载
}

CHUMOD_API void chumod_shutdown(void) {
    if (g_api) g_api->log_info("Example Mod 已卸载");
}
```

将其编译为 32 位 Windows DLL 后放入 `mods/` 即可。完整的 Rust / C++ 示例、生命周期、配置、热重载等说明见开发文档。

## 文档

- [Mod 开发指南](docs/mod-development.md) — 三种 Mod 编写方式、生命周期、配置、依赖、热重载、崩溃保护
- [API 参考](docs/api-reference.md) — 完整 C ABI 函数表与导出说明
- [C/C++ 头文件](include/chumod.h) — `chumod.h`

## 许可证

[Apache-2.0](LICENSE)
