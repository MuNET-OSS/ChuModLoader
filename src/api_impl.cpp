#include "api_impl.h"
#include <MinHook.h>
#include <Windows.h>
#include <cstring>
#include <unordered_map>
#include <vector>
#include <string>
#include <mutex>

namespace {

std::mutex g_svc_mutex;
std::unordered_map<std::string, void*> g_services;

struct Subscriber {
    std::string topic;
    ChuModMessageCallback cb;
};
std::mutex g_msg_mutex;
std::vector<Subscriber> g_subscribers;

uintptr_t api_aob_scan(uintptr_t start, uint32_t size, const uint8_t* pat, const char* mask) {
    size_t len = strlen(mask);
    if (size < len) return 0;
    for (size_t i = 0; i <= size - len; i++) {
        auto mem = reinterpret_cast<const uint8_t*>(start + i);
        bool ok = true;
        for (size_t j = 0; j < len; j++) {
            if (mask[j] == 'x' && mem[j] != pat[j]) { ok = false; break; }
        }
        if (ok) return start + i;
    }
    return 0;
}

int api_mem_read(uintptr_t addr, void* buf, uint32_t size) {
    DWORD old_protect;
    if (!VirtualProtect(reinterpret_cast<void*>(addr), size, PAGE_EXECUTE_READWRITE, &old_protect))
        return -1;
    memcpy(buf, reinterpret_cast<void*>(addr), size);
    VirtualProtect(reinterpret_cast<void*>(addr), size, old_protect, &old_protect);
    return 0;
}

int api_mem_write(uintptr_t addr, const void* buf, uint32_t size) {
    DWORD old_protect;
    if (!VirtualProtect(reinterpret_cast<void*>(addr), size, PAGE_EXECUTE_READWRITE, &old_protect))
        return -1;
    memcpy(reinterpret_cast<void*>(addr), buf, size);
    VirtualProtect(reinterpret_cast<void*>(addr), size, old_protect, &old_protect);
    return 0;
}

int api_mem_fill(uintptr_t addr, uint8_t value, uint32_t size) {
    DWORD old_protect;
    if (!VirtualProtect(reinterpret_cast<void*>(addr), size, PAGE_EXECUTE_READWRITE, &old_protect))
        return -1;
    memset(reinterpret_cast<void*>(addr), value, size);
    VirtualProtect(reinterpret_cast<void*>(addr), size, old_protect, &old_protect);
    return 0;
}

int api_hook_create(void* target, void* detour, void** original) {
    return MH_CreateHook(target, detour, original) == MH_OK ? 0 : -1;
}

int api_hook_enable(void* target) {
    return MH_EnableHook(target) == MH_OK ? 0 : -1;
}

int api_hook_disable(void* target) {
    return MH_DisableHook(target) == MH_OK ? 0 : -1;
}

int api_hook_remove(void* target) {
    return MH_RemoveHook(target) == MH_OK ? 0 : -1;
}

int api_register_service(const char* name, void* ptr) {
    std::lock_guard<std::mutex> lock(g_svc_mutex);
    g_services[name] = ptr;
    return 0;
}

void* api_get_service(const char* name) {
    std::lock_guard<std::mutex> lock(g_svc_mutex);
    auto it = g_services.find(name);
    return it != g_services.end() ? it->second : nullptr;
}

int api_publish(const char* topic, void* data, uint32_t size) {
    std::lock_guard<std::mutex> lock(g_msg_mutex);
    for (auto& sub : g_subscribers) {
        if (sub.topic == topic) {
            sub.cb(topic, data, size);
        }
    }
    return 0;
}

int api_subscribe(const char* topic, ChuModMessageCallback callback) {
    std::lock_guard<std::mutex> lock(g_msg_mutex);
    g_subscribers.push_back({topic, callback});
    return 0;
}

ChuModAPI g_api = {};

}

namespace api {

void init() {
    MH_Initialize();

    g_api.struct_size = sizeof(ChuModAPI);
    g_api.log = nullptr;

    g_api.aob_scan = api_aob_scan;
    g_api.mem_read = api_mem_read;
    g_api.mem_write = api_mem_write;
    g_api.mem_fill = api_mem_fill;

    g_api.hook_create = api_hook_create;
    g_api.hook_enable = api_hook_enable;
    g_api.hook_disable = api_hook_disable;
    g_api.hook_remove = api_hook_remove;

    g_api.register_service = api_register_service;
    g_api.get_service = api_get_service;
    g_api.publish = api_publish;
    g_api.subscribe = api_subscribe;
}

void shutdown() {
    MH_DisableHook(MH_ALL_HOOKS);
    MH_Uninitialize();
    g_services.clear();
    g_subscribers.clear();
}

ChuModAPI* get_api() {
    return &g_api;
}

}
