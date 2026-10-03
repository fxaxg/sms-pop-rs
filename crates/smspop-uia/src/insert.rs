//! 把验证码真正写进目标控件。
//!
//! ★ 这是**整个项目里唯一会改动别人内容**的地方，
//!   而且它只在**用户点击候选条**之后才会被调用。
//!   没有任何自动路径会走到这里 —— 没有定时器，没有后台任务。
//!
//! 两种机制：
//!
//! * `direct`（默认）—— `IUIAutomationValuePattern::SetValue` 整体写入。
//!   它**不要求控件当时有焦点**，所以候选条被点击（哪怕焦点被动了）也照样写得进去。
//! * `simulate` —— 聚焦目标后用 [`crate::typing`] 逐字敲键盘。
//!   适合那些对 `SetValue` 不生效的输入框；代价是需要焦点、也更慢。
//!
//! 两条路都先经过 [`smspop_core::insertion::plan`] 决定"写什么"。

use std::time::Duration;

use log::{info, warn};
use windows::core::BSTR;
use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, IUIAutomationValuePattern, UIA_ValuePatternId,
};

use smspop_core::config::InsertMode;
use smspop_core::insertion;

use crate::{typing, InsertOutcome};

/// 往 `element` 里写 `code`。
///
/// `element` 是上一次探测留下的目标；为 `None` 说明从来没探测成功过。
pub(crate) fn run(
    element: Option<&IUIAutomationElement>,
    code: &str,
    mode: InsertMode,
    type_delay: Duration,
) -> InsertOutcome {
    let Some(element) = element else {
        return InsertOutcome::no("目标已失效，请手动粘贴");
    };

    if code.is_empty() {
        return InsertOutcome::no("没有可用的验证码");
    }

    // 能拿到 ValuePattern 就能读当前值、也能直接写；
    // simulate 模式下拿不到也可以继续（读不到当前值就当作"盲追加"）。
    let value_pattern = match unsafe {
        element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
    } {
        Ok(pattern) => {
            let read_only = unsafe { pattern.CurrentIsReadOnly() }
                .map(|flag| flag.as_bool())
                .unwrap_or(true);

            if read_only {
                return InsertOutcome::no("目标是只读的");
            }

            Some(pattern)
        }
        Err(_) => None,
    };

    if mode == InsertMode::Direct && value_pattern.is_none() {
        return InsertOutcome::no("目标不支持写入，请手动粘贴");
    }

    // ★ 用户点击的**这一刻**才重新读一次当前值 ——
    //   从"探测到光标"到"用户点击"之间，用户可能已经打了字。
    //   写入规则靠的就是这个最新值。拿不到就当作空。
    let current = value_pattern
        .as_ref()
        .and_then(|pattern| unsafe { pattern.CurrentValue() }.ok())
        .map(|value| value.to_string());

    let plan = insertion::plan(current.as_deref(), Some(code));

    if !plan.should_write() {
        // 日志里只有原因，没有内容
        info!("按规则不写入：{}", plan.reason);

        return InsertOutcome::no(plan.reason);
    }

    match mode {
        InsertMode::Direct => {
            let Some(pattern) = value_pattern else {
                return InsertOutcome::no("目标不支持写入，请手动粘贴");
            };

            let value = plan.value.unwrap_or_default();

            match unsafe { pattern.SetValue(&BSTR::from(value.as_str())) } {
                Ok(()) => {
                    info!("已把验证码直接写入目标控件（{}）", plan.reason);

                    InsertOutcome::yes("已填入")
                }
                Err(error) => {
                    warn!("写入验证码失败：{error}");

                    InsertOutcome::no("写入失败，请手动粘贴")
                }
            }
        }

        InsertMode::Simulate => {
            // 补全时只敲缺的那几位；追加时是整段验证码
            let typed = plan.typed.unwrap_or_else(|| code.to_string());

            typing::type_text(element, &typed, type_delay)
        }
    }
}
