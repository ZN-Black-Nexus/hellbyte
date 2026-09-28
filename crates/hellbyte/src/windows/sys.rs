//! Win32 declarations (imported with raw-dylib: no import libraries or
//! MSVC toolchain needed) and small safe-ish helpers.

#![allow(non_snake_case, clippy::upper_case_acronyms, dead_code)]

pub type HANDLE = isize;
pub type HWND = isize;
pub type BOOL = i32;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MONITORINFO {
    pub cbSize: u32,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: usize,
    pub lParam: isize,
    pub time: u32,
    pub pt: POINT,
    pub private: u32,
}

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
    pub lpfnWndProc: unsafe extern "system" fn(HWND, u32, usize, isize) -> isize,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HANDLE,
    pub hIcon: HANDLE,
    pub hCursor: HANDLE,
    pub hbrBackground: HANDLE,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
    pub hIconSm: HANDLE,
}

#[repr(C)]
pub struct BITMAPINFO256 {
    pub biSize: u32,
    pub biWidth: i32,
    pub biHeight: i32,
    pub biPlanes: u16,
    pub biBitCount: u16,
    pub biCompression: u32,
    pub biSizeImage: u32,
    pub biXPelsPerMeter: i32,
    pub biYPelsPerMeter: i32,
    pub biClrUsed: u32,
    pub biClrImportant: u32,
    pub colors: [u32; 256],
}

#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: HANDLE,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [u8; 32],
}

#[repr(C)]
pub struct RAWINPUTDEVICE {
    pub usUsagePage: u16,
    pub usUsage: u16,
    pub dwFlags: u32,
    pub hwndTarget: HWND,
}

/// RAWINPUTHEADER followed by RAWMOUSE (enough for mouse deltas).
#[repr(C)]
pub struct RAWMOUSEINPUT {
    pub dwType: u32,
    pub dwSize: u32,
    pub hDevice: HANDLE,
    pub wParam: usize,
    pub usFlags: u16,
    pub usButtonFlags: u16,
    pub usButtonData: u16,
    pub ulRawButtons: u32,
    pub lLastX: i32,
    pub lLastY: i32,
    pub ulExtraInformation: u32,
}

#[repr(C)]
pub struct KEY_EVENT_RECORD {
    pub bKeyDown: BOOL,
    pub wRepeatCount: u16,
    pub wVirtualKeyCode: u16,
    pub wVirtualScanCode: u16,
    pub uChar: u16,
    pub dwControlKeyState: u32,
}

#[repr(C)]
pub struct INPUT_RECORD {
    pub EventType: u16,
    pub _pad: u16,
    pub Event: KEY_EVENT_RECORD,
}

#[repr(C)]
#[derive(Default)]
pub struct CONSOLE_SCREEN_BUFFER_INFO {
    pub dwSize: [i16; 2],
    pub dwCursorPosition: [i16; 2],
    pub wAttributes: u16,
    pub srWindow: [i16; 4],
    pub dwMaximumWindowSize: [i16; 2],
}

macro_rules! dll {
    ($lib:literal { $($body:tt)* }) => {
        #[cfg_attr(target_arch = "x86", link(name = $lib, kind = "raw-dylib", import_name_type = "undecorated"))]
        #[cfg_attr(not(target_arch = "x86"), link(name = $lib, kind = "raw-dylib"))]
        unsafe extern "system" { $($body)* }
    };
}

dll!("kernel32" {
    pub fn GetModuleHandleW(name: *const u16) -> HANDLE;
    pub fn ExitProcess(code: u32) -> !;
    pub fn GetCommandLineW() -> *const u16;
    pub fn Sleep(ms: u32);
    pub fn QueryPerformanceCounter(out: *mut i64) -> BOOL;
    pub fn QueryPerformanceFrequency(out: *mut i64) -> BOOL;
    pub fn CreateFileW(name: *const u16, access: u32, share: u32, sec: *const u8, disp: u32, flags: u32, tmpl: HANDLE) -> HANDLE;
    pub fn WriteFile(h: HANDLE, buf: *const u8, n: u32, written: *mut u32, ov: *mut u8) -> BOOL;
    pub fn ReadFile(h: HANDLE, buf: *mut u8, n: u32, read: *mut u32, ov: *mut u8) -> BOOL;
    pub fn CloseHandle(h: HANDLE) -> BOOL;
    pub fn CreateDirectoryW(name: *const u16, sec: *const u8) -> BOOL;
    pub fn GetEnvironmentVariableW(name: *const u16, buf: *mut u16, n: u32) -> u32;
    pub fn GetStdHandle(which: u32) -> HANDLE;
    pub fn AttachConsole(pid: u32) -> BOOL;
    pub fn GetConsoleMode(h: HANDLE, mode: *mut u32) -> BOOL;
    pub fn SetConsoleMode(h: HANDLE, mode: u32) -> BOOL;
    pub fn SetConsoleOutputCP(cp: u32) -> BOOL;
    pub fn GetNumberOfConsoleInputEvents(h: HANDLE, n: *mut u32) -> BOOL;
    pub fn ReadConsoleInputW(h: HANDLE, buf: *mut INPUT_RECORD, n: u32, read: *mut u32) -> BOOL;
    pub fn GetConsoleScreenBufferInfo(h: HANDLE, info: *mut CONSOLE_SCREEN_BUFFER_INFO) -> BOOL;
});

dll!("user32" {
    pub fn RegisterClassExW(c: *const WNDCLASSEXW) -> u16;
    pub fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: HWND, menu: HANDLE, inst: HANDLE, param: *const u8) -> HWND;
    pub fn DefWindowProcW(h: HWND, m: u32, w: usize, l: isize) -> isize;
    pub fn PeekMessageW(msg: *mut MSG, h: HWND, min: u32, max: u32, remove: u32) -> BOOL;
    pub fn TranslateMessage(msg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(msg: *const MSG) -> isize;
    pub fn ShowWindow(h: HWND, cmd: i32) -> BOOL;
    pub fn GetClientRect(h: HWND, r: *mut RECT) -> BOOL;
    pub fn AdjustWindowRect(r: *mut RECT, style: u32, menu: BOOL) -> BOOL;
    pub fn LoadCursorW(inst: HANDLE, name: *const u16) -> HANDLE;
    pub fn SetCursor(c: HANDLE) -> HANDLE;
    pub fn ShowCursor(show: BOOL) -> i32;
    pub fn ClipCursor(r: *const RECT) -> BOOL;
    pub fn GetWindowRect(h: HWND, r: *mut RECT) -> BOOL;
    pub fn GetDC(h: HWND) -> HANDLE;
    pub fn ReleaseDC(h: HWND, dc: HANDLE) -> i32;
    pub fn BeginPaint(h: HWND, ps: *mut PAINTSTRUCT) -> HANDLE;
    pub fn EndPaint(h: HWND, ps: *const PAINTSTRUCT) -> BOOL;
    pub fn RegisterRawInputDevices(d: *const RAWINPUTDEVICE, n: u32, size: u32) -> BOOL;
    pub fn GetRawInputData(h: HANDLE, cmd: u32, data: *mut u8, size: *mut u32, header: u32) -> u32;
    pub fn MessageBoxW(h: HWND, text: *const u16, caption: *const u16, ty: u32) -> i32;
    pub fn SetWindowLongW(h: HWND, index: i32, v: i32) -> i32;
    pub fn SetWindowPos(h: HWND, after: HWND, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> BOOL;
    pub fn GetSystemMetrics(i: i32) -> i32;
    pub fn GetForegroundWindow() -> HWND;
    pub fn MonitorFromWindow(h: HWND, flags: u32) -> HANDLE;
    pub fn GetMonitorInfoW(m: HANDLE, info: *mut MONITORINFO) -> BOOL;
});

dll!("gdi32" {
    pub fn StretchDIBits(dc: HANDLE, x: i32, y: i32, w: i32, h: i32, sx: i32, sy: i32, sw: i32, sh: i32, bits: *const u8, bmi: *const BITMAPINFO256, usage: u32, rop: u32) -> i32;
    pub fn PatBlt(dc: HANDLE, x: i32, y: i32, w: i32, h: i32, rop: u32) -> BOOL;
});

dll!("winmm" {
    pub fn timeBeginPeriod(ms: u32) -> u32;
});

pub const INVALID_HANDLE: HANDLE = -1;

/// Microsecond monotonic clock.
pub fn now_us() -> u64 {
    let (mut c, mut f) = (0i64, 0i64);
    unsafe {
        QueryPerformanceCounter(&mut c);
        QueryPerformanceFrequency(&mut f);
    }
    if f <= 0 {
        return 0;
    }
    ((c as i128 * 1_000_000) / f as i128) as u64
}

pub fn sleep_us(us: u64) {
    unsafe { Sleep((us / 1000) as u32) }
}

/// ASCII/UTF-8 bytes -> NUL-terminated UTF-16 in `out`.
pub fn wide<'a>(s: &[u8], out: &'a mut [u16]) -> &'a [u16] {
    let n = s.len().min(out.len() - 1);
    for i in 0..n {
        out[i] = s[i] as u16;
    }
    out[n] = 0;
    &out[..n + 1]
}

pub fn stdout_write(h: HANDLE, b: &[u8]) -> bool {
    let mut off = 0;
    while off < b.len() {
        let mut w = 0u32;
        let ok = unsafe { WriteFile(h, b[off..].as_ptr(), (b.len() - off).min(1 << 20) as u32, &mut w, core::ptr::null_mut()) };
        if ok == 0 || w == 0 {
            return false;
        }
        off += w as usize;
    }
    true
}
