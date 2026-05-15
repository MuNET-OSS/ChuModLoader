mod config_ini;
mod config_toml;
mod hook;
mod ipc;
mod log_api;
mod memory;
mod rtti;

use crate::types::ChuModAPI;

pub use config_ini::set_current_config;
pub use rtti::set_rtti_info;

static mut G_API: ChuModAPI = ChuModAPI {
    struct_size: 0,
    log: None,
    aob_scan: None,
    mem_read: None,
    mem_write: None,
    mem_fill: None,
    hook_create: None,
    hook_enable: None,
    hook_disable: None,
    hook_remove: None,
    register_service: None,
    get_service: None,
    publish: None,
    subscribe: None,
    rtti_find_vtable: None,
    config_get_int: None,
    config_get_float: None,
    config_get_bool: None,
    config_get_string: None,
    config_set_int: None,
    config_set_float: None,
    config_set_bool: None,
    config_set_string: None,
};

fn make_api() -> ChuModAPI {
    ChuModAPI {
        struct_size: std::mem::size_of::<ChuModAPI>() as u32,
        log: None,
        aob_scan: Some(memory::api_aob_scan),
        mem_read: Some(memory::api_mem_read),
        mem_write: Some(memory::api_mem_write),
        mem_fill: Some(memory::api_mem_fill),
        hook_create: Some(hook::api_hook_create),
        hook_enable: Some(hook::api_hook_enable),
        hook_disable: Some(hook::api_hook_disable),
        hook_remove: Some(hook::api_hook_remove),
        register_service: Some(ipc::api_register_service),
        get_service: Some(ipc::api_get_service),
        publish: Some(ipc::api_publish),
        subscribe: Some(ipc::api_subscribe),
        rtti_find_vtable: Some(rtti::api_rtti_find_vtable),
        config_get_int: Some(config_ini::api_config_get_int),
        config_get_float: Some(config_ini::api_config_get_float),
        config_get_bool: Some(config_ini::api_config_get_bool),
        config_get_string: Some(config_ini::api_config_get_string),
        config_set_int: Some(config_ini::api_config_set_int),
        config_set_float: Some(config_ini::api_config_set_float),
        config_set_bool: Some(config_ini::api_config_set_bool),
        config_set_string: Some(config_ini::api_config_set_string),
    }
}

pub unsafe fn init() {
    G_API = make_api();
}

pub unsafe fn shutdown() {
    hook::shutdown();
    ipc::shutdown();
}

pub unsafe fn get_api() -> *mut ChuModAPI {
    &raw mut G_API
}
