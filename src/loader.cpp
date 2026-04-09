#include "loader.h"
#include "../include/chumod.h"

#include <Windows.h>

#include <algorithm>
#include <cstdarg>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>

namespace {

struct LoadedMod {
    HMODULE handle;
    ChuModShutdownFunc shutdown;
    char name[64];
};

std::vector<LoadedMod> g_loaded_mods;
bool g_loaded = false;
char g_base_dir[MAX_PATH] = {0};
FILE* g_log_fp = nullptr;
HANDLE g_console = INVALID_HANDLE_VALUE;

void path_directory(char* path) {
    const size_t length = strlen(path);
    for (size_t i = length; i > 0; --i) {
        if (path[i - 1] == '\\' || path[i - 1] == '/') {
            path[i - 1] = '\0';
            return;
        }
    }
}

void get_self_base_dir(char* out_dir, DWORD out_size) {
    HMODULE self_module = nullptr;
    GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                       reinterpret_cast<LPCSTR>(&get_self_base_dir),
                       &self_module);

    out_dir[0] = '\0';
    if (self_module == nullptr) {
        return;
    }

    if (GetModuleFileNameA(self_module, out_dir, out_size) == 0) {
        out_dir[0] = '\0';
        return;
    }

    path_directory(out_dir);
}

void join_path(char* out_path, DWORD out_size, const char* left, const char* right) {
    out_path[0] = '\0';
    if (left == nullptr || right == nullptr) {
        return;
    }

    const size_t left_len = strlen(left);
    const bool has_sep = left_len > 0 && (left[left_len - 1] == '\\' || left[left_len - 1] == '/');
    _snprintf_s(out_path,
                out_size,
                _TRUNCATE,
                has_sep ? "%s%s" : "%s\\%s",
                left,
                right);
}

void write_log(const char* fmt, ...) {
    char buf[512];
    SYSTEMTIME st = {};
    GetLocalTime(&st);
    int prefix = _snprintf_s(buf, sizeof(buf), _TRUNCATE,
        "[%02u:%02u:%02u.%03u] [loader] ",
        st.wHour, st.wMinute, st.wSecond, st.wMilliseconds);

    va_list args;
    va_start(args, fmt);
    int body = vsnprintf(buf + prefix, sizeof(buf) - prefix - 2, fmt, args);
    va_end(args);
    if (body < 0) body = 0;
    int total = prefix + body;
    buf[total++] = '\n';
    buf[total] = '\0';

    if (g_log_fp) { fputs(buf, g_log_fp); fflush(g_log_fp); }

    if (g_console != INVALID_HANDLE_VALUE) {
        DWORD written;
        WriteConsoleA(g_console, buf, total, &written, nullptr);
    }
}

bool is_mod_enabled(const char* ini_path, const char* mod_name) {
    char value[32] = {0};
    GetPrivateProfileStringA("mods", mod_name, "", value, static_cast<DWORD>(sizeof(value)), ini_path);

    if (value[0] == '\0') {
        return true;
    }

    return atoi(value) != 0;
}

} 

namespace loader {

void load_mods() {
    if (g_loaded) {
        return;
    }
    g_loaded = true;

    g_console = GetStdHandle(STD_OUTPUT_HANDLE);
    if (g_console == NULL || g_console == INVALID_HANDLE_VALUE) {
        AttachConsole(ATTACH_PARENT_PROCESS);
        g_console = GetStdHandle(STD_OUTPUT_HANDLE);
    }

    get_self_base_dir(g_base_dir, MAX_PATH);
    if (g_base_dir[0] == '\0') {
        return;
    }

    char log_path[MAX_PATH] = {0};
    join_path(log_path, MAX_PATH, g_base_dir, "chusan_loader.log");
    fopen_s(&g_log_fp, log_path, "w");

    char mods_dir[MAX_PATH] = {0};
    char ini_path[MAX_PATH] = {0};
    char pattern[MAX_PATH] = {0};
    join_path(mods_dir, MAX_PATH, g_base_dir, "mods");
    join_path(ini_path, MAX_PATH, g_base_dir, "mods.ini");
    join_path(pattern, MAX_PATH, mods_dir, "*.dll");

    write_log("loader start: base=%s", g_base_dir);
    write_log("scan mods dir: %s", mods_dir);
    write_log("config file: %s", ini_path);

    WIN32_FIND_DATAA find_data = {};
    HANDLE find_handle = FindFirstFileA(pattern, &find_data);
    if (find_handle == INVALID_HANDLE_VALUE) {
        write_log("no mods found or cannot open directory");
        return;
    }

    do {
        if ((find_data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0) {
            continue;
        }

        const char* mod_name = find_data.cFileName;
        char full_path[MAX_PATH] = {0};
        join_path(full_path, MAX_PATH, mods_dir, mod_name);

        if (!is_mod_enabled(ini_path, mod_name)) {
            write_log("mod disabled: %s", full_path);
            continue;
        }

        HMODULE mod_handle = nullptr;
        __try {
            mod_handle = LoadLibraryA(full_path);
        } __except (EXCEPTION_EXECUTE_HANDLER) {
            mod_handle = nullptr;
        }

        if (mod_handle == nullptr) {
            write_log("failed to load mod: %s (err=%lu)", full_path, GetLastError());
            continue;
        }

        g_loaded_mods.push_back({mod_handle, nullptr, ""});
        auto& mod = g_loaded_mods.back();
        _snprintf_s(mod.name, sizeof(mod.name), _TRUNCATE, "%s", mod_name);

        auto name_fn = reinterpret_cast<ChuModNameFunc>(GetProcAddress(mod_handle, CHUMOD_NAME_NAME));
        if (name_fn) {
            const char* n = name_fn();
            if (n) _snprintf_s(mod.name, sizeof(mod.name), _TRUNCATE, "%s", n);
        }

        auto init_fn = reinterpret_cast<ChuModInitFunc>(GetProcAddress(mod_handle, CHUMOD_INIT_NAME));
        if (init_fn) {
            ChuModInfo info = {};
            info.api_version = 1;
            info.loader_version = "0.1";
            HMODULE game = GetModuleHandleA("chusanApp.exe");
            if (game) {
                info.game_module = "chusanApp.exe";
                info.game_base = reinterpret_cast<uintptr_t>(game);
                auto dos = reinterpret_cast<PIMAGE_DOS_HEADER>(game);
                auto nt = reinterpret_cast<PIMAGE_NT_HEADERS>(info.game_base + dos->e_lfanew);
                info.game_size = nt->OptionalHeader.SizeOfImage;
            }
            int ret = init_fn(&info, write_log);
            if (ret != 0) {
                write_log("mod init failed (ret=%d): %s", ret, mod.name);
                FreeLibrary(mod_handle);
                g_loaded_mods.pop_back();
                continue;
            }
        }

        mod.shutdown = reinterpret_cast<ChuModShutdownFunc>(GetProcAddress(mod_handle, CHUMOD_SHUTDOWN_NAME));
        write_log("loaded mod: %s", mod.name);
    } while (FindNextFileA(find_handle, &find_data) != 0);

    FindClose(find_handle);
    write_log("mods loaded: %u", static_cast<unsigned>(g_loaded_mods.size()));
}

void unload_mods() {
    for (auto it = g_loaded_mods.rbegin(); it != g_loaded_mods.rend(); ++it) {
        if (it->shutdown) it->shutdown();
        if (it->handle) FreeLibrary(it->handle);
    }

    g_loaded_mods.clear();
    g_loaded = false;
    write_log("loader shutdown");
    if (g_log_fp) { fclose(g_log_fp); g_log_fp = nullptr; }
}

}
