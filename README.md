# ChuModLoader

Mod loading framework for CHUNITHM. Proxies `version.dll` to load mod DLLs from `mods/` at runtime.

> **[中文说明](README_cn.md)**

## Features

- `version.dll` proxy, forwards all 17 exports to the real system DLL
- Auto-scans `mods/*.dll` on startup, no config needed
- Optional Mod API (`chumod_init`) with memory read/write, AOB scan, inline hook, inter-mod IPC; plain DLLs work too
- `mods.ini` to disable individual mods
- Built-in MinHook

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

### Quick Start

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

Requires CMake 3.15+ and MSVC (Win32/x86):

```bash
cmake -B build -A Win32
cmake --build build --config Release
```

Or open the folder in Visual Studio (`CMakePresets.json` included).

Output: `build/bin/Release/version.dll`

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
├── include/chumod.h        # Mod API header
├── src/
│   ├── main.cpp             # version.dll proxy entry
│   ├── loader.cpp           # mod scanning & loading
│   ├── api_impl.cpp         # API implementation
│   └── version.def          # export definitions
├── thirdparty/minhook/
├── docs/
│   ├── mod-development.md
│   └── api-reference.md
├── CMakeLists.txt
└── CMakePresets.json
```

## License

MIT
