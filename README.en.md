# ChuModLoader

English | [简体中文](README.md)

Mod loading framework for CHUNITHM. Proxies `version.dll` to load mod DLLs from `mods/` at runtime.

## Features

- `version.dll` proxy, forwards all 17 system exports
- Auto-scans `mods/*.dll` on startup
- Mod API (C ABI): memory read/write, AOB scan, inline hook, RTTI vtable lookup, inter-mod IPC
- TOML / INI config API
- Leveled logging (info/warn/error) + console colors + per-mod log files
- Dependency topological sorting
- Crash protection (catch_unwind)
- Game version detection + min_loader_version check
- Lifecycle: init → on_ready → on_frame → shutdown
- Hot reload (reload_mod API + reload.flag)
- Crash dump + stack trace
- Written in Rust; mods can be Rust, C/C++, or any language producing a Win32 DLL

## Installation

1. Build or download `version.dll`
2. Place it in the game directory (next to `chusanApp.exe`)
3. Drop mod DLLs into `mods/`
4. Launch the game normally

## Build

Requires Rust nightly with `i686-pc-windows-msvc` target:

```bash
cargo build --release
```

Output: `target/i686-pc-windows-msvc/release/version.dll`

## Mod Development

See [docs/mod-development.md](docs/mod-development.md) and [docs/api-reference.md](docs/api-reference.md).

Header: [include/chumod.h](include/chumod.h)

## License

Apache-2.0
