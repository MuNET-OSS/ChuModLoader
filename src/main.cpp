#include "loader.h"
#include <Windows.h>
#include <cstdio>

static HMODULE g_real_version = nullptr;
static FARPROC fp[17] = {};

extern "C" {
    __declspec(naked) void __stdcall p_GetFileVersionInfoA()       { __asm { jmp [fp+ 0] } }
    __declspec(naked) void __stdcall p_GetFileVersionInfoByHandle(){ __asm { jmp [fp+ 4] } }
    __declspec(naked) void __stdcall p_GetFileVersionInfoExA()     { __asm { jmp [fp+ 8] } }
    __declspec(naked) void __stdcall p_GetFileVersionInfoExW()     { __asm { jmp [fp+12] } }
    __declspec(naked) void __stdcall p_GetFileVersionInfoSizeA()   { __asm { jmp [fp+16] } }
    __declspec(naked) void __stdcall p_GetFileVersionInfoSizeExA() { __asm { jmp [fp+20] } }
    __declspec(naked) void __stdcall p_GetFileVersionInfoSizeExW() { __asm { jmp [fp+24] } }
    __declspec(naked) void __stdcall p_GetFileVersionInfoSizeW()   { __asm { jmp [fp+28] } }
    __declspec(naked) void __stdcall p_GetFileVersionInfoW()       { __asm { jmp [fp+32] } }
    __declspec(naked) void __stdcall p_VerFindFileA()              { __asm { jmp [fp+36] } }
    __declspec(naked) void __stdcall p_VerFindFileW()              { __asm { jmp [fp+40] } }
    __declspec(naked) void __stdcall p_VerInstallFileA()           { __asm { jmp [fp+44] } }
    __declspec(naked) void __stdcall p_VerInstallFileW()           { __asm { jmp [fp+48] } }
    __declspec(naked) void __stdcall p_VerLanguageNameA()          { __asm { jmp [fp+52] } }
    __declspec(naked) void __stdcall p_VerLanguageNameW()          { __asm { jmp [fp+56] } }
    __declspec(naked) void __stdcall p_VerQueryValueA()            { __asm { jmp [fp+60] } }
    __declspec(naked) void __stdcall p_VerQueryValueW()            { __asm { jmp [fp+64] } }
}

static const char* export_names[] = {
    "GetFileVersionInfoA", "GetFileVersionInfoByHandle",
    "GetFileVersionInfoExA", "GetFileVersionInfoExW",
    "GetFileVersionInfoSizeA", "GetFileVersionInfoSizeExA",
    "GetFileVersionInfoSizeExW", "GetFileVersionInfoSizeW",
    "GetFileVersionInfoW",
    "VerFindFileA", "VerFindFileW",
    "VerInstallFileA", "VerInstallFileW",
    "VerLanguageNameA", "VerLanguageNameW",
    "VerQueryValueA", "VerQueryValueW"
};

static void load_real_version() {
    char sys_dir[MAX_PATH];
    GetSystemDirectoryA(sys_dir, MAX_PATH);
    char real_path[MAX_PATH];
    _snprintf_s(real_path, MAX_PATH, _TRUNCATE, "%s\\version.dll", sys_dir);

    g_real_version = LoadLibraryA(real_path);
    if (!g_real_version) return;

    for (int i = 0; i < 17; i++)
        fp[i] = GetProcAddress(g_real_version, export_names[i]);
}

BOOL APIENTRY DllMain(HMODULE h_module, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        DisableThreadLibraryCalls(h_module);
        load_real_version();
        CreateThread(nullptr, 0, [](LPVOID) -> DWORD {
            Sleep(2000);
            loader::load_mods();
            return 0;
        }, nullptr, 0, nullptr);
    } else if (reason == DLL_PROCESS_DETACH) {
        loader::unload_mods();
        if (g_real_version) FreeLibrary(g_real_version);
    }
    return TRUE;
}
