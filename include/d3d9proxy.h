#ifndef D3D9PROXY_H
#define D3D9PROXY_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void (*D3D9ProxyPresentCallback)(void* device);

typedef struct D3D9ProxyAPI {
    void (*set_frame_lock)(uint32_t fps);
    void* (*get_device)(void);
    uintptr_t (*get_hwnd)(void);
    void (*register_present_callback)(D3D9ProxyPresentCallback callback);
} D3D9ProxyAPI;

#ifdef __cplusplus
}
#endif

#endif
