//! 决定"要不要把这个验证码写进当前输入框，写什么"。
//!
//! ★ 这一步是整个预填功能里**最危险**的地方：写错就是动了用户已经输好的内容。
//!   所以规则刻意保守 —— 拿不准就不动，用户至少还有剪贴板可以粘。
//!
//! 写入策略（从保守到主动）：
//!
//! 1. 字段是空的 / 只有空白 → 直接写入
//! 2. 字段里已经就是这个验证码 → 不动
//! 3. 字段里已经包含这个验证码（开头或结尾）→ 不动，别重复追加
//! 4. 用户手打了前几位 → 补全成完整的
//! 5. 其余情况 → **追加到已有内容后面**（早期版本是"有内容就不动"）
//!
//! 计划里同时给出两种机制要用的内容：
//!
//! * `value` —— 用 `ValuePattern.SetValue` 时整体写入的完整值
//! * `typed` —— 用模拟键盘逐字键入时**只需要敲的字符**
//!   （补全时只敲缺的那几位；追加时就是整段验证码）
//!
//! 纯函数，可单测。项目里没有任何自动路径会调用它 ——
//! 只有用户真的点了候选条，才会走到这里。

/// 写入决策。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertionDecision {
    /// 可以写。
    Write,
    /// 不要碰这个字段。
    DoNotTouch,
}

/// 一次写入的完整计划。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertionPlan {
    pub decision: InsertionDecision,
    /// 直接写入（`SetValue`）时要写的完整值（`DoNotTouch` 时为 `None`）。
    pub value: Option<String>,
    /// 模拟键盘逐字键入时要敲的字符（`DoNotTouch` 时为 `None`）。
    ///
    /// 补全时只有缺的那几位；追加时是整段验证码。
    pub typed: Option<String>,
    /// 人话原因，会显示在候选条上，也会进日志。
    pub reason: String,
}

impl InsertionPlan {
    pub fn should_write(&self) -> bool {
        self.decision == InsertionDecision::Write
    }

    fn do_not_touch(reason: impl Into<String>) -> Self {
        Self {
            decision: InsertionDecision::DoNotTouch,
            value: None,
            typed: None,
            reason: reason.into(),
        }
    }

    fn write(
        value: impl Into<String>,
        typed: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            decision: InsertionDecision::Write,
            value: Some(value.into()),
            typed: Some(typed.into()),
            reason: reason.into(),
        }
    }
}

/// 根据字段**当前**内容和验证码，决定怎么写。
///
/// 调用方应该在用户点击的那一刻重新读一次当前值再调它 ——
/// 从"探测到光标"到"用户点击"之间，用户可能已经打了字。
pub fn plan(current_value: Option<&str>, code: Option<&str>) -> InsertionPlan {
    let Some(code) = code.filter(|code| !code.is_empty()) else {
        return InsertionPlan::do_not_touch("没有可用的验证码");
    };

    let Some(current_value) = current_value else {
        return InsertionPlan::write(code, code, "字段是空的，直接写入");
    };

    if current_value.is_empty() {
        return InsertionPlan::write(code, code, "字段是空的，直接写入");
    }

    let trimmed = current_value.trim();

    if trimmed.is_empty() {
        return InsertionPlan::write(code, code, "字段里只有空白");
    }

    if trimmed == code {
        return InsertionPlan::do_not_touch("验证码已经在字段里了");
    }

    // 字段里已经含有一段验证码（在开头或结尾）→ 别重复追加
    if trimmed.starts_with(code) || trimmed.ends_with(code) {
        return InsertionPlan::do_not_touch("字段里已经有验证码了");
    }

    // 用户手打了前几位（这是验证码输入框里最常见的情况）→ 补全成完整的
    if let Some(tail) = code.strip_prefix(trimmed) {
        return InsertionPlan::write(code, tail, "用户已输入前几位，补全");
    }

    // 其余情况 → 追加到已有内容后面（保留用户原有的内容和格式）
    InsertionPlan::write(format!("{current_value}{code}"), code, "追加到已有内容后面")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reason_of(current: Option<&str>, code: Option<&str>) -> String {
        plan(current, code).reason
    }

    #[test]
    fn 空字段直接写入() {
        let plan = plan(Some(""), Some("868740"));

        assert!(plan.should_write());
        assert_eq!(plan.value.as_deref(), Some("868740"));
        assert_eq!(plan.typed.as_deref(), Some("868740"));
        assert_eq!(plan.reason, "字段是空的，直接写入");
    }

    #[test]
    fn 字段为_none_也当作空() {
        let plan = plan(None, Some("868740"));

        assert!(plan.should_write());
        assert_eq!(plan.value.as_deref(), Some("868740"));
        assert_eq!(plan.typed.as_deref(), Some("868740"));
    }

    #[test]
    fn 只有空白也写入() {
        let plan = plan(Some("   \n  "), Some("868740"));

        assert!(plan.should_write());
        assert_eq!(plan.reason, "字段里只有空白");
    }

    #[test]
    fn 用户打了前几位就补全() {
        for partial in ["8", "868", "8687"] {
            let plan = plan(Some(partial), Some("868740"));

            assert!(plan.should_write(), "前几位 {partial} 应该补全");
            assert_eq!(plan.value.as_deref(), Some("868740"));
            // 模拟键入时只敲缺的那几位，避免把用户已打的重复一遍
            assert_eq!(plan.typed.as_deref(), Some(&"868740"[partial.len()..]));
            assert_eq!(plan.reason, "用户已输入前几位，补全");
        }
    }

    #[test]
    fn 前几位带空格也认() {
        let plan = plan(Some("  868  "), Some("868740"));

        assert!(plan.should_write());
        assert_eq!(plan.value.as_deref(), Some("868740"));
        assert_eq!(plan.typed.as_deref(), Some("740"));
    }

    #[test]
    fn 验证码已经在字段里了就不动() {
        let plan = plan(Some("868740"), Some("868740"));

        assert!(!plan.should_write());
        assert_eq!(plan.reason, "验证码已经在字段里了");
    }

    #[test]
    fn 已有其他内容时追加而不是覆盖() {
        // 早期版本这里是"不覆盖"，现在改成追加到后面
        let plan = plan(Some("hello world"), Some("868740"));

        assert!(plan.should_write());
        assert_eq!(plan.value.as_deref(), Some("hello world868740"));
        assert_eq!(plan.typed.as_deref(), Some("868740"));
        assert_eq!(plan.reason, "追加到已有内容后面");
    }

    #[test]
    fn 追加保留用户原有的空白和格式() {
        let plan = plan(Some("验证码："), Some("868740"));

        assert!(plan.should_write());
        assert_eq!(plan.value.as_deref(), Some("验证码：868740"));
    }

    #[test]
    fn 上一个验证码会被追加() {
        // 旧语义下这是"不覆盖"；新语义是追加（用户明确要追加）
        let plan = plan(Some("111111"), Some("868740"));

        assert!(plan.should_write());
        assert_eq!(plan.value.as_deref(), Some("111111868740"));
    }

    #[test]
    fn 没有验证码时不动() {
        assert!(!plan(Some(""), None).should_write());
        assert_eq!(reason_of(Some(""), None), "没有可用的验证码");
        assert_eq!(reason_of(Some(""), Some("")), "没有可用的验证码");
    }

    #[test]
    fn 字段更长但以验证码开头时不动() {
        // "868740 已过期" —— 字段里已经有码了，别又追加一遍
        let plan = plan(Some("868740 已过期"), Some("868740"));

        assert!(!plan.should_write());
        assert_eq!(plan.reason, "字段里已经有验证码了");
    }

    #[test]
    fn 字段以验证码结尾时也不重复追加() {
        let plan = plan(Some("收到 868740"), Some("868740"));

        assert!(!plan.should_write());
        assert_eq!(plan.reason, "字段里已经有验证码了");
    }

    #[test]
    fn 只有空白和验证码组合的情况() {
        // 字段是 " 868740 " → trim 后等于验证码，不动
        let plan = plan(Some(" 868740 "), Some("868740"));

        assert!(!plan.should_write());
        assert_eq!(plan.reason, "验证码已经在字段里了");
    }

    #[test]
    fn 追加时_setvalue_的值等于_current_加_typed() {
        // 对"追加"这一类计划，value 必须等于 current + typed，
        // 这样两种机制写入的结果才一致。
        for current in ["hello ", "验证码：", "abc"] {
            let plan = plan(Some(current), Some("868740"));

            assert_eq!(plan.decision, InsertionDecision::Write);
            assert_eq!(plan.typed.as_deref(), Some("868740"));
            assert_eq!(
                plan.value.as_deref(),
                Some(format!("{current}868740").as_str())
            );
        }
    }
}
