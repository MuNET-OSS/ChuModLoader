use std::ffi::c_void;
use std::fs::{self, File};
use std::io::Write;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{BOOL, HANDLE, MAX_PATH};
use windows_sys::Win32::Storage::FileSystem::CREATE_ALWAYS;
use windows_sys::Win32::System::Diagnostics::Debug::{
    AddrModeFlat, MiniDumpNormal, MiniDumpWriteDump, StackWalk, SymCleanup, SymFromAddr,
    SymFunctionTableAccess, SymGetModuleBase, SymInitialize, EXCEPTION_POINTERS,
    MINIDUMP_EXCEPTION_INFORMATION, STACKFRAME, SYMBOL_INFO,
};
use windows_sys::Win32::System::ProcessStatus::{GetModuleBaseNameA, GetModuleInformation, MODULEINFO};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetCurrentProcessId, GetCurrentThread, GetCurrentThreadId};

use super::log::{log_error, log_info};
use super::pe::get_self_base_dir;

const EXCEPTION_EXECUTE_HANDLER: i32 = 1;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const GENERIC_WRITE: u32 = 0x40000000;
const IMAGE_FILE_MACHINE_I386: u32 = 0x014c;

extern "system" {
    fn CreateDirectoryA(path: *const u8, security: *const c_void) -> i32;
    fn CreateFileA(
        name: *const u8,
        access: u32,
        share: u32,
        security: *const c_void,
        disposition: u32,
        flags: u32,
        template: *mut c_void,
    ) -> HANDLE;
    fn CloseHandle(handle: HANDLE) -> BOOL;
    fn GetLocalTime(st: *mut SYSTEMTIME);
    fn SetUnhandledExceptionFilter(
        filter: Option<unsafe extern "system" fn(*mut EXCEPTION_POINTERS) -> i32>,
    ) -> Option<unsafe extern "system" fn(*mut EXCEPTION_POINTERS) -> i32>;
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

#[cfg(target_arch = "x86")]
type NativeContext = windows_sys::Win32::System::Diagnostics::Debug::CONTEXT;

pub unsafe fn install() {
    SetUnhandledExceptionFilter(Some(unhandled_exception_filter));
    log_info("crash dump handler installed");
}

unsafe extern "system" fn unhandled_exception_filter(exception: *mut EXCEPTION_POINTERS) -> i32 {
    if let Err(err) = write_crash_report(exception) {
        log_error(&format!("failed to write crash dump: {}", err));
    }
    EXCEPTION_EXECUTE_HANDLER
}

unsafe fn write_crash_report(exception: *mut EXCEPTION_POINTERS) -> Result<(), String> {
    let base_dir = get_self_base_dir().ok_or_else(|| "cannot resolve base dir".to_string())?;
    let crash_dir = format!("{}\\mods\\crash", base_dir);
    CreateDirectoryA(format!("{}\\mods\0", base_dir).as_ptr(), null());
    CreateDirectoryA(format!("{}\0", crash_dir).as_ptr(), null());

    let stamp = timestamp();
    let dump_path = format!("{}\\crash_{}.dmp", crash_dir, stamp);
    let log_path = format!("{}\\crash_{}.log", crash_dir, stamp);

    write_minidump(&dump_path, exception)?;
    write_text_log(&log_path, exception, &dump_path)?;
    log_error(&format!("crash dump written: {}", dump_path));
    Ok(())
}

unsafe fn write_minidump(path: &str, exception: *mut EXCEPTION_POINTERS) -> Result<(), String> {
    let path_c = format!("{}\0", path);
    let file = CreateFileA(
        path_c.as_ptr(),
        GENERIC_WRITE,
        0,
        null(),
        CREATE_ALWAYS,
        FILE_ATTRIBUTE_NORMAL,
        null_mut(),
    );
    if file == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        return Err(format!("CreateFileA failed for {}", path));
    }

    let exception_info = MINIDUMP_EXCEPTION_INFORMATION {
        ThreadId: GetCurrentThreadId(),
        ExceptionPointers: exception,
        ClientPointers: 0,
    };
    let ok = MiniDumpWriteDump(
        GetCurrentProcess(),
        GetCurrentProcessId(),
        file,
        MiniDumpNormal,
        &exception_info,
        null_mut(),
        null_mut(),
    );
    CloseHandle(file);
    if ok == 0 {
        return Err("MiniDumpWriteDump failed".to_string());
    }
    Ok(())
}

unsafe fn write_text_log(path: &str, exception: *mut EXCEPTION_POINTERS, dump_path: &str) -> Result<(), String> {
    let mut file = File::create(path).map_err(|err| err.to_string())?;
    writeln!(file, "ChuModLoader crash report").map_err(|err| err.to_string())?;
    writeln!(file, "dump: {}", dump_path).map_err(|err| err.to_string())?;

    if exception.is_null() || (*exception).ExceptionRecord.is_null() {
        writeln!(file, "exception: <null>").map_err(|err| err.to_string())?;
        return Ok(());
    }

    let record = &*(*exception).ExceptionRecord;
    writeln!(file, "exception_code: 0x{:08X}", record.ExceptionCode).map_err(|err| err.to_string())?;
    writeln!(file, "exception_address: 0x{:08X}", record.ExceptionAddress as usize).map_err(|err| err.to_string())?;
    if let Some(module) = module_offset(record.ExceptionAddress as usize) {
        writeln!(file, "exception_module: {}+0x{:X}", module.0, module.1).map_err(|err| err.to_string())?;
    }

    #[cfg(target_arch = "x86")]
    if !(*exception).ContextRecord.is_null() {
        write_registers(&mut file, &*((*exception).ContextRecord as *const NativeContext))?;
        write_stack_trace(&mut file, &mut *((*exception).ContextRecord as *mut NativeContext))?;
    }

    #[cfg(not(target_arch = "x86"))]
    writeln!(file, "registers/stack: unsupported target arch").map_err(|err| err.to_string())?;

    Ok(())
}

#[cfg(target_arch = "x86")]
fn write_registers(file: &mut File, ctx: &NativeContext) -> Result<(), String> {
    writeln!(file, "registers:").map_err(|err| err.to_string())?;
    writeln!(file, "  EAX=0x{:08X} EBX=0x{:08X} ECX=0x{:08X} EDX=0x{:08X}", ctx.Eax, ctx.Ebx, ctx.Ecx, ctx.Edx).map_err(|err| err.to_string())?;
    writeln!(file, "  ESI=0x{:08X} EDI=0x{:08X} EBP=0x{:08X} ESP=0x{:08X}", ctx.Esi, ctx.Edi, ctx.Ebp, ctx.Esp).map_err(|err| err.to_string())?;
    writeln!(file, "  EIP=0x{:08X} EFLAGS=0x{:08X}", ctx.Eip, ctx.EFlags).map_err(|err| err.to_string())?;
    Ok(())
}

#[cfg(target_arch = "x86")]
unsafe fn write_stack_trace(file: &mut File, ctx: &mut NativeContext) -> Result<(), String> {
    writeln!(file, "stack_trace:").map_err(|err| err.to_string())?;
    let process = GetCurrentProcess();
    let thread = GetCurrentThread();
    SymInitialize(process, null(), 1);

    let mut frame: STACKFRAME = zeroed();
    frame.AddrPC.Offset = ctx.Eip;
    frame.AddrPC.Mode = AddrModeFlat;
    frame.AddrFrame.Offset = ctx.Ebp;
    frame.AddrFrame.Mode = AddrModeFlat;
    frame.AddrStack.Offset = ctx.Esp;
    frame.AddrStack.Mode = AddrModeFlat;

    for index in 0..64 {
        let ok = StackWalk(
            IMAGE_FILE_MACHINE_I386,
            process,
            thread,
            &mut frame,
            ctx as *mut _ as *mut c_void,
            None,
            Some(SymFunctionTableAccess),
            Some(SymGetModuleBase),
            None,
        );
        if ok == 0 || frame.AddrPC.Offset == 0 {
            break;
        }
        let addr = frame.AddrPC.Offset;
        let symbol = symbol_from_addr(process, addr as u64).unwrap_or_else(|| module_offset(addr as usize).map(|(m, o)| format!("{}+0x{:X}", m, o)).unwrap_or_else(|| "<unknown>".to_string()));
        writeln!(file, "  #{:02} 0x{:08X} {}", index, addr as u32, symbol).map_err(|err| err.to_string())?;
    }

    SymCleanup(process);
    Ok(())
}

#[cfg(target_arch = "x86")]
unsafe fn symbol_from_addr(process: HANDLE, addr: u64) -> Option<String> {
    let mut storage = [0u8; size_of::<SYMBOL_INFO>() + 512];
    let symbol = storage.as_mut_ptr() as *mut SYMBOL_INFO;
    (*symbol).SizeOfStruct = size_of::<SYMBOL_INFO>() as u32;
    (*symbol).MaxNameLen = 511;
    let mut displacement = 0u64;
    if SymFromAddr(process, addr, &mut displacement, symbol) == 0 {
        return None;
    }
    let name_ptr = (*symbol).Name.as_ptr() as *const u8;
    let len = (0..511).position(|i| *name_ptr.add(i) == 0).unwrap_or(511);
    let name = String::from_utf8_lossy(std::slice::from_raw_parts(name_ptr, len)).into_owned();
    Some(format!("{}+0x{:X}", name, displacement))
}

unsafe fn module_offset(addr: usize) -> Option<(String, usize)> {
    let module = module_from_address(addr)?;
    let mut name = [0u8; MAX_PATH as usize];
    let len = GetModuleBaseNameA(
        GetCurrentProcess(),
        module,
        name.as_mut_ptr(),
        name.len() as u32,
    );
    let module_name = if len == 0 {
        "<module>".to_string()
    } else {
        String::from_utf8_lossy(&name[..len as usize]).into_owned()
    };
    Some((module_name, addr.saturating_sub(module as usize)))
}

unsafe fn module_from_address(addr: usize) -> Option<*mut c_void> {
    let mut module = null_mut();
    let flags = 0x00000004u32 | 0x00000002u32;
    let ok = windows_sys::Win32::System::LibraryLoader::GetModuleHandleExA(
        flags,
        addr as *const u8,
        &mut module,
    );
    if ok == 0 || module.is_null() {
        return None;
    }
    let mut info: MODULEINFO = zeroed();
    if GetModuleInformation(GetCurrentProcess(), module, &mut info, size_of::<MODULEINFO>() as u32) == 0 {
        return None;
    }
    Some(module)
}

unsafe fn timestamp() -> String {
    let mut st: SYSTEMTIME = zeroed();
    GetLocalTime(&mut st);
    format!(
        "{:04}{:02}{:02}_{:02}{:02}{:02}_{:03}",
        st.w_year, st.w_month, st.w_day, st.w_hour, st.w_minute, st.w_second, st.w_milliseconds
    )
}

pub fn log_panic_context(scope: &str, name: &str) {
    let base_dir = get_self_base_dir().unwrap_or_default();
    if base_dir.is_empty() {
        log_error(&format!("panic caught in {}: {}", scope, name));
        return;
    }
    let crash_dir = format!("{}\\mods\\crash", base_dir);
    let _ = fs::create_dir_all(&crash_dir);
    let path = format!("{}\\panic_{}.log", crash_dir, unsafe { timestamp() });
    if let Ok(mut file) = File::create(&path) {
        let _ = writeln!(file, "ChuModLoader panic context");
        let _ = writeln!(file, "scope: {}", scope);
        let _ = writeln!(file, "mod: {}", name);
    }
    log_error(&format!("panic caught in {}: {} (context={})", scope, name, path));
}
