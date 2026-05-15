use windows_sys::Win32::System::LibraryLoader::{
    GetModuleFileNameA, GetModuleHandleExA, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
};

use super::state::HMODULE;

const MAX_PATH: usize = 260;

#[repr(C)]
struct ImageDosHeader {
    e_magic: u16,
    _pad: [u16; 29],
    e_lfanew: i32,
}

#[repr(C)]
struct ImageFileHeader {
    machine: u16,
    number_of_sections: u16,
    _pad: [u32; 3],
    size_of_optional_header: u16,
    characteristics: u16,
}

#[repr(C)]
struct ImageOptionalHeader32 {
    _pad: [u8; 56],
    size_of_image: u32,
}

#[repr(C)]
struct ImageNtHeaders32 {
    signature: u32,
    file_header: ImageFileHeader,
    optional_header: ImageOptionalHeader32,
}

#[repr(C)]
struct ImageSectionHeader {
    name: [u8; 8],
    virtual_size: u32,
    virtual_address: u32,
    _rest: [u32; 4],
    _characteristics: u32,
}

pub fn get_self_base_dir() -> Option<String> {
    unsafe {
        let mut self_module: HMODULE = std::ptr::null_mut();
        let dummy_addr = get_self_base_dir as *const () as *const u8;
        GetModuleHandleExA(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            dummy_addr,
            &mut self_module,
        );
        if self_module.is_null() {
            return None;
        }

        let mut buf = [0u8; MAX_PATH];
        let len = GetModuleFileNameA(self_module, buf.as_mut_ptr(), buf.len() as u32);
        if len == 0 {
            return None;
        }

        let path = String::from_utf8_lossy(&buf[..len as usize]).into_owned();
        path.rfind('\\')
            .or_else(|| path.rfind('/'))
            .map(|pos| path[..pos].to_string())
    }
}

pub fn parse_game_info(game: HMODULE) -> (u32, usize, u32, usize, u32) {
    unsafe {
        let base = game as usize;
        let dos = &*(base as *const ImageDosHeader);
        let nt = &*((base + dos.e_lfanew as usize) as *const ImageNtHeaders32);
        let game_size = nt.optional_header.size_of_image;

        let first_section = (nt as *const ImageNtHeaders32 as usize)
            + std::mem::size_of::<u32>()
            + std::mem::size_of::<ImageFileHeader>()
            + nt.file_header.size_of_optional_header as usize;

        let mut text_base = 0usize;
        let mut text_size = 0u32;
        let mut rdata_base = 0usize;
        let mut rdata_size = 0u32;

        for i in 0..nt.file_header.number_of_sections as usize {
            let sec = &*((first_section + i * std::mem::size_of::<ImageSectionHeader>())
                as *const ImageSectionHeader);
            if &sec.name[..5] == b".text" {
                text_base = base + sec.virtual_address as usize;
                text_size = sec.virtual_size;
            }
            if &sec.name[..6] == b".rdata" {
                rdata_base = base + sec.virtual_address as usize;
                rdata_size = sec.virtual_size;
            }
        }

        (game_size, text_base, text_size, rdata_base, rdata_size)
    }
}

pub fn read_game_version(base_dir: &str) -> Option<String> {
    extern "system" {
        fn GetFileVersionInfoSizeA(filename: *const u8, handle: *mut u32) -> u32;
        fn GetFileVersionInfoA(filename: *const u8, handle: u32, len: u32, data: *mut std::ffi::c_void) -> i32;
        fn VerQueryValueA(
            block: *const std::ffi::c_void,
            sub_block: *const u8,
            value: *mut *mut std::ffi::c_void,
            len: *mut u32,
        ) -> i32;
    }

    unsafe {
        let path = format!("{}\\chusanApp.exe\0", base_dir);
        let mut handle = 0u32;
        let size = GetFileVersionInfoSizeA(path.as_ptr(), &mut handle);
        if size == 0 {
            return None;
        }

        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoA(path.as_ptr(), 0, size, data.as_mut_ptr() as *mut _) == 0 {
            return None;
        }

        let mut trans_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut trans_len = 0u32;
        if VerQueryValueA(
            data.as_ptr() as *const _,
            b"\\VarFileInfo\\Translation\0".as_ptr(),
            &mut trans_ptr,
            &mut trans_len,
        ) == 0
            || trans_len < 4
        {
            return None;
        }

        let lang = *(trans_ptr as *const u16);
        let codepage = *((trans_ptr as *const u16).add(1));
        for key in ["FileVersion", "ProductVersion"] {
            let query = format!(
                "\\StringFileInfo\\{:04x}{:04x}\\{}\0",
                lang, codepage, key
            );
            let mut value_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut value_len = 0u32;
            if VerQueryValueA(
                data.as_ptr() as *const _,
                query.as_ptr(),
                &mut value_ptr,
                &mut value_len,
            ) != 0
                && !value_ptr.is_null()
                && value_len > 1
            {
                let value = std::ffi::CStr::from_ptr(value_ptr as *const i8)
                    .to_string_lossy()
                    .trim()
                    .to_string();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
        None
    }
}
