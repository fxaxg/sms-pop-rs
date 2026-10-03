//! UIA 探测：焦点是不是一个可写的文本框，以及光标在哪。

use std::ffi::c_void;

use log::debug;
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Ole::{
    SafeArrayDestroy, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound,
};
use windows::Win32::UI::Accessibility::{
    IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern, IUIAutomationTextRange,
    IUIAutomationValuePattern, TextUnit_Character, UIA_TextPatternId, UIA_ValuePatternId,
};

use smspop_core::config::InsertMode;
use smspop_core::placement::PixelRect;

use crate::interop;
use crate::{FocusTarget, ProbeOutcome};

/// 探测结果 + 元素本身。
///
/// 元素要留在工作线程上（点击时写的就是它），所以它不跟着 [`ProbeOutcome`] 回传。
pub(crate) struct ProbeResult {
    pub outcome: ProbeOutcome,
    pub element: Option<IUIAutomationElement>,
}

pub(crate) fn run(automation: &IUIAutomation, mode: InsertMode) -> ProbeResult {
    let element = match unsafe { automation.GetFocusedElement() } {
        Ok(element) => element,
        Err(error) => {
            log::warn!("取焦点元素失败：{error}");

            return ProbeResult {
                outcome: ProbeOutcome::Failed(format!("取焦点元素失败：{error}")),
                element: None,
            };
        }
    };

    // 可写性按**能力**判断，不按控件类型 ——
    // 记事本的控件类型是 Document 而不是 Edit，但它确实支持 ValuePattern。
    // 按类型判断的话记事本这种就漏了。
    let mut supports_value = false;
    let mut is_read_only = true;
    let mut value_len = 0usize;

    match unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) } {
        Ok(pattern) => {
            supports_value = true;

            is_read_only = unsafe { pattern.CurrentIsReadOnly() }
                .map(|flag| flag.as_bool())
                .unwrap_or(true);

            // ★ 只取长度。内容绝不带出这个函数，也绝不进日志 ——
            //   那个框里可能是密码。
            value_len = unsafe { pattern.CurrentValue() }
                .map(|value| value.to_string().chars().count())
                .unwrap_or(0);
        }
        Err(error) => debug!("这个控件没有 ValuePattern：{error}"),
    }

    // simulate 模式还要知道：有没有 TextPattern、能不能拿键盘焦点。
    // direct 模式用不上，但查询本身很便宜。
    let supports_text =
        unsafe { element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }
            .is_ok();

    let keyboard_focusable = unsafe { element.CurrentIsKeyboardFocusable() }
        .map(|flag| flag.as_bool())
        .unwrap_or(false);

    // 判定"能不能填"：
    //
    // * direct  —— 必须有可写的 ValuePattern（这样才能整体写入）
    // * simulate —— 可写的 ValuePattern **或者** 像个可键入的文本控件都行；
    //   后者读不到当前值，追加判断会退化成"盲追加"，这是放宽门槛的代价。
    let editable = match mode {
        InsertMode::Direct => supports_value && !is_read_only,
        InsertMode::Simulate => {
            (supports_value && !is_read_only) || (supports_text && keyboard_focusable)
        }
    };

    let skip_reason = if editable {
        None
    } else {
        match mode {
            InsertMode::Direct if !supports_value => {
                Some("这个控件不支持程序化写入（没有 ValuePattern）".to_string())
            }
            InsertMode::Direct => Some("控件是只读的".to_string()),
            InsertMode::Simulate if !supports_text => {
                Some("这个控件不支持写入，也没有文本模式".to_string())
            }
            InsertMode::Simulate => Some("这个控件不能获得键盘焦点".to_string()),
        }
    };

    let context = interop::focus_context();

    let (caret, caret_source) = match caret_from_text_pattern(&element) {
        Some(rect) => (Some(rect), Some("uia")),
        None => match context.caret {
            Some(rect) => (Some(rect), Some("win32")),
            None => (None, None),
        },
    };

    ProbeResult {
        outcome: ProbeOutcome::Found(FocusTarget {
            editable,
            skip_reason,
            value_len,
            is_read_only,
            caret,
            caret_source,
            dpi: context.dpi,
            class_name: context.class_name,
        }),
        element: Some(element),
    }
}

/// 用 UIA 的 TextPattern 拿光标位置。
///
/// ★ 关键一条：**零长度选区**（也就是普通光标）时 `GetBoundingRectangles`
///   会返回**空数组**，所以得先把选区扩成一个字符再量。
///   而"普通光标"恰恰是最常见的情况，这条路才是主路。
fn caret_from_text_pattern(element: &IUIAutomationElement) -> Option<PixelRect> {
    let pattern =
        match unsafe { element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }
        {
            Ok(pattern) => pattern,
            Err(error) => {
                debug!("这个控件没有 TextPattern：{error}");
                return None;
            }
        };

    let selection = match unsafe { pattern.GetSelection() } {
        Ok(selection) => selection,
        Err(error) => {
            debug!("取选区失败：{error}");
            return None;
        }
    };

    let count = unsafe { selection.Length() }.unwrap_or(0);

    if count <= 0 {
        return None;
    }

    let range = match unsafe { selection.GetElement(0) } {
        Ok(range) => range,
        Err(error) => {
            debug!("取第一个选区失败：{error}");
            return None;
        }
    };

    // 有实际选区 → 直接用它的矩形
    if let Some(first) = first_rect(&range) {
        return Some(first);
    }

    // 零长度选区：先往前扩一个字符，不行再往后扩一个
    for forward in [true, false] {
        let Ok(clone) = (unsafe { range.Clone() }) else {
            continue;
        };

        if !forward {
            let _ = unsafe { clone.Move(TextUnit_Character, -1) };
        }

        if unsafe { clone.ExpandToEnclosingUnit(TextUnit_Character) }.is_err() {
            continue;
        }

        if let Some(first) = first_rect(&clone) {
            return Some(first);
        }
    }

    // 注：C# 版这里还有一层"退回整个控件的矩形"兜底。
    //    那个位置会明显偏（基本落在控件左上角），与其把候选条弹到错的地方，
    //    不如按纪律 3 直接不给 —— 用户还有剪贴板可以粘。
    None
}

/// 取一组矩形里的第一个。
fn first_rect(range: &IUIAutomationTextRange) -> Option<PixelRect> {
    bounding_rects(range)?.into_iter().next()
}

/// 读 `GetBoundingRectangles` 的返回值。
///
/// 返回的是一串 double，每 4 个一组：`left, top, width, height`。
///
/// ★ 这个 `SAFEARRAY` 是**调用方负责释放**的 —— 读完必须 `SafeArrayDestroy`，
///   否则每次探测都漏一块。
fn bounding_rects(range: &IUIAutomationTextRange) -> Option<Vec<PixelRect>> {
    let array = unsafe { range.GetBoundingRectangles() }.ok()?;

    if array.is_null() {
        return None;
    }

    let values = unsafe { read_doubles(array) };

    unsafe {
        let _ = SafeArrayDestroy(array);
    }

    let values = values?;

    let (chunks, _) = values.as_chunks::<4>();

    let rects: Vec<PixelRect> = chunks
        .iter()
        .map(|&[left, top, width, height]| {
            PixelRect::new(
                left.round() as i32,
                top.round() as i32,
                (left + width).round() as i32,
                (top + height).round() as i32,
            )
        })
        .collect();

    if rects.is_empty() {
        None
    } else {
        Some(rects)
    }
}

/// 把 `SAFEARRAY` 里的 double 全抄出来。
unsafe fn read_doubles(array: *mut SAFEARRAY) -> Option<Vec<f64>> {
    let lower = SafeArrayGetLBound(array, 1).ok()?;
    let upper = SafeArrayGetUBound(array, 1).ok()?;

    let count = (upper - lower + 1).max(0) as usize;
    let mut values = Vec::with_capacity(count);

    for index in 0..count {
        let mut value = 0.0f64;
        let position = lower + index as i32;

        SafeArrayGetElement(array, &position, &mut value as *mut f64 as *mut c_void).ok()?;

        values.push(value);
    }

    Some(values)
}
