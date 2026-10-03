//! 模拟键盘逐字键入（`insertion.mode = "simulate"`）。
//!
//! 和直接 `SetValue` 不同，这条路**需要目标控件真正拿到焦点**：
//! `SendInput` 把按键投进前台线程的输入队列，谁在前台谁就收到。
//!
//! 所以顺序是：聚焦目标 → 确认前台确实是目标 → 光标移到末尾 → 逐字键入。
//! 每敲一个字之前都重新确认一次前台，用户中途点到别处就立刻停手，
//! 绝不让剩下的字符漏进别的窗口。
//!
//! ★ 字符用 `KEYEVENTF_UNICODE` 逐个 UTF-16 单元发送，绕过键盘布局和输入法。
//!   这是数字验证码最可靠的方式（`VkKeyScan` 那套在大写/符号/多语言下会翻车）。

use std::mem::size_of;
use std::thread;
use std::time::Duration;

use log::{debug, info};
use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, IUIAutomationTextPattern, TextPatternRangeEndpoint_End,
    TextPatternRangeEndpoint_Start, UIA_TextPatternId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_END,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::InsertOutcome;

/// 聚焦 `element` 之后，把 `text` 逐字键入进去。
///
/// `delay` 是每个字符之间的间隔；零表示不额外等待。
pub(crate) fn type_text(
    element: &IUIAutomationElement,
    text: &str,
    delay: Duration,
) -> InsertOutcome {
    if text.is_empty() {
        return InsertOutcome::no("没有可键入的内容");
    }

    // 1) 聚焦目标。候选条本身是 NOACTIVATE 的，不会抢焦点，
    //    但这里要主动把焦点放到输入框上（用户点击前焦点可能已经不在了）。
    if unsafe { element.SetFocus() }.is_err() {
        return InsertOutcome::no("目标不接受焦点，无法模拟键入");
    }

    // 给焦点切换和输入法状态一点时间
    thread::sleep(Duration::from_millis(80));

    // 2) 安全闸：前台必须真的是目标进程，否则一个字符都不敲
    let pid = unsafe { element.CurrentProcessId() }.unwrap_or(0) as u32;

    if pid == 0 || !foreground_is(pid) {
        return InsertOutcome::no("目标不在前台，已放弃模拟键入");
    }

    // 3) 光标移到末尾（没有 TextPattern 就退一格 End 键）
    if !move_caret_to_end(element) {
        debug!("目标没有 TextPattern，退回发送 End 键");
        tap_vk(VK_END);
        thread::sleep(Duration::from_millis(20));
    }

    // 4) 逐字键入
    let total = text.encode_utf16().count();
    let mut typed = 0usize;

    for unit in text.encode_utf16() {
        if !foreground_is(pid) {
            return InsertOutcome::no(format!("焦点中途改变，已停止（{typed}/{total}）"));
        }

        if !send_unicode(unit) {
            return InsertOutcome::no(format!("键入被系统拒绝（{typed}/{total}）"));
        }

        typed += 1;

        if !delay.is_zero() {
            thread::sleep(delay);
        }
    }

    info!("已模拟键入 {typed} 个字符");

    InsertOutcome::yes("已逐字填入")
}

/// 当前前台窗口是不是 `pid` 这个进程的。
fn foreground_is(pid: u32) -> bool {
    let hwnd = unsafe { GetForegroundWindow() };

    if hwnd.0.is_null() {
        return false;
    }

    let mut window_pid = 0u32;

    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
    }

    window_pid == pid
}

/// 把光标折叠到文档末尾。
///
/// 能用 UIA 的 TextPattern 就用（不产生按键）；拿不到就走调用方的 End 键兜底。
fn move_caret_to_end(element: &IUIAutomationElement) -> bool {
    let Ok(pattern) =
        (unsafe { element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) })
    else {
        return false;
    };

    let Ok(range) = (unsafe { pattern.DocumentRange() }) else {
        return false;
    };

    // 把 start 端点折到 end 端点 → 得到一个落在末尾的空选区，
    // 再 Select() 就是把光标放过去。
    if unsafe {
        range.MoveEndpointByRange(
            TextPatternRangeEndpoint_Start,
            &range,
            TextPatternRangeEndpoint_End,
        )
    }
    .is_err()
    {
        return false;
    }

    unsafe { range.Select() }.is_ok()
}

/// 按一下某个虚拟键（down + up）。
fn tap_vk(vk: VIRTUAL_KEY) {
    let down = key_input(vk, 0, KEYBD_EVENT_FLAGS(0));
    let up = key_input(vk, 0, KEYEVENTF_KEYUP);

    unsafe {
        SendInput(&[down, up], size_of::<INPUT>() as i32);
    }
}

/// 发送一个 UTF-16 单元的按下 / 抬起。
///
/// 返回 false 表示系统没有把两个事件都插进去（通常是被 UIPI 挡了）。
fn send_unicode(unit: u16) -> bool {
    let down = key_input(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE);
    let up = key_input(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP);

    let sent = unsafe { SendInput(&[down, up], size_of::<INPUT>() as i32) };

    sent == 2
}

/// 造一个键盘 `INPUT`。
fn key_input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
