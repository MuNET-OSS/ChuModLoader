# API 参考

`chumod.h` — ChuModLoader Mod API v1.0.0

## 常量

| 名称 | 值 | 说明 |
|------|------|------|
| `CHUMOD_API_VERSION` | `1` | 当前 API 版本号 |
| `CHUMOD_API` | `__declspec(dllexport)` | mod 函数的导出标记 |

## ChuModInfo

传给 `chumod_init` 的游戏进程信息。

```c
typedef struct {
    uint32_t    api_version;     // CHUMOD_API_VERSION
    const char* loader_version;  // 如 "1.0.0"，双模式独立运行时为 "standalone"
    const char* game_module;     // "chusanApp.exe"
    uintptr_t   game_base;       // 游戏模块基址
    uint32_t    game_size;       // PE 头中的 SizeOfImage
    uintptr_t   text_base;       // .text 段起始地址
    uint32_t    text_size;       // .text 段大小
} ChuModInfo;
```

`text_base` 和 `text_size` 用于限定 AOB 扫描范围到可执行代码区域。

## ChuModAPI

`chumod_init` 传入的函数表，全局保存使用。

返回值：`0` 成功，非零失败（除特别说明）。

---

### 日志

#### `log(const char* fmt, ...)`

printf 风格日志，输出到 `chusan_loader.log` 和控制台。

```c
api->log("player score: %d", score);
api->log("hook target: 0x%08X", addr);
```

> 双模式独立运行时 `log` 为 NULL。

---

### 内存 — 特征码扫描

#### `aob_scan(uintptr_t start, uint32_t size, const uint8_t* pattern, const char* mask) → uintptr_t`

扫描字节序列，返回首个匹配地址，未找到返回 `0`。

- `start` — 扫描起始地址（一般用 `info->text_base`）
- `size` — 扫描的字节数（一般用 `info->text_size`）
- `pattern` — 要匹配的字节数组
- `mask` — 字符掩码，长度和 pattern 相同：
  - `'x'` — 精确匹配
  - `'?'` — 通配，任何字节都匹配

```c
// 查找函数开头: push ebp; mov ebp,esp; sub esp,??
const uint8_t sig[] = { 0x55, 0x8B, 0xEC, 0x83, 0xEC };
uintptr_t addr = api->aob_scan(info->text_base, info->text_size, sig, "xxxxx");
```

---

### 内存 — 读 / 写 / 填充

自动处理页保护（`VirtualProtect`）。

#### `mem_read(uintptr_t addr, void* buf, uint32_t size) → int`

```c
uint32_t value;
api->mem_read(target, &value, sizeof(value));
```

#### `mem_write(uintptr_t addr, const void* buf, uint32_t size) → int`

```c
// 把条件跳转改成无条件跳转
uint8_t jmp = 0xEB;
api->mem_write(branch_addr, &jmp, 1);
```

#### `mem_fill(uintptr_t addr, uint8_t value, uint32_t size) → int`

```c
// NOP 掉一条 5 字节的 CALL 指令
api->mem_fill(call_addr, 0x90, 5);
```

---

### Hook

基于 [retour](https://crates.io/crates/retour)。`hook_create` 后需要 `hook_enable` 激活。

#### `hook_create(void* target, void* detour, void** original) → int`

#### `hook_enable(void* target) → int`

#### `hook_disable(void* target) → int`

#### `hook_remove(void* target) → int`

**完整示例：**

```c
typedef int (__stdcall *CheckFunc_t)(void* self);
static CheckFunc_t orig_check = NULL;

int __stdcall hook_check(void* self) {
    return orig_check(self);
}

api->hook_create((void*)target, (void*)hook_check, (void**)&orig_check);
api->hook_enable((void*)target);

// cleanup
api->hook_disable((void*)target);
api->hook_remove((void*)target);
```

---

### Mod 间通信 — 服务

命名指针注册表，线程安全。

#### `register_service(const char* name, void* service_ptr) → int`

#### `get_service(const char* name) → void*`

返回注册的指针，未找到返回 `NULL`。

**示例：**

```c
// --- 提供方 mod ---
struct ScoreService {
    int version;
    int (*get_score)(void);
};

static struct ScoreService svc = { 1, my_get_score };
api->register_service("score_service", &svc);

// --- 客户方 mod ---
struct ScoreService* s = (struct ScoreService*)api->get_service("score_service");
if (s && s->version >= 1) {
    int score = s->get_score();
}
```

> 依赖其他 mod 的服务时用 `chumod_depends` 保证加载顺序。

---

### Mod 间通信 — 消息

发布/订阅消息总线，同步调用，注册线程安全。

#### `publish(const char* topic, void* data, uint32_t size) → int`

向 `topic` 所有订阅者同步发送数据。

#### `subscribe(const char* topic, ChuModMessageCallback callback) → int`

**回调：**

```c
void callback(const char* topic, void* data, uint32_t size);
```

**示例：**

```c
// 订阅
void on_event(const char* topic, void* data, uint32_t size) {
    int value = *(int*)data;
    api->log("received %s: %d", topic, value);
}
api->subscribe("game_event", on_event);

// 发布
int payload = 42;
api->publish("game_event", &payload, sizeof(payload));
```

---

## Mod 导出函数

均为可选，loader 通过 `GetProcAddress` 查找。

### `chumod_name() → const char*`

Mod 显示名称，用于日志。不导出则用文件名。

### `chumod_init(const ChuModInfo* info, const ChuModAPI* api) → int`

初始化入口。返回 `0` 成功，非零则 loader 卸载该 mod。

### `chumod_shutdown()`

退出时清理，按加载逆序调用。

### `chumod_depends() → const char*`

逗号分隔的依赖列表，loader 保证先加载。

```c
CHUMOD_API const char* chumod_depends() {
    return "base_mod,utility_mod";
}
```

---

## 双模式宏

兼容 loader 和 `inject -k` 两种加载方式。

### `CHUMOD_DUAL_MODE(init_func)`

生成 `chumod_init` 导出和后备线程。独立注入时等 3 秒后调 `init_func`，此时 `ChuModAPI` 字段全为 NULL。

### `CHUMOD_DUAL_MODE_START()`

在 `DllMain` `DLL_PROCESS_ATTACH` 中调用。

```c
#include "chumod.h"

static int my_init(const ChuModInfo* info, const ChuModAPI* api) {
    if (api->log) api->log("hello");  // 双模式下检查 NULL
    return 0;
}

CHUMOD_DUAL_MODE(my_init)

BOOL APIENTRY DllMain(HMODULE h, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        DisableThreadLibraryCalls(h);
        CHUMOD_DUAL_MODE_START();
    }
    return TRUE;
}
```

---

## 符号名称

用于手动 `GetProcAddress` 或 DEF 文件：

| 导出函数 | 字符串常量 |
|---------|-----------|
| `chumod_init` | `CHUMOD_INIT_NAME` |
| `chumod_shutdown` | `CHUMOD_SHUTDOWN_NAME` |
| `chumod_name` | `CHUMOD_NAME_NAME` |
| `chumod_depends` | `CHUMOD_DEPENDS_NAME` |
