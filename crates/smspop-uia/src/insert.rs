//! 把验证码真正写进目标控件。
//!
//! ★ 这是**整个项目里唯一会改动别人内容**的地方，
//!   默认由点击候选条触发。显式开启的自动路径仅尝试一次，且不盲写或追加普通文本。
//!
//! 两种机制：
//!
//! * `direct`（默认）—— `IUIAutomationValuePattern::SetValue` 整体写入。
//!   写入前仍检查目标身份、焦点和会话，避免对旧控件写入。
//! * `simulate` —— 对当前焦点目标用 [`crate::typing`] 逐字敲键盘，不主动夺回焦点。
//!   适合那些对 `SetValue` 不生效的输入框；代价是需要焦点、也更慢。
//!
//! 手动使用 [`smspop_core::insertion::plan`]，自动使用更保守的 plan_automatic。

use std::time::Duration;

use log::{info, warn};
use windows::core::BSTR;
use windows::Win32::UI::Accessibility::{
    IUIAutomation, IUIAutomationElement, IUIAutomationValuePattern, UIA_ValuePatternId,
};

use smspop_core::config::InsertMode;
use smspop_core::insertion;

use crate::worker::WriteValidity;
use crate::{typing, InsertOutcome};

/// 往 `element` 里写 `code`。
///
/// `element` 是上一次探测留下的目标；为 `None` 说明从来没探测成功过。
pub(crate) fn run(
    automation: &IUIAutomation,
    element: Option<&IUIAutomationElement>,
    code: &str,
    mode: InsertMode,
    type_delay: Duration,
    automatic: bool,
    validity: &WriteValidity,
) -> InsertOutcome {
    let Some(element) = element else {
        return InsertOutcome::no("目标已失效，请手动粘贴");
    };
    if !validity.focused(automation, element) {
        return InsertOutcome::no("焦点已改变或请求已取消，请手动粘贴");
    }
    if automatic
        && unsafe { element.CurrentIsPassword() }
            .map(|v| v.as_bool())
            .unwrap_or(true)
    {
        return InsertOutcome::no("不向密码框填入验证码");
    }

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

    let plan = if automatic {
        insertion::plan_automatic(current.as_deref(), Some(code))
    } else {
        insertion::plan(current.as_deref(), Some(code))
    };

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

            if !validity.focused(automation, element) {
                return InsertOutcome::no("焦点已改变或请求已取消，请手动粘贴");
            }

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

            typing::type_text(automation, element, &typed, type_delay, validity)
        }
    }
}
