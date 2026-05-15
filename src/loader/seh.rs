use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::types::{ChuModAPI, ChuModInfo, ChuModInitFunc, ChuModShutdownFunc};

use super::log::log_info;

pub unsafe fn call_mod_init(
    name: &str,
    init: ChuModInitFunc,
    info: *const ChuModInfo,
    api: *const ChuModAPI,
) -> Option<i32> {
    match catch_unwind(AssertUnwindSafe(|| init(info, api))) {
        Ok(ret) => Some(ret),
        Err(_) => {
            log_info(&format!("mod init panic caught, skip mod: {}", name));
            None
        }
    }
}

pub unsafe fn call_mod_shutdown(name: &str, shutdown: ChuModShutdownFunc) {
    if catch_unwind(AssertUnwindSafe(|| shutdown())).is_err() {
        log_info(&format!("mod shutdown panic caught: {}", name));
    }
}
