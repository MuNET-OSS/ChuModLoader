use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, CreateSolidBrush, SetBkColor, SetTextColor, FW_NORMAL, HBRUSH,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW, MoveWindow,
    PostQuitMessage, RegisterClassExW, SendMessageW, SetWindowTextW, ShowWindow, TranslateMessage,
    CW_USEDEFAULT, MSG, WNDCLASSEXW, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_VISIBLE, WS_VSCROLL,
    WM_COMMAND, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_CTLCOLORBTN, WM_DESTROY, WM_SIZE, SW_SHOW,
};

const ID_TEXT: isize = 100;
const ID_COPY: isize = 101;
const ID_OPEN: isize = 102;
const ID_EXIT: isize = 103;

const ES_MULTILINE: u32 = 0x0004;
const ES_AUTOVSCROLL: u32 = 0x0040;
const ES_READONLY: u32 = 0x0800;
const WM_SETFONT: u32 = 0x0030;
const EM_SETSEL: u32 = 0x00B1;
const WM_COPY: u32 = 0x0301;
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;

#[derive(Clone, Copy)]
struct Theme {
    bg: COLORREF,
    text_bg: COLORREF,
    text_fg: COLORREF,
    dark: bool,
}

const DARK: Theme = Theme {
    bg: 0x002B2B2B,
    text_bg: 0x001E1E1E,
    text_fg: 0x00DCDCDC,
    dark: true,
};

const LIGHT: Theme = Theme {
    bg: 0x00F3F3F3,
    text_bg: 0x00FFFFFF,
    text_fg: 0x00202020,
    dark: false,
};

/// 读系统 AppsUseLightTheme：0=深色 非0=浅色（默认浅色）
fn current_theme() -> &'static Theme {
    if system_uses_dark() {
        &DARK
    } else {
        &LIGHT
    }
}

fn system_uses_dark() -> bool {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_DWORD,
    };
    unsafe {
        let subkey = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        let value = wide("AppsUseLightTheme");
        let mut hkey: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey) != 0 {
            return false;
        }
        let mut data: u32 = 1;
        let mut size = std::mem::size_of::<u32>() as u32;
        let mut value_type = REG_DWORD;
        let ok = RegQueryValueExW(
            hkey,
            value.as_ptr(),
            std::ptr::null_mut(),
            &mut value_type,
            &mut data as *mut u32 as *mut u8,
            &mut size,
        );
        RegCloseKey(hkey);
        ok == 0 && data == 0
    }
}

const MARGIN: i32 = 16;
const BTN_H: i32 = 36;
const BTN_GAP: i32 = 12;

static CRASH_DIR: OnceLock<Vec<u16>> = OnceLock::new();
static THEME: OnceLock<&'static Theme> = OnceLock::new();
static mut TEXT_HWND: HWND = std::ptr::null_mut();
static mut BG_BRUSH: HBRUSH = std::ptr::null_mut();
static mut TEXT_BRUSH: HBRUSH = std::ptr::null_mut();
static mut UI_FONT: windows_sys::Win32::Graphics::Gdi::HFONT = std::ptr::null_mut();

fn theme() -> &'static Theme {
    THEME.get().copied().unwrap_or(&LIGHT)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 弹出崩溃报告窗口（深色主题、等宽字体、可滚动/复制 + Copy/Open Folder/Exit 按钮）
/// 阻塞直到用户关闭，之后进程随崩溃退出
pub fn show(report: &str, crash_dir: &str, zip_path: Option<&str>) {
    let _ = CRASH_DIR.set(wide(crash_dir));

    let mut body = String::new();
    body.push_str("Chusan has crashed Nya...! (>_<)\r\n");
    body.push_str(&"-".repeat(60));
    body.push_str("\r\n\r\n");
    body.push_str(&report.replace('\n', "\r\n"));
    body.push_str("\r\n\r\nA crash report was saved to:\r\n  ");
    body.push_str(crash_dir);
    if let Some(zip) = zip_path {
        body.push_str("\r\n\r\nPlease send this zip if you need help Nya~\r\n  ");
        body.push_str(zip);
    }

    unsafe { run_window(&body) };
}

unsafe fn run_window(body: &str) {
    let class_name = wide("ChusanCrashWindow");
    let hinstance = GetModuleHandleW(std::ptr::null());

    let th = current_theme();
    let _ = THEME.set(th);

    BG_BRUSH = CreateSolidBrush(th.bg);
    TEXT_BRUSH = CreateSolidBrush(th.text_bg);
    let font_name = wide("Cascadia Mono");
    UI_FONT = CreateFontW(18, 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, 1, 0, 0, 4, 0, font_name.as_ptr());
    if UI_FONT.is_null() {
        let fallback = wide("Consolas");
        UI_FONT = CreateFontW(18, 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, 1, 0, 0, 4, 0, fallback.as_ptr());
    }

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: std::ptr::null_mut(),
        hCursor: std::ptr::null_mut(),
        hbrBackground: BG_BRUSH,
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };
    RegisterClassExW(&wc);

    let title = wide("Chusan Crashed Nya~ (>_<)");
    let hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        760,
        600,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        hinstance,
        std::ptr::null(),
    );
    if hwnd.is_null() {
        return;
    }

    enable_dark_titlebar(hwnd, theme().dark);

    let text = create_text_box(hwnd, hinstance, body);
    TEXT_HWND = text;
    create_button(hwnd, hinstance, "Copy Log", ID_COPY);
    create_button(hwnd, hinstance, "Open Folder", ID_OPEN);
    create_button(hwnd, hinstance, "Exit", ID_EXIT);
    layout(hwnd);

    ShowWindow(hwnd, SW_SHOW);

    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

unsafe fn enable_dark_titlebar(hwnd: HWND, dark: bool) {
    let dwmapi = wide("dwmapi.dll");
    let module = windows_sys::Win32::System::LibraryLoader::LoadLibraryW(dwmapi.as_ptr());
    if module.is_null() {
        return;
    }
    type SetAttrFn = unsafe extern "system" fn(HWND, u32, *const core::ffi::c_void, u32) -> i32;
    let proc = windows_sys::Win32::System::LibraryLoader::GetProcAddress(module, b"DwmSetWindowAttribute\0".as_ptr());
    if let Some(proc) = proc {
        let set_attr: SetAttrFn = std::mem::transmute(proc);
        let enabled: i32 = if dark { 1 } else { 0 };
        set_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &enabled as *const i32 as *const _, 4);
    }
}

unsafe fn create_text_box(parent: HWND, hinstance: windows_sys::Win32::Foundation::HMODULE, body: &str) -> HWND {
    let class = wide("EDIT");
    let hwnd = CreateWindowExW(
        0,
        class.as_ptr(),
        std::ptr::null(),
        WS_CHILD | WS_VISIBLE | WS_VSCROLL | ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY,
        0,
        0,
        100,
        100,
        parent,
        ID_TEXT as _,
        hinstance,
        std::ptr::null(),
    );
    let text = wide(body);
    SetWindowTextW(hwnd, text.as_ptr());
    if !UI_FONT.is_null() {
        SendMessageW(hwnd, WM_SETFONT, UI_FONT as WPARAM, 1);
    }
    hwnd
}

unsafe fn create_button(parent: HWND, hinstance: windows_sys::Win32::Foundation::HMODULE, label: &str, id: isize) {
    let class = wide("BUTTON");
    let text = wide(label);
    let hwnd = CreateWindowExW(
        0,
        class.as_ptr(),
        text.as_ptr(),
        WS_CHILD | WS_VISIBLE,
        0,
        0,
        100,
        BTN_H,
        parent,
        id as _,
        hinstance,
        std::ptr::null(),
    );
    if !UI_FONT.is_null() {
        SendMessageW(hwnd, WM_SETFONT, UI_FONT as WPARAM, 1);
    }
}

unsafe fn layout(hwnd: HWND) {
    let mut rect: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rect);
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;

    let text_h = h - BTN_H - MARGIN * 3;
    if !TEXT_HWND.is_null() {
        MoveWindow(TEXT_HWND, MARGIN, MARGIN, w - MARGIN * 2, text_h, 1);
    }

    let btn_w = 130;
    let btn_y = h - MARGIN - BTN_H;
    let mut x = MARGIN;
    for id in [ID_COPY, ID_OPEN, ID_EXIT] {
        if let Some(btn) = find_child(hwnd, id) {
            MoveWindow(btn, x, btn_y, btn_w, BTN_H, 1);
        }
        x += btn_w + BTN_GAP;
    }
}

unsafe fn find_child(parent: HWND, id: isize) -> Option<HWND> {
    let child = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(parent, id as i32);
    (!child.is_null()).then_some(child)
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            layout(hwnd);
            0
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC => {
            let hdc = wparam as windows_sys::Win32::Graphics::Gdi::HDC;
            let th = theme();
            SetTextColor(hdc, th.text_fg);
            SetBkColor(hdc, th.text_bg);
            TEXT_BRUSH as LRESULT
        }
        WM_CTLCOLORBTN => BG_BRUSH as LRESULT,
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as isize;
            match id {
                ID_COPY if !TEXT_HWND.is_null() => {
                    SendMessageW(TEXT_HWND, EM_SETSEL, 0, -1isize as LPARAM);
                    SendMessageW(TEXT_HWND, WM_COPY, 0, 0);
                }
                ID_OPEN => open_crash_folder(),
                ID_EXIT => PostQuitMessage(0),
                _ => {}
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn open_crash_folder() {
    let Some(dir) = CRASH_DIR.get() else {
        return;
    };
    let verb = wide("open");
    windows_sys::Win32::UI::Shell::ShellExecuteW(
        std::ptr::null_mut(),
        verb.as_ptr(),
        dir.as_ptr(),
        std::ptr::null(),
        std::ptr::null(),
        SW_SHOW,
    );
}
