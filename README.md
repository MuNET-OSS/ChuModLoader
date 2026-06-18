# ChuModLoader

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

- [功能特性](#功能特性)
- [工作原理](#工作原理)
- [安装](#安装)
- [目录结构](#目录结构)
- [构建](#构建)
- [Mod 开发](#mod-开发)
- [文档](#文档)
- [许可证](#许可证)

## 功能特性

| 分类 | 说明 |
| --- | --- |
| **DLL 代理** | 代理 `winhttp.dll`，转发系统 `winhttp.dll` 的全部导出，对游戏完全透明 |
| **自动加载** | 启动时自动扫描并加载 `mods/*.dll` |
| **内存操作** | 内存读写、填充、AOB 特征扫描 |
| **Hook** | Inline hook（创建 / 启用 / 禁用 / 移除）、RTTI vtable 查找 |
| **d3d9 服务** | 代理 Direct3D 9 设备，提供每帧回调、帧率锁定、设备 / 窗口句柄 |
| **Mod 间通信** | 命名服务查找 + 基于 topic 的发布 / 订阅 |
| **配置** | TOML / INI 单 Mod 配置读写 |
| **日志** | 分级日志（info / warn / error）+ 控制台颜色 + per-mod 日志文件 |
| **依赖管理** | 依赖声明 + 拓扑排序，确保加载顺序 |
| **生命周期** | `init → on_ready → on_frame → shutdown` 完整生命周期 |
| **热重载** | `reload_mod` API + `reload.flag` 文件触发 |
| **崩溃保护** | `catch_unwind` 包裹回调 + SEH 过滤器 + crash dump + 栈回溯 |
| **版本检测** | 游戏版本检测 + `min_loader_version` 兼容性检查 |

## 工作原理

`chusanApp.exe` 静态导入了系统 `winhttp.dll`。ChuModLoader 把自己编译成同名的 `winhttp.dll` 放在游戏目录，由于 DLL 搜索顺序优先于 `System32`，游戏启动时会自然加载它。框架再把所有导出转发给真正的系统 `winhttp.dll`，因此对游戏完全透明。

加载后，框架并不会立刻初始化（此时游戏自身尚未运行）。它在 `DllMain` 中**劫持游戏 EXE 的入口点**：改写入口处的指令，使游戏真正开始执行时先跳转到框架的 bootstrap，完成 Mod 加载、crash dump 安装、d3d9 代理注入后，再恢复原始入口点继续运行游戏。

```text
chusanApp.exe 启动
  └─ 加载游戏目录的 winhttp.dll（实为 ChuModLoader）
       ├─ 转发系统导出 → System32\winhttp.dll
       └─ DllMain：劫持游戏入口点
            游戏入口点首次执行
              └─ bootstrap
                   ├─ 安装 crash dump / SEH
                   ├─ 扫描 mods/*.dll，依赖拓扑排序
                   ├─ 逐个驱动 Mod 生命周期
                   │    init → on_ready → on_frame(循环) → shutdown
                   ├─ 注入 d3d9 设备代理
                   └─ 恢复原始入口点，游戏正常运行
```

## 安装

1. 构建或下载 `winhttp.dll`
2. 放到游戏目录（和 `chusanApp.exe` 同级）
3. 把 Mod DLL 放进 `mods/` 目录
4. 正常启动游戏

加载日志会写入 `chumod_loader.log`，单 Mod 日志位于 `mods/log/`，崩溃报告位于 `mods/crash/`。

## 目录结构

```text
ChuModLoader/
├─ Cargo.toml        # workspace（成员：chu-abi、loader）
├─ rust-toolchain.toml
├─ chu-abi/          # C ABI 定义（ChuModInfo / ChuModAPI 等共享类型）
│  ├─ Cargo.toml
│  └─ src/
├─ loader/           # 加载器主体，编译产物为 winhttp.dll
│  ├─ Cargo.toml
│  ├─ build.rs       # 生成 winhttp 导出转发
│  └─ src/
│     ├─ lib.rs      # 入口、winhttp 代理、游戏入口点劫持
│     ├─ loader/     # Mod 扫描、依赖排序、日志、生命周期、崩溃处理
│     ├─ api_impl/   # ChuModAPI 函数表实现
│     └─ d3d9/       # Direct3D 9 设备代理
├─ include/
│  └─ chumod.h       # C/C++ Mod 头文件
└─ docs/             # 开发文档
```

游戏目录下框架运行时使用的布局：

```text
游戏目录/
├─ chusanApp.exe
├─ winhttp.dll                       # ChuModLoader
├─ chumod_loader.log                 # 加载器日志
└─ mods/
   ├─ <mod>.dll                      # Mod 二进制
   ├─ config/<mod_name>.toml         # 单 Mod 配置（或 .ini）
   ├─ manifest/<mod_name>.toml       # 单 Mod manifest
   ├─ log/<mod_name>.log             # 单 Mod 日志
   ├─ crash/                         # 崩溃报告
   └─ reload.flag                    # 创建即触发全部热重载
```

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
