use std::ffi::{c_char, c_void, CStr};

use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    FindClose, FindFirstFileA, FindNextFileA, GetFileAttributesA, FILE_ATTRIBUTE_DIRECTORY,
    INVALID_FILE_ATTRIBUTES, WIN32_FIND_DATAA,
};

use super::log::log_info;

extern "system" {
    fn CreateDirectoryA(path: *const u8, security: *const c_void) -> i32;
    fn GetPrivateProfileStringA(
        app: *const u8,
        key: *const u8,
        default: *const u8,
        ret: *mut u8,
        size: u32,
        file: *const u8,
    ) -> u32;
}

pub fn ensure_mods_layout(base_dir: &str) -> (String, String) {
    unsafe {
        let mods_dir = format!("{}\\mods", base_dir);
        let ini_path = format!("{}\\mods.ini", base_dir);

        let mods_dir_c = format!("{}\0", mods_dir);
        if GetFileAttributesA(mods_dir_c.as_ptr()) == INVALID_FILE_ATTRIBUTES {
            CreateDirectoryA(mods_dir_c.as_ptr(), std::ptr::null());
            log_info(&format!("created mods dir: {}", mods_dir));
        }

        (mods_dir, ini_path)
    }
}

pub fn is_mod_enabled(ini_path: &str, mod_name: &str) -> bool {
    unsafe {
        let mut value = [0u8; 32];
        let section = b"mods\0";
        let ini_cstr = format!("{}\0", ini_path);
        let mod_cstr = format!("{}\0", mod_name);
        let empty = b"\0";

        GetPrivateProfileStringA(
            section.as_ptr(),
            mod_cstr.as_ptr(),
            empty.as_ptr(),
            value.as_mut_ptr(),
            value.len() as u32,
            ini_cstr.as_ptr(),
        );

        if value[0] == 0 {
            return true;
        }

        let s = CStr::from_ptr(value.as_ptr() as *const c_char);
        s.to_str()
            .map_or(true, |v| v.parse::<i32>().unwrap_or(1) != 0)
    }
}

pub fn scan_mod_files(mods_dir: &str, ini_path: &str) -> Vec<(String, String)> {
    unsafe {
        let pattern = format!("{}\\*.dll\0", mods_dir);
        let mut find_data: WIN32_FIND_DATAA = std::mem::zeroed();
        let find_handle = FindFirstFileA(pattern.as_ptr(), &mut find_data);
        if find_handle == INVALID_HANDLE_VALUE {
            log_info("no mods found or cannot open directory");
            return Vec::new();
        }

        let mut mods = Vec::new();
        loop {
            if (find_data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) == 0 {
                let mod_name_cstr = CStr::from_ptr(find_data.cFileName.as_ptr() as *const c_char);
                let mod_name = mod_name_cstr.to_string_lossy().into_owned();
                let full_path = format!("{}\\{}", mods_dir, mod_name);

                if is_mod_enabled(ini_path, &mod_name) {
                    mods.push((mod_name, full_path));
                } else {
                    log_info(&format!("mod disabled: {}", full_path));
                }
            }

            if FindNextFileA(find_handle, &mut find_data) == 0 {
                break;
            }
        }
        FindClose(find_handle);
        mods
    }
}

pub fn scan_manifest_files(base_dir: &str) -> Vec<String> {
    unsafe {
        let manifest_dir = format!("{}\\mods\\manifest", base_dir);

        let pattern = format!("{}\\*.toml\0", manifest_dir);
        let mut find_data: WIN32_FIND_DATAA = std::mem::zeroed();
        let find_handle = FindFirstFileA(pattern.as_ptr(), &mut find_data);
        if find_handle == INVALID_HANDLE_VALUE {
            return Vec::new();
        }

        let mut manifests = Vec::new();
        loop {
            if (find_data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) == 0 {
                let name = CStr::from_ptr(find_data.cFileName.as_ptr() as *const c_char)
                    .to_string_lossy()
                    .into_owned();
                manifests.push(format!("{}\\{}", manifest_dir, name));
            }

            if FindNextFileA(find_handle, &mut find_data) == 0 {
                break;
            }
        }
        FindClose(find_handle);
        manifests
    }
}
