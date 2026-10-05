//! 与传输层无关的统一通知模型。
//!
//! 上层（过滤、提取、弹窗、webhook）只认这个类型，完全不知道 ANCS 的存在 ——
//! 这样以后接 Android 时上层不用改。

use std::time::SystemTime;

use crate::ancs::Attributes;
use crate::otp;

/// 一条手机通知。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhoneNotification {
    /// 来源设备标识（BLE 地址等）。
    pub device_id: String,

    /// 来源设备展示名，例如 iPhone 的名称。
    pub device_name: Option<String>,

    /// 传输层内的通知唯一 ID（ANCS 里是 NotificationUID）。
    pub uid: u32,

    /// App 标识（iOS 上是 bundle id，例如 `com.apple.MobileSMS`）。
    pub app_identifier: Option<String>,

    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub message: Option<String>,

    /// 识别出的验证码，没有则为 `None`。
    pub code: Option<String>,

    pub received_at: SystemTime,
}

impl PhoneNotification {
    /// 从 ANCS 回读到的属性构造，顺手把验证码提取出来。
    pub fn from_ancs(
        device_id: impl Into<String>,
        device_name: Option<String>,
        attributes: Attributes,
    ) -> Self {
        let mut notification = Self {
            device_id: device_id.into(),
            device_name,
            uid: attributes.uid,
            app_identifier: attributes.app_identifier.clone(),
            title: attributes.title.clone(),
            subtitle: attributes.subtitle.clone(),
            message: attributes.message.clone(),
            code: None,
            received_at: SystemTime::now(),
        };

        notification.code = otp::extract_otp(&notification.full_text());
        notification
    }

    pub fn has_code(&self) -> bool {
        self.code.as_ref().is_some_and(|code| !code.is_empty())
    }

    /// 保守提取短信正文签名；无法识别时由 UI 显示通用文案。
    pub fn otp_source_hint(&self) -> Option<String> {
        if !self.has_code() || self.app_identifier.as_deref() != Some("com.apple.MobileSMS") {
            return None;
        }
        crate::otp_source::extract_sms_source(self.message.as_deref()?)
    }

    /// 内容指纹（设备 + App + 标题 + 正文）。
    ///
    /// 用途：iOS 在每次 ANCS 订阅成功后会把通知中心的存量重推一遍，
    /// uid 每次都是新的，按 uid 去重拦不住 —— 订阅后的宽限期里改按这个键去重。
    pub fn content_key(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.device_id,
            self.app_identifier.as_deref().unwrap_or(""),
            self.title.as_deref().unwrap_or(""),
            self.message.as_deref().unwrap_or("")
        )
    }

    /// 标题/副标题/正文拼起来（跳过空白），用于展示与关键词匹配。
    pub fn full_text(&self) -> String {
        [&self.title, &self.subtitle, &self.message]
            .iter()
            .filter_map(|value| value.as_deref())
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 弹窗上展示的一行摘要（优先正文）。
    pub fn summary(&self) -> String {
        match self.message.as_deref() {
            Some(message) if !message.trim().is_empty() => message.to_string(),
            _ => self.title.clone().unwrap_or_default(),
        }
    }

    /// 发件人/来源，用于弹窗上的一行小字。
    ///
    /// 优先用标题（短信的标题就是发件号码），其次才是 App 标识。
    pub fn origin(&self) -> String {
        if let Some(title) = self
            .title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
        {
            return title.to_string();
        }

        if let Some(app) = self.app_identifier.as_deref().filter(|app| !app.is_empty()) {
            return friendly_app_name(app);
        }

        self.device_name.clone().unwrap_or_default()
    }

    /// App 标识转成人看得懂的名字。认不出来就原样返回 bundle id —— 不猜。
    pub fn app_display_name(&self) -> String {
        self.app_identifier
            .as_deref()
            .filter(|app| !app.is_empty())
            .map(friendly_app_name)
            .unwrap_or_default()
    }
}

/// 几个常见 iOS App 的 bundle id 映射。
///
/// 只放"肯定认得出来"的；认不出来就原样显示 bundle id，
/// 免得把某银行的短信标成"短信"之后用户更糊涂。
pub fn friendly_app_name(bundle_id: &str) -> String {
    match bundle_id {
        "com.apple.MobileSMS" => "短信".to_string(),
        "com.apple.mobilemail" => "邮件".to_string(),
        "com.tencent.xin" => "微信".to_string(),
        "com.tencent.mqq" => "QQ".to_string(),
        "com.alipay.iphoneclient" => "支付宝".to_string(),
        "com.taobao.taobao4iphone" => "淘宝".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ancs::{ATTR_APP_IDENTIFIER, ATTR_MESSAGE, ATTR_SUBTITLE, ATTR_TITLE};

    #[test]
    fn 来源提示只识别有验证码的短信正文() {
        let mut notification = PhoneNotification::from_ancs(
            "device",
            None,
            Attributes {
                app_identifier: Some("com.apple.MobileSMS".into()),
                message: Some("【网易】验证码123456".into()),
                ..Default::default()
            },
        );
        assert_eq!(notification.otp_source_hint(), Some("网易".into()));
        notification.app_identifier = Some("com.tencent.xin".into());
        assert_eq!(notification.otp_source_hint(), None);
        notification.app_identifier = Some("com.apple.MobileSMS".into());
        notification.code = None;
        assert_eq!(notification.otp_source_hint(), None);
    }

    fn attributes(uid: u32, tuples: &[(u8, &str)]) -> Attributes {
        let mut attributes = Attributes {
            uid,
            ..Default::default()
        };

        for (id, value) in tuples {
            match *id {
                ATTR_APP_IDENTIFIER => attributes.app_identifier = Some(value.to_string()),
                ATTR_TITLE => attributes.title = Some(value.to_string()),
                ATTR_SUBTITLE => attributes.subtitle = Some(value.to_string()),
                ATTR_MESSAGE => attributes.message = Some(value.to_string()),
                _ => {}
            }
        }

        attributes
    }

    #[test]
    fn 从_ancs_属性构造时自动提取验证码() {
        let notification = PhoneNotification::from_ancs(
            "device",
            Some("iPhone".into()),
            attributes(
                39,
                &[
                    (ATTR_APP_IDENTIFIER, "com.apple.MobileSMS"),
                    (ATTR_TITLE, "106835812921010"),
                    (ATTR_MESSAGE, "【网易】验证码：868740，请勿泄露"),
                ],
            ),
        );

        assert_eq!(notification.uid, 39);
        assert!(notification.has_code());
        assert_eq!(notification.code.as_deref(), Some("868740"));
        assert_eq!(notification.origin(), "106835812921010");
    }

    #[test]
    fn 没有验证码时_has_code_为假() {
        let notification = PhoneNotification::from_ancs(
            "device",
            None,
            attributes(1, &[(ATTR_MESSAGE, "今天天气不错")]),
        );

        assert!(!notification.has_code());
        assert_eq!(notification.code, None);
    }

    #[test]
    fn full_text_跳过空白部分() {
        let notification = PhoneNotification::from_ancs(
            "device",
            None,
            attributes(
                1,
                &[
                    (ATTR_TITLE, "标题"),
                    (ATTR_SUBTITLE, "   "),
                    (ATTR_MESSAGE, "正文"),
                ],
            ),
        );

        assert_eq!(notification.full_text(), "标题\n正文");
    }

    #[test]
    fn summary_优先正文_没有正文才用标题() {
        let with_message = PhoneNotification::from_ancs(
            "d",
            None,
            attributes(1, &[(ATTR_TITLE, "标题"), (ATTR_MESSAGE, "正文")]),
        );
        assert_eq!(with_message.summary(), "正文");

        let without_message =
            PhoneNotification::from_ancs("d", None, attributes(1, &[(ATTR_TITLE, "标题")]));
        assert_eq!(without_message.summary(), "标题");
    }

    #[test]
    fn origin_认得出常见_app() {
        let notification = PhoneNotification::from_ancs(
            "d",
            Some("iPhone".into()),
            attributes(1, &[(ATTR_APP_IDENTIFIER, "com.apple.MobileSMS")]),
        );

        // 没有标题时用 App 标识，并且映射成中文
        assert_eq!(notification.origin(), "短信");
        assert_eq!(notification.app_display_name(), "短信");
    }

    #[test]
    fn origin_认不出来的_app_原样返回() {
        assert_eq!(
            friendly_app_name("com.some.unknown.app"),
            "com.some.unknown.app"
        );
    }

    #[test]
    fn origin_最后兜底到设备名() {
        let notification =
            PhoneNotification::from_ancs("d", Some("我的 iPhone".into()), attributes(1, &[]));

        assert_eq!(notification.origin(), "我的 iPhone");
    }
}
