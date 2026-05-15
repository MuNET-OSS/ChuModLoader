use std::ffi::{c_char, c_void};
use std::io::Write;

use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Console::{GetConsoleScreenBufferInfo, SetConsoleTextAttribute, WriteConsoleA, CONSOLE_SCREEN_BUFFER_INFO};

use super::state::{LoaderState, STATE};

extern "system" {
    fn GetLocalTime(st: *mut SYSTEMTIME);
}

#[repr(C)]
struct SYSTEMTIME {
    w_year: u16,
    w_month: u16,
    w_day_of_week: u16,
    w_day: u16,
    w_hour: u16,
    w_minute: u16,
    w_second: u16,
    w_milliseconds: u16,
}

#[derive(Clone, Copy)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }

    fn color(self) -> u16 {
        match self {
            Self::Info => 0x0A,
            Self::Warn => 0x0E,
            Self::Error => 0x0C,
        }
    }
}

pub fn write_log_inner(state: &mut LoaderState, msg: &str) {
    write_log_inner_level(state, LogLevel::Info, msg);
}

pub fn write_log_inner_level(state: &mut LoaderState, level: LogLevel, msg: &str) {
    unsafe {
        let mut st: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut st);

        let formatted = format!(
            "[{:02}:{:02}:{:02}.{:03}] [loader] [{}] {}\n",
            st.w_hour,
            st.w_minute,
            st.w_second,
            st.w_milliseconds,
            level.label(),
            msg
        );

        if let Some(ref mut f) = state.log_file {
            let _ = f.write_all(formatted.as_bytes());
            let _ = f.flush();
        }

        if state.console != INVALID_HANDLE_VALUE && !state.console.is_null() {
            let mut written = 0u32;
            let mut console_info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            let has_console_info = GetConsoleScreenBufferInfo(state.console, &mut console_info) != 0;
            SetConsoleTextAttribute(state.console, level.color());
            WriteConsoleA(
                state.console,
                formatted.as_ptr(),
                formatted.len() as u32,
                &mut written,
                std::ptr::null(),
            );
            if has_console_info {
                SetConsoleTextAttribute(state.console, console_info.wAttributes);
            }
        }
    }
}

pub fn log_info(msg: &str) {
    if let Ok(mut state) = STATE.lock() {
        write_log_inner_level(&mut state, LogLevel::Info, msg);
    }
}

pub fn log_warn(msg: &str) {
    if let Ok(mut state) = STATE.lock() {
        write_log_inner_level(&mut state, LogLevel::Warn, msg);
    }
}

pub fn log_error(msg: &str) {
    if let Ok(mut state) = STATE.lock() {
        write_log_inner_level(&mut state, LogLevel::Error, msg);
    }
}

#[no_mangle]
pub unsafe extern "C" fn write_log_variadic(fmt: *const c_char, args: ...) {
    extern "C" {
        fn vsnprintf(buf: *mut u8, size: usize, fmt: *const c_char, args: *const c_void) -> i32;
    }
    let mut buf = [0u8; 480];
    let len = vsnprintf(
        buf.as_mut_ptr(),
        buf.len(),
        fmt,
        &args as *const _ as *const c_void,
    );
    let len = if len < 0 {
        0
    } else {
        len.min(buf.len() as i32 - 1)
    } as usize;
    let text = String::from_utf8_lossy(&buf[..len]);
    log_info(&text);
}
