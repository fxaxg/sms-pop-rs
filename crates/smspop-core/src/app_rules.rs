//! User-configured URL actions. Notification text is never executable input.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "StoredRule")]
pub struct AppRule {
    pub app_id: String,
    pub name: String,
    pub enabled: bool,
    pub target: String,
}

// Read earlier experimental configurations without resetting the user's settings.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRule {
    app_id: String,
    name: String,
    enabled: bool,
    target: String,
    #[serde(default)]
    kind: Option<String>,
}

impl TryFrom<StoredRule> for AppRule {
    type Error = String;
    fn try_from(rule: StoredRule) -> Result<Self, Self::Error> {
        if rule
            .kind
            .as_deref()
            .is_some_and(|kind| !matches!(kind, "executable" | "https" | "url"))
        {
            return Err("未知的打开方式".into());
        }
        Ok(Self {
            app_id: rule.app_id,
            name: rule.name,
            enabled: rule.enabled && rule.kind.as_deref() != Some("executable"),
            target: rule.target,
        })
    }
}

impl AppRule {
    pub fn validate(&self) -> Result<(), String> {
        if self.app_id.trim().is_empty() || self.name.trim().is_empty() {
            return Err("来源标识和规则名称不能为空".into());
        }
        validate_url(&self.target)
    }
}

pub fn validate_url(target: &str) -> Result<(), String> {
    if target.chars().any(|c| c.is_control() || c.is_whitespace())
        || target.contains(['"', '\\', '<', '>'])
    {
        return Err("URL 包含非法字符".into());
    }
    let (scheme, rest) = target
        .split_once(':')
        .ok_or("请输入 HTTPS 链接或应用 URL Scheme")?;
    if scheme.len() < 2
        || !scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return Err("无效的 URL 协议".into());
    }
    let scheme = scheme.to_ascii_lowercase();
    if matches!(
        scheme.as_str(),
        "file"
            | "javascript"
            | "data"
            | "vbscript"
            | "http"
            | "shell"
            | "powershell"
            | "cmd"
            | "ms-settings"
            | "ms-msdt"
            | "search-ms"
            | "search"
            | "ms-appinstaller"
    ) {
        return Err("不支持此 URL 协议".into());
    }
    if scheme == "https" {
        let authority = rest
            .strip_prefix("//")
            .unwrap_or("")
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("");
        if authority.is_empty() || authority.contains('@') {
            return Err("请输入有效的 HTTPS 链接".into());
        }
    } else if rest.is_empty() {
        return Err("应用协议目标不能为空，可使用 scheme://".into());
    }
    Ok(())
}

pub fn matching_rule<'a>(rules: &'a [AppRule], app_id: Option<&str>) -> Option<&'a AppRule> {
    let id = app_id?;
    rules.iter().find(|rule| {
        rule.enabled && rule.app_id.eq_ignore_ascii_case(id) && rule.validate().is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn 支持应用协议和安全网页() {
        for url in [
            "weixin://",
            "tencent://",
            "mailto:hello@example.com",
            "https://example.com/inbox",
            "HTTPS://example.com",
        ] {
            assert!(validate_url(url).is_ok(), "{url}");
        }
        for url in [
            "C:\\Apps\\Weixin.exe",
            "javascript:alert(1)",
            "file:///foo",
            "https://",
            "https://?foo",
            "https://user@example.com",
            "ms-msdt://foo",
            "weixin://\n",
            "http://example.com",
            "weixin:",
        ] {
            assert!(validate_url(url).is_err(), "{url}");
        }
    }
    #[test]
    fn 精确匹配且忽略禁用() {
        let mut rules = vec![AppRule {
            app_id: "com.tencent.xin".into(),
            name: "微信".into(),
            enabled: true,
            target: "weixin://".into(),
        }];
        assert!(matching_rule(&rules, Some("COM.TENCENT.XIN")).is_some());
        assert!(matching_rule(&rules, Some("com.tencent.xin.other")).is_none());
        rules[0].enabled = false;
        assert!(matching_rule(&rules, Some("com.tencent.xin")).is_none());
    }
    #[test]
    fn 旧程序规则停用但保留其他设置() {
        let config = crate::config::Config::from_json_str(r#"{"notifications":{"app_rules":[{"app_id":"com.tencent.xin","name":"微信","enabled":true,"kind":"executable","target":"C:\\Weixin.exe"}]}}"#).unwrap();
        assert!(!config.notifications.app_rules[0].enabled);
        assert_eq!(config.notifications.app_rules[0].target, "C:\\Weixin.exe");
    }
}
