//! 配置模型与加载。
//!
//! 配置文件在 `%LOCALAPPDATA%\SmsPop\config.json`，首次运行自动生成带注释的版本。
//! 正常用法是在设置界面里改（那边保存的也是这份文件），手改也行 —— 解析时允许 `//` 注释。

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::filter::{FilterOptions, NotificationFilter};
use crate::model::PhoneNotification;

/// 整个配置树。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// 通知弹出：把所有（通过过滤的）iPhone 通知弹到 Windows 上。
    pub notifications: NotificationOptions,

    /// 验证码增强：识别验证码、复制、光标候选条。
    pub otp: OtpOptions,
}

impl Config {
    /// 这条通知是否通过 App / 关键词过滤。
    pub fn passes_filter(&self, notification: &PhoneNotification) -> bool {
        NotificationFilter::new(self.notifications.filter.clone()).passes(notification)
    }

    /// 这条通知该不该弹窗。
    ///
    /// 弹窗有两种来源：
    /// - 通知弹出开着 → 所有通过过滤的通知都弹；
    /// - 验证码增强开着 → 带验证码的通知**总是**弹（即使通知弹出被关掉）。
    ///
    /// 调用前请先确认 [`Config::passes_filter`]。
    pub fn should_popup(&self, notification: &PhoneNotification) -> bool {
        if !self.passes_filter(notification) {
            return false;
        }

        self.notifications.enabled || self.should_offer_otp(notification)
    }

    /// 该不该给这条通知提供验证码增强（复制 + 候选条）。
    ///
    /// 调用前请先确认 [`Config::passes_filter`]。
    pub fn should_offer_otp(&self, notification: &PhoneNotification) -> bool {
        self.otp.enabled && notification.has_code()
    }
}

/// 通知弹出。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NotificationOptions {
    /// 总开关。关掉之后只剩验证码通知（如果验证码增强开着）。
    pub enabled: bool,

    /// App / 关键词过滤。
    pub filter: FilterOptions,

    /// 弹窗外观与行为。
    pub popup: PopupOptions,
}

impl Default for NotificationOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            filter: FilterOptions::default(),
            popup: PopupOptions::default(),
        }
    }
}

/// 验证码增强。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OtpOptions {
    /// 总开关：识别通知里的验证码。
    pub enabled: bool,

    /// 收到验证码时自动复制到剪贴板。
    pub auto_copy: bool,

    /// 光标候选条。
    pub caret: CaretOptions,

    /// 点击候选条后的填入方式。
    pub insertion: InsertionOptions,
}

impl Default for OtpOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_copy: true,
            caret: CaretOptions::default(),
            insertion: InsertionOptions::default(),
        }
    }
}

/// 右下角弹窗的外观与行为。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PopupOptions {
    /// 停留秒数，过了就淡出。
    pub duration_seconds: u32,

    /// 屏幕上最多同时显示几个，超出的先关掉最老的。
    pub max_visible: usize,

    /// 弹窗宽度（DIP）。
    pub width: f64,
}

impl Default for PopupOptions {
    fn default() -> Self {
        Self {
            duration_seconds: 8,
            max_visible: 3,
            width: 380.0,
        }
    }
}

/// 光标候选条的行为。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CaretOptions {
    /// 总开关。关掉之后验证码只剩弹窗 + 自动复制。
    pub enabled: bool,

    /// 候选条停留秒数，过了还没点就自己消失。
    pub duration_seconds: u32,

    /// 候选条与光标之间的间距（DIP）。
    ///
    /// 默认 **24** 而不是 6：中文输入法的候选窗也在光标正下方，
    /// 间距太小会重叠被挡住（实测被微信输入法挡过）。
    pub gap: f64,
}

impl Default for CaretOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            duration_seconds: 8,
            gap: 24.0,
        }
    }
}

/// 验证码怎么填进输入框。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InsertMode {
    /// 直接 `SetValue`：快，且不要求目标控件有焦点。
    #[default]
    Direct,

    /// 模拟键盘逐字键入：慢，需要目标可聚焦，适合对 `SetValue` 不生效的输入框。
    Simulate,
}

/// 填入方式。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InsertionOptions {
    /// `direct` 或 `simulate`。
    pub mode: InsertMode,

    /// `simulate` 模式下每个字符之间的间隔（毫秒）。
    ///
    /// 会被夹到 0~1000，免得配置写错把填入拖成几十秒。
    pub type_delay_ms: u32,
}

impl Default for InsertionOptions {
    fn default() -> Self {
        Self {
            mode: InsertMode::default(),
            type_delay_ms: 60,
        }
    }
}

impl InsertionOptions {
    /// 逐字键入的间隔，已夹到 0~1000ms。
    pub fn type_delay(&self) -> std::time::Duration {
        std::time::Duration::from_millis(u64::from(self.type_delay_ms.min(1000)))
    }
}

/// 配置加载失败的原因。
#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "读配置文件失败：{error}"),
            Self::Json(error) => write!(f, "配置文件不是合法的 JSON：{error}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl Config {
    /// 从文件加载。文件不存在时返回默认配置（并且**不**报错）。
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_json_str(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(ConfigError::Io(error)),
        }
    }

    /// 从 JSON 文本解析。允许 `//` 行注释。
    pub fn from_json_str(text: &str) -> Result<Self, ConfigError> {
        Ok(serde_json::from_str(&strip_line_comments(text))?)
    }

    /// 序列化成 JSON（不带注释）。
    pub fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// 写一份带注释的默认配置到指定路径。父目录会被创建。
    pub fn write_default_config(path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, DEFAULT_CONFIG_JSON)
    }

    /// 保存配置。父目录会被创建。
    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, self.to_json_string())
    }
}

/// 首次运行时生成的配置文件内容。
///
/// 故意带注释 —— 这份文件得自己讲清楚每个键是干什么的。
/// 解析时 [`strip_line_comments`] 会把注释去掉。
pub const DEFAULT_CONFIG_JSON: &str = r#"{
  // ── 通知弹出 ────────────────────────────────────────────────
  // 把 iPhone 的通知实时弹到 Windows 右下角。
  "notifications": {
    // 总开关。关掉之后只有带验证码的通知会弹（如果验证码增强开着）。
    "enabled": true,

    "filter": {
      // 永不弹出的 App（iOS 的 bundle id，精确匹配，不区分大小写）
      "exclude_apps": [],
      // 非空时，只有这些 App 的通知才弹
      "include_apps": [],

      // 命中任一关键词就不弹（包含匹配，不区分大小写）
      "exclude_keywords": [],
      // 非空时，必须命中其中一个关键词才弹
      "include_keywords": []
    },

    "popup": {
      // 停留秒数
      "duration_seconds": 8,
      // 最多同时显示几个，超出的先关掉最老的
      "max_visible": 3,
      // 弹窗宽度（像素）
      "width": 380.0
    }
  },

  // ── 验证码增强 ──────────────────────────────────────────────
  "otp": {
    // 总开关：识别通知里的验证码
    "enabled": true,
    // 收到验证码时自动复制到剪贴板
    "auto_copy": true,

    // 光标候选条：收到验证码时，如果光标正好在一个可写输入框里，
    // 会在光标旁冒出一条「填入 868740」。**只有你点它才会写进去**。
    "caret": {
      "enabled": true,
      // 候选条停留秒数
      "duration_seconds": 8,
      // 与光标之间的间距。那个 24 是故意的：中文输入法的候选窗也在
      // 光标正下方，间距太小会被它挡住。嫌远可以调小。
      "gap": 24.0
    },

    // 点候选条之后，验证码怎么进输入框。两种模式都会**追加**到已有内容
    // 后面（除非内容本身就是验证码）；如果已经手打了前几位，会补全。
    "insertion": {
      // direct   = 直接写入（快，不碰焦点，绝大多数输入框都适用）
      // simulate = 模拟键盘逐字键入（慢，需要目标可聚焦）
      "mode": "direct",
      // simulate 模式下每个字符之间的间隔（毫秒，会被夹到 0~1000）
      "type_delay_ms": 60
    }
  }
}
"#;

/// 去掉 `//` 开头的行注释。
///
/// 只认**不在字符串里**的 `//`，并且会正确跳过 `"` 里的 `\"` 转义。
pub fn strip_line_comments(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut chars = text.chars().peekable();

    while let Some(current) = chars.next() {
        if in_string {
            output.push(current);

            if escaped {
                escaped = false;
            } else if current == '\\' {
                escaped = true;
            } else if current == '"' {
                in_string = false;
            }

            continue;
        }

        if current == '"' {
            in_string = true;
            output.push(current);
            continue;
        }

        if current == '/' && chars.peek() == Some(&'/') {
            for next in chars.by_ref() {
                if next == '\n' {
                    output.push('\n');
                    break;
                }
            }

            continue;
        }

        output.push(current);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ancs::Attributes;
    use crate::model::PhoneNotification;

    fn notification_with_code(code: Option<&str>) -> PhoneNotification {
        let mut notification = PhoneNotification::from_ancs(
            "device",
            None,
            Attributes {
                uid: 1,
                message: Some("你好".to_string()),
                ..Default::default()
            },
        );
        notification.code = code.map(str::to_string);
        notification
    }

    #[test]
    fn 默认配置能解析() {
        let config = Config::from_json_str(DEFAULT_CONFIG_JSON).expect("默认配置必须能解析");

        assert_eq!(config, Config::default());
        assert!(config.notifications.enabled);
        assert!(config.otp.enabled);
        assert!(config.otp.auto_copy);
        assert!(config.otp.caret.enabled);
        assert_eq!(config.otp.caret.gap, 24.0);
        assert_eq!(config.otp.insertion.mode, InsertMode::Direct);
    }

    #[test]
    fn 默认行为是全都弹() {
        let config = Config::default();

        assert!(config.should_popup(&notification_with_code(Some("123456"))));
        assert!(config.should_popup(&notification_with_code(None)));
    }

    #[test]
    fn 关掉通知弹出后只剩验证码通知() {
        let mut config = Config::default();
        config.notifications.enabled = false;

        assert!(config.should_popup(&notification_with_code(Some("123456"))));
        assert!(!config.should_popup(&notification_with_code(None)));
    }

    #[test]
    fn 两个功能都关掉什么都不弹() {
        let mut config = Config::default();
        config.notifications.enabled = false;
        config.otp.enabled = false;

        assert!(!config.should_popup(&notification_with_code(Some("123456"))));
        assert!(!config.should_popup(&notification_with_code(None)));
    }

    #[test]
    fn 过滤器先于一刀切开关() {
        let mut config = Config::default();
        config.notifications.filter.exclude_keywords = vec!["广告".to_string()];

        let mut ad = notification_with_code(Some("123456"));
        ad.message = Some("这是一条广告，验证码：123456".to_string());

        assert!(!config.should_popup(&ad));
    }

    #[test]
    fn 逐字延迟会被夹到合法范围() {
        let slow =
            Config::from_json_str(r#"{ "otp": { "insertion": { "type_delay_ms": 999999 } } }"#)
                .unwrap();
        let fast =
            Config::from_json_str(r#"{ "otp": { "insertion": { "type_delay_ms": 0 } } }"#).unwrap();

        assert_eq!(
            slow.otp.insertion.type_delay(),
            std::time::Duration::from_millis(1000)
        );
        assert_eq!(fast.otp.insertion.type_delay(), std::time::Duration::ZERO);
    }

    #[test]
    fn 空对象得到全默认值() {
        assert_eq!(Config::from_json_str("{}").unwrap(), Config::default());
    }

    #[test]
    fn 可以只写想改的字段() {
        let config = Config::from_json_str(r#"{ "otp": { "caret": { "gap": 6.0 } } }"#).unwrap();

        assert_eq!(config.otp.caret.gap, 6.0);
        assert_eq!(config.otp.caret.duration_seconds, 8);
        assert_eq!(config.notifications.popup.width, 380.0);
    }

    #[test]
    fn 未知字段会报错而不是被静默忽略() {
        let error = Config::from_json_str(r#"{ "notifications": { "enabeld": true } }"#);
        assert!(
            error.is_err(),
            "拼错的字段名应该报错，不然用户会以为改生效了"
        );
    }

    #[test]
    fn 行注释被正确去掉() {
        let text = r#"{
  // 这是注释
  "notifications": { "enabled": false }, // 行尾注释
  "otp": { "enabled": false } // 最后一行
}"#;

        let config = Config::from_json_str(text).unwrap();
        assert!(!config.notifications.enabled);
        assert!(!config.otp.enabled);
    }

    #[test]
    fn 字符串里的双斜杠不会被当成注释() {
        let config = Config::from_json_str(
            r#"{ "notifications": { "filter": { "include_keywords": ["http://a.example"] } } }"#,
        )
        .unwrap();

        assert_eq!(
            config.notifications.filter.include_keywords,
            vec!["http://a.example".to_string()]
        );
    }

    #[test]
    fn 转义引号里的双斜杠也不会被切掉() {
        let stripped = strip_line_comments(r#""a\"//b" // 真注释"#);

        assert_eq!(stripped.trim(), r#""a\"//b""#);
    }

    #[test]
    fn 没有换行的文件末尾注释也能处理() {
        assert_eq!(strip_line_comments("{} // 结束").trim(), "{}");
    }

    #[test]
    fn 序列化再解析回来一致() {
        let config = Config::default();
        let round_tripped = Config::from_json_str(&config.to_json_string()).unwrap();

        assert_eq!(config, round_tripped);
    }

    #[test]
    fn 文件不存在时返回默认而不是报错() {
        let config = Config::load_or_default("这个路径肯定不存在/config.json").unwrap();
        assert_eq!(config, Config::default());
    }
}
