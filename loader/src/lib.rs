#![allow(non_snake_case, clippy::manual_c_str_literals, clippy::upper_case_acronyms)]
#![feature(c_variadic)]

mod api_impl;
mod d3d9;
mod loader;

use std::ffi::c_void;
use std::ptr;

use windows_sys::Win32::Foundation::{BOOL, HMODULE, TRUE};
use windows_sys::Win32::System::Diagnostics::Debug::FlushInstructionCache;
use windows_sys::Win32::System::LibraryLoader::{DisableThreadLibraryCalls, GetModuleHandleA};
use windows_sys::Win32::System::Memory::{
    VirtualAlloc, VirtualProtect, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

const DLL_PROCESS_ATTACH: u32 = 1;
const DLL_PROCESS_DETACH: u32 = 0;
const IMAGE_DOS_SIGNATURE: u16 = 0x5A4D;
const IMAGE_NT_SIGNATURE: u32 = 0x0000_4550;
const JMP_REL32_LEN: usize = 5;
const HIJACK_ORIGINAL_LEN: usize = 16;
const ENTRY_STUB_LEN: usize = 16;
const TRAMPOLINE_EXTRA_LEN: usize = 5;

static mut HIJACK: EntryHijack = EntryHijack::new();

#[repr(C)]
struct ImageDosHeader {
    e_magic: u16,
    e_cblp: u16,
    e_cp: u16,
    e_crlc: u16,
    e_cparhdr: u16,
    e_minalloc: u16,
    e_maxalloc: u16,
    e_ss: u16,
    e_sp: u16,
    e_csum: u16,
    e_ip: u16,
    e_cs: u16,
    e_lfarlc: u16,
    e_ovno: u16,
    e_res: [u16; 4],
    e_oemid: u16,
    e_oeminfo: u16,
    e_res2: [u16; 10],
    e_lfanew: i32,
}

#[repr(C)]
struct ImageFileHeader {
    machine: u16,
    number_of_sections: u16,
    time_date_stamp: u32,
    pointer_to_symbol_table: u32,
    number_of_symbols: u32,
    size_of_optional_header: u16,
    characteristics: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ImageDataDirectory {
    virtual_address: u32,
    size: u32,
}

#[repr(C)]
struct ImageOptionalHeader32 {
    magic: u16,
    major_linker_version: u8,
    minor_linker_version: u8,
    size_of_code: u32,
    size_of_initialized_data: u32,
    size_of_uninitialized_data: u32,
    address_of_entry_point: u32,
    base_of_code: u32,
    base_of_data: u32,
    image_base: u32,
    section_alignment: u32,
    file_alignment: u32,
    major_operating_system_version: u16,
    minor_operating_system_version: u16,
    major_image_version: u16,
    minor_image_version: u16,
    major_subsystem_version: u16,
    minor_subsystem_version: u16,
    win32_version_value: u32,
    size_of_image: u32,
    size_of_headers: u32,
    check_sum: u32,
    subsystem: u16,
    dll_characteristics: u16,
    size_of_stack_reserve: u32,
    size_of_stack_commit: u32,
    size_of_heap_reserve: u32,
    size_of_heap_commit: u32,
    loader_flags: u32,
    number_of_rva_and_sizes: u32,
    data_directory: [ImageDataDirectory; 16],
}

#[repr(C)]
struct ImageNtHeaders32 {
    signature: u32,
    file_header: ImageFileHeader,
    optional_header: ImageOptionalHeader32,
}

struct EntryHijack {
    game_base: usize,
    entry: *mut u8,
    overwritten_len: usize,
    original: [u8; HIJACK_ORIGINAL_LEN],
    trampoline: *mut u8,
    stub: *mut u8,
    installed: bool,
}

impl EntryHijack {
    const fn new() -> Self {
        Self {
            game_base: 0,
            entry: ptr::null_mut(),
            overwritten_len: 0,
            original: [0; 16],
            trampoline: ptr::null_mut(),
            stub: ptr::null_mut(),
            installed: false,
        }
    }
}

#[no_mangle]
unsafe extern "system" fn DllMain(h_module: HMODULE, reason: u32, _reserved: *mut c_void) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => {
            DisableThreadLibraryCalls(h_module);
            install_entry_hijack();
        }
        DLL_PROCESS_DETACH if _reserved.is_null() => {
            loader::unload_mods();
        }
        _ => {}
    }
    TRUE
}

unsafe fn install_entry_hijack() {
    if HIJACK.installed {
        return;
    }

    let game = GetModuleHandleA(ptr::null());
    if game.is_null() {
        diag("hijack: GetModuleHandleA(NULL) returned null");
        return;
    }
    let game_base = game as usize;
    let Some(entry) = image_entry_point(game_base) else {
        diag("hijack: image_entry_point failed");
        return;
    };
    let Some(overwritten_len) = instruction_span(entry, JMP_REL32_LEN) else {
        diag(&format!("hijack: instruction_span failed at {:p}", entry));
        return;
    };
    if overwritten_len > HIJACK_ORIGINAL_LEN {
        diag(&format!("hijack: overwritten_len {} too large", overwritten_len));
        return;
    }

    ptr::copy_nonoverlapping(entry, ptr::addr_of_mut!(HIJACK.original).cast::<u8>(), overwritten_len);
    let Some(trampoline) = build_trampoline(entry, overwritten_len) else {
        return;
    };
    let Some(stub) = build_entry_stub(entry) else {
        return;
    };

    if !write_jmp(entry, stub) {
        return;
    }

    HIJACK.game_base = game_base;
    HIJACK.entry = entry;
    HIJACK.overwritten_len = overwritten_len;
    HIJACK.trampoline = trampoline;
    HIJACK.stub = stub;
    HIJACK.installed = true;
    diag(&format!("hijack: installed, entry={:p}, overwritten={}, stub={:p}, trampoline={:p}", entry, overwritten_len, stub, trampoline));
}

unsafe fn image_entry_point(module_base: usize) -> Option<*mut u8> {
    let dos = (module_base as *const ImageDosHeader).as_ref()?;
    if dos.e_magic != IMAGE_DOS_SIGNATURE || dos.e_lfanew < 0 {
        return None;
    }
    let nt = ((module_base + dos.e_lfanew as usize) as *const ImageNtHeaders32).as_ref()?;
    if nt.signature != IMAGE_NT_SIGNATURE || nt.optional_header.address_of_entry_point == 0 {
        return None;
    }
    Some((module_base + nt.optional_header.address_of_entry_point as usize) as *mut u8)
}

unsafe fn build_trampoline(entry: *mut u8, overwritten_len: usize) -> Option<*mut u8> {
    let size = overwritten_len + TRAMPOLINE_EXTRA_LEN;
    let trampoline = VirtualAlloc(
        ptr::null_mut(),
        size,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_EXECUTE_READWRITE,
    ) as *mut u8;
    if trampoline.is_null() {
        return None;
    }

    ptr::copy_nonoverlapping(entry, trampoline, overwritten_len);
    write_rel32_jmp_unprotected(trampoline.add(overwritten_len), entry.add(overwritten_len));
    flush_icache(trampoline.cast(), size);
    Some(trampoline)
}

unsafe fn build_entry_stub(entry: *mut u8) -> Option<*mut u8> {
    let stub = VirtualAlloc(
        ptr::null_mut(),
        ENTRY_STUB_LEN,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_EXECUTE_READWRITE,
    ) as *mut u8;
    if stub.is_null() {
        return None;
    }

    let mut cursor = stub;
    *cursor = 0x60;
    cursor = cursor.add(1);
    *cursor = 0x9C;
    cursor = cursor.add(1);
    write_rel32_call(cursor, entry_bootstrap as *const () as *const u8);
    cursor = cursor.add(5);
    *cursor = 0x9D;
    cursor = cursor.add(1);
    *cursor = 0x61;
    cursor = cursor.add(1);
    write_rel32_jmp_unprotected(cursor, entry);
    flush_icache(stub.cast(), ENTRY_STUB_LEN);
    Some(stub)
}

unsafe extern "system" fn entry_bootstrap() {
    {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("bootstrap_diag.log") {
            let installed = ptr::addr_of!(HIJACK.installed).read();
            let _ = writeln!(f, "entry_bootstrap called, hijack.installed={}", installed);
        }
    }

    if HIJACK.installed {
        restore_entry_point();
    }

    loader::crash_dump::install();
    loader::load_mods();

    if HIJACK.game_base != 0 {
        d3d9::install_early(HIJACK.game_base);
    }
}

unsafe fn restore_entry_point() {
    let entry = HIJACK.entry;
    let len = HIJACK.overwritten_len;
    if entry.is_null() || len == 0 {
        return;
    }

    let mut old_protect = 0;
    if VirtualProtect(entry.cast(), len, PAGE_EXECUTE_READWRITE, &mut old_protect) == 0 {
        return;
    }
    ptr::copy_nonoverlapping(ptr::addr_of!(HIJACK.original).cast::<u8>(), entry, len);
    let mut ignored = 0;
    let _ = VirtualProtect(entry.cast(), len, old_protect, &mut ignored);
    flush_icache(entry.cast(), len);
}

unsafe fn write_jmp(address: *mut u8, target: *const u8) -> bool {
    let mut old_protect = 0;
    if VirtualProtect(address.cast(), JMP_REL32_LEN, PAGE_EXECUTE_READWRITE, &mut old_protect) == 0 {
        return false;
    }
    write_rel32_jmp_unprotected(address, target);
    let mut ignored = 0;
    let _ = VirtualProtect(address.cast(), JMP_REL32_LEN, old_protect, &mut ignored);
    flush_icache(address.cast(), JMP_REL32_LEN);
    true
}

unsafe fn write_rel32_call(address: *mut u8, target: *const u8) {
    *address = 0xE8;
    write_rel32(address.add(1), address.add(5), target);
}

unsafe fn write_rel32_jmp_unprotected(address: *mut u8, target: *const u8) {
    *address = 0xE9;
    write_rel32(address.add(1), address.add(5), target);
}

unsafe fn write_rel32(slot: *mut u8, next_instruction: *const u8, target: *const u8) {
    let rel = (target as isize).wrapping_sub(next_instruction as isize) as i32;
    ptr::write_unaligned(slot.cast::<i32>(), rel);
}

unsafe fn flush_icache(address: *const c_void, len: usize) {
    let _ = FlushInstructionCache(GetCurrentProcess(), address, len);
}

unsafe fn instruction_span(start: *const u8, min_len: usize) -> Option<usize> {
    let mut total = 0;
    while total < min_len {
        let len = instruction_len(start.add(total))?;
        if len == 0 || total + len > 16 {
            return None;
        }
        total += len;
    }
    Some(total)
}

unsafe fn instruction_len(mut code: *const u8) -> Option<usize> {
    let start = code;
    while let 0x26 | 0x2E | 0x36 | 0x3E | 0x64 | 0x65 | 0x66 | 0x67 | 0xF0 | 0xF2 | 0xF3 = *code {
        code = code.add(1);
    }

    let opcode = *code;
    code = code.add(1);
    let mut has_modrm = false;
    let mut imm_len = 0usize;

    match opcode {
        0x00..=0x03 | 0x08..=0x0B | 0x10..=0x13 | 0x18..=0x1B | 0x20..=0x23 | 0x28..=0x2B
        | 0x30..=0x33 | 0x38..=0x3B | 0x62 | 0x63 | 0x69 | 0x6B | 0x84..=0x8F | 0xC0 | 0xC1
        | 0xC4 | 0xC5 | 0xD0..=0xD3 | 0xF6 | 0xF7 | 0xFE | 0xFF => {
            has_modrm = true;
            imm_len = match opcode {
                0x69 => 4,
                0x6B | 0xC0 | 0xC1 => 1,
                _ => 0,
            };
        }
        0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C | 0x6A | 0xA8 | 0xB0..=0xB7
        | 0xC2 | 0xCA | 0xCD | 0xD4 | 0xD5 | 0xE0..=0xE3 | 0xEB => imm_len = 1,
        0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D | 0x68 | 0xA0..=0xA3 | 0xA9
        | 0xB8..=0xBF | 0xC7 | 0xE8 | 0xE9 => {
            has_modrm = opcode == 0xC7;
            imm_len = 4;
        }
        0x70..=0x7F => imm_len = 1,
        0x90..=0x9F | 0x50..=0x5F | 0x6C..=0x6F | 0xA4..=0xA7 | 0xAA..=0xAF
        | 0xC3 | 0xC9 | 0xCB | 0xCC | 0xCE | 0xCF | 0xF4 | 0xF5 | 0xF8..=0xFD => {}
        0x0F => {
            let second = *code;
            code = code.add(1);
            match second {
                0x80..=0x8F => imm_len = 4,
                0x90..=0x9F | 0xA3..=0xA5 | 0xAB | 0xAD | 0xAF | 0xB0..=0xB7 | 0xBA
                | 0xBB | 0xBC | 0xBD | 0xBE | 0xBF | 0xC0 | 0xC1 => {
                    has_modrm = true;
                    imm_len = if matches!(second, 0xA4 | 0xAC | 0xBA) { 1 } else { 0 };
                }
                _ => return None,
            }
        }
        _ => return Some(code.offset_from(start) as usize),
    }

    if has_modrm {
        code = code.add(modrm_len(code)?);
    }
    Some(code.offset_from(start) as usize + imm_len)
}

unsafe fn modrm_len(code: *const u8) -> Option<usize> {
    let modrm = *code;
    let mode = modrm >> 6;
    let rm = modrm & 7;
    let mut len = 1usize;

    if mode != 3 && rm == 4 {
        let sib = *code.add(len);
        len += 1;
        if mode == 0 && (sib & 7) == 5 {
            len += 4;
        }
    }

    match mode {
        0 if rm == 5 => len += 4,
        1 => len += 1,
        2 => len += 4,
        0 | 3 => {}
        _ => return None,
    }

    Some(len)
}

fn diag(msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("hijack_diag.log") {
        let _ = writeln!(f, "{}", msg);
    }
}
