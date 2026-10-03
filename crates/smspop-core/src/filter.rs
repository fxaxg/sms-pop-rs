//! App / 关键词过滤。纯逻辑，可单测。
//!
//! 这里只回答一个问题：这条通知**过不过滤**。
//! 「过滤通过之后弹不弹」是 [`crate::config::Config::should_popup`] 的事。

use serde::{Deserialize, Serialize};

use crate::model::PhoneNotification;

/// 过滤规则。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FilterOptions {
    /// 永不弹出的 App（bundle id，例如 `com.tencent.xin`）。
    pub exclude_apps: Vec<String>,

    /// 非空时，只有这些 App 的通知才弹。
    pub include_apps: Vec<String>,

    /// 命中任一关键词就不弹。
    pub exclude_keywords: Vec<String>,

    /// 非空时，必须命中其中一个关键词才弹。
    pub include_keywords: Vec<String>,
}

/// 过滤器。持有规则，逐条判定。
#[derive(Debug, Clone, Default)]
pub struct NotificationFilter {
    options: FilterOptions,
}

impl NotificationFilter {
    pub fn new(options: FilterOptions) -> Self {
        Self { options }
    }

    pub fn options(&self) -> &FilterOptions {
        &self.options
    }

    /// 这条通知是否通过过滤（不排除任何通知时恒为真）。
    pub fn passes(&self, notification: &PhoneNotification) -> bool {
        let app = notification.app_identifier.as_deref().unwrap_or_default();
        let text = notification.full_text();

        if matches_any(app, &self.options.exclude_apps) {
            return false;
        }

        if !self.options.include_apps.is_empty() && !matches_any(app, &self.options.include_apps) {
            return false;
        }

        if contains_any(&text, &self.options.exclude_keywords) {
            return false;
        }

        if !self.options.include_keywords.is_empty()
            && !contains_any(&text, &self.options.include_keywords)
        {
            return false;
        }

        true
    }
}

/// App 标识是**精确**比较（大小写不敏感）。
fn matches_any(value: &str, list: &[String]) -> bool {
    list.iter()
        .filter(|item| !item.trim().is_empty())
        .any(|item| value.eq_ignore_ascii_case(item.trim()))
}

/// 关键词是**包含**比较（大小写不敏感）。
fn contains_any(text: &str, keywords: &[String]) -> bool {
    if text.is_empty() {
        return false;
    }

    let haystack = text.to_lowercase();

    keywords
        .iter()
        .filter(|keyword| !keyword.trim().is_empty())
        .any(|keyword| haystack.contains(&keyword.trim().to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ancs::Attributes;

    fn notification(app: Option<&str>, message: &str) -> PhoneNotification {
        PhoneNotification::from_ancs(
            "device",
            None,
            Attributes {
                uid: 1,
                app_identifier: app.map(str::to_string),
                message: Some(message.to_string()),
                ..Default::default()
            },
        )
    }

    #[test]
    fn 默认全部放行() {
        let filter = NotificationFilter::default();

        assert!(filter.passes(&notification(Some("com.tencent.xin"), "在吗")));
        assert!(filter.passes(&notification(None, "")));
    }

    #[test]
    fn 排除名单是不区分大小写的精确匹配() {
        let filter = NotificationFilter::new(FilterOptions {
            exclude_apps: vec!["com.tencent.XIN".to_string()],
            ..Default::default()
        });

        assert!(!filter.passes(&notification(Some("com.tencent.xin"), "在吗")));

        // 只是前缀相同，不算命中
        assert!(filter.passes(&notification(Some("com.tencent.xin2"), "在吗")));
    }

    #[test]
    fn 白名单非空时只放行名单内的() {
        let filter = NotificationFilter::new(FilterOptions {
            include_apps: vec!["com.apple.MobileSMS".to_string()],
            ..Default::default()
        });

        assert!(filter.passes(&notification(Some("com.apple.MobileSMS"), "你好")));
        assert!(!filter.passes(&notification(Some("com.tencent.xin"), "你好")));
    }

    #[test]
    fn 关键词是包含匹配且不区分大小写() {
        let filter = NotificationFilter::new(FilterOptions {
            exclude_keywords: vec!["广告".to_string()],
            ..Default::default()
        });

        assert!(!filter.passes(&notification(None, "这是一条广告短信")));
        assert!(filter.passes(&notification(None, "这是一条正常短信")));
    }

    #[test]
    fn 包含关键词是或的关系() {
        let filter = NotificationFilter::new(FilterOptions {
            include_keywords: vec!["银行".to_string(), "验证".to_string()],
            ..Default::default()
        });

        assert!(filter.passes(&notification(None, "招商银行提醒")));
        assert!(filter.passes(&notification(None, "请验证身份")));
        assert!(!filter.passes(&notification(None, "外卖到了")));
    }

    #[test]
    fn 空字符串条目被忽略不影响判定() {
        let filter = NotificationFilter::new(FilterOptions {
            exclude_apps: vec!["".to_string(), "   ".to_string()],
            exclude_keywords: vec!["".to_string()],
            ..Default::default()
        });

        assert!(filter.passes(&notification(Some("com.tencent.xin"), "你好")));
    }

    #[test]
    fn 排除优先于包含() {
        let filter = NotificationFilter::new(FilterOptions {
            include_apps: vec!["com.apple.MobileSMS".to_string()],
            exclude_apps: vec!["com.apple.MobileSMS".to_string()],
            ..Default::default()
        });

        assert!(!filter.passes(&notification(Some("com.apple.MobileSMS"), "你好")));
    }
}
