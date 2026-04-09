# ChuModLoader

Mod loading framework for CHUNITHM. Proxies `version.dll` to load mod DLLs from `mods/` at runtime.

> **[中文说明](README_cn.md)**

> **v2.0.0** — Loader rewritten from C++ to Rust. Mod C ABI unchanged; existing mod DLLs work without modification. Legacy C++ code at tag [`v1.0.0-cpp`](https://github.com/Applesaber/ChuModLoader/tree/v1.0.0-cpp).

## Features

- `version.dll` proxy, forwards all 17 exports to the real system DLL
- Auto-scans `mods/*.dll` on startup, no config needed
- Optional Mod API (`chumod_init`) with memory read/write, AOB scan, inline hook, inter-mod IPC; plain DLLs work too
- `mods.ini` to disable individual mods
- Inline hooking via [retour](https://crates.io/crates/retour)
- Written in Rust, mods can be written in Rust, C/C++, or any language that produces a Win32 DLL

## Installation

1. Build or download `version.dll`
2. Place it in the game directory (next to `chusanApp.exe`)
3. Drop mod DLLs into `mods/`
4. Launch the game normally

`mods/` directory and `mods.ini` are created automatically on first run.

## Configuration

`mods.ini` is auto-generated in the game directory. To disable a mod:

```ini
[mods]
mod_name.dll=0
```

Mods not listed (or set to `1`) are loaded by default.

## For Mod Developers

See [Mod Development Guide](docs/mod-development.md) and [API Reference](docs/api-reference.md).

### Quick Start (Rust)

```rust
use std::ffi::{c_char, c_void};

#[repr(C)]
pub struct ChuModInfo {
    pub api_version: u32,
    pub loader_version: *const c_char,
    pub game_module: *const c_char,
    pub game_base: usize,
    pub game_size: u32,
    pub text_base: usize,
    pub text_size: u32,
}

#[repr(C)]
pub struct ChuModAPI {
    pub struct_size: u32,
    pub log: Option<unsafe extern "C" fn(*const c_char, ...)>,
    // ... other fields
}

#[no_mangle]
pub extern "C" fn chumod_name() -> *const c_char {
    b"My Rust Mod\0".as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "C" fn chumod_init(
    info: *const ChuModInfo,
    _api: *const ChuModAPI,
) -> i32 {
    // your init code
    0
}

#[no_mangle]
pub extern "C" fn chumod_shutdown() {}
```

### Quick Start (C/C++)

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

All exports are optional. Plain DLL with `DllMain` only also works.

## Building

Requires Rust toolchain with `i686-pc-windows-msvc` target:

```bash
rustup toolchain install nightly --target i686-pc-windows-msvc
cargo +nightly build --release
```

Output: `target/i686-pc-windows-msvc/release/version.dll`

> **Legacy C++ build** (no longer maintained): The loader was previously built with CMake 3.15+ and MSVC (`cmake -B build -A Win32 && cmake --build build --config Release`).

## How It Works

Exploits Windows DLL search order (application directory before System32):

1. Loads the real `version.dll` from System32, forwards exports via naked JMP
2. Background thread waits 2s, scans `mods/*.dll`, calls `LoadLibrary` on each
3. Parses PE headers for `.text` section info, calls `chumod_init` on API-exporting mods
4. On exit, calls `chumod_shutdown` in reverse order, then `FreeLibrary`

## Logging

Output to `chusan_loader.log` and console if available. Format: `[HH:MM:SS.mmm] [loader] message`

## Project Structure

```
ChuModLoader/
├── include/chumod.h        # Mod API header (for C/C++ mods)
├── src/
│   ├── lib.rs               # version.dll proxy entry (DllMain + forward_dll)
│   ├── loader.rs            # mod scanning & loading
│   └── api_impl.rs          # API implementation (retour hooks, memory, IPC)
├── docs/
│   ├── mod-development.md
│   └── api-reference.md
├── Cargo.toml
└── build.rs
```

## License

MIT
