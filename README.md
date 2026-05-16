# ChuModLoader

[English](README.en.md) | 简体中文

CHUNITHM Mod 加载框架。通过代理 `version.dll`，在游戏启动时自动从 `mods/` 目录加载 mod DLL。

## 功能

- `version.dll` 代理，转发全部 17 个系统导出
- 自动扫描 `mods/*.dll` 加载
- Mod API (C ABI)：内存读写、AOB 扫描、inline hook、RTTI vtable 查找、mod 间通信
- TOML / INI 配置 API
- 分级日志 (info/warn/error) + 控制台颜色 + per-mod 日志文件
- 依赖拓扑排序
- 崩溃保护 (catch_unwind)
- 游戏版本检测 + min_loader_version 检查
- 生命周期: init → on_ready → on_frame → shutdown
- 热重载 (reload_mod API + reload.flag)
- crash dump + 栈回溯
- Rust 编写，mod 可用 Rust、C/C++ 或任何能编译 Win32 DLL 的语言

## 安装

1. 构建或下载 `version.dll`
2. 放到游戏目录（和 `chusanApp.exe` 同级）
3. 把 mod DLL 放进 `mods/`
4. 正常启动游戏

## 构建

需要 Rust nightly + `i686-pc-windows-msvc`：

```bash
cargo build --release
```

输出: `target/i686-pc-windows-msvc/release/version.dll`

## Mod 开发

参见 [docs/mod-development.md](docs/mod-development.md) 和 [docs/api-reference.md](docs/api-reference.md)。

头文件: [include/chumod.h](include/chumod.h)

## 许可证

Apache-2.0
