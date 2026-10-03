//! 一小撮 Win32 兜底。
//!
//! UIA 覆盖不到的地方（老式 Win32 编辑框的插入符、工作区、DPI、类名）在这里。

use std::mem::size_of;

use smspop_core::placement::PixelRect;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO,
};

/// UIA 探测失败或拿不到插入符时的全部 Win32 信息。
#[derive(Debug, Clone)]
pub struct FocusContext {
    /// 前台窗口的 DPI（拿不到就 96）。
    pub dpi: u32,
    /// 前台窗口的类名（仅用于日志）。
    pub class_name: Option<String>,
    /// 焦点线程的插入符（物理像素）。
    pub caret: Option<PixelRect>,
}

/// 一次性把 Win32 那边能问的都问出来。
pub fn focus_context() -> FocusContext {
    let foreground = unsafe { GetForegroundWindow() };

    if foreground.0.is_null() {
        return FocusContext {
            dpi: 96,
            class_name: None,
            caret: None,
        };
    }

    let dpi = match unsafe { GetDpiForWindow(foreground) } {
        0 => 96,
        dpi => dpi,
    };

    FocusContext {
        dpi,
        class_name: class_name(foreground),
        caret: caret_from_gui_thread(foreground),
    }
}

/// 前台窗口的类名。
fn class_name(hwnd: HWND) -> Option<String> {
    let mut buffer = [0u16; 256];

    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };

    if length <= 0 {
        return None;
    }

    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

/// 焦点线程的插入符位置。
///
/// 对**老式 Win32 编辑框**（比如某些对话框里的输入框）比 UIA 更准，
/// 所以 UIA 拿不到时用它兜底。
fn caret_from_gui_thread(foreground: HWND) -> Option<PixelRect> {
    let thread_id = unsafe { GetWindowThreadProcessId(foreground, None) };

    if thread_id == 0 {
        return None;
    }

    let mut info = GUITHREADINFO {
        cbSize: size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };

    if unsafe { GetGUIThreadInfo(thread_id, &mut info) }.is_err() {
        return None;
    }

    if info.hwndCaret.0.is_null() {
        return None;
    }

    // rcCaret 是**客户区坐标**，要转成屏幕坐标
    let mut point = POINT {
        x: info.rcCaret.left,
        y: info.rcCaret.bottom,
    };

    if !unsafe { ClientToScreen(info.hwndCaret, &mut point) }.as_bool() {
        return None;
    }

    let height = (info.rcCaret.bottom - info.rcCaret.top).max(1);

    Some(PixelRect::from_caret_point(point.x, point.y, height))
}
