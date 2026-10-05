//! 正文来源提示，不代表经过认证的发送方。只接受明确的短信签名。

pub fn extract_sms_source(message: &str) -> Option<String> {
    let text = message.trim();
    if text.matches('【').count() != 1 || text.matches('】').count() != 1 {
        return None;
    }
    let start = text.find('【')?;
    let end = text.find('】')?;
    if end <= start || (start != 0 && end + '】'.len_utf8() != text.len()) {
        return None;
    }
    let name = text[start + '【'.len_utf8()..end].trim();
    if !(2..=12).contains(&name.chars().count())
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(&c))
        || name.chars().all(|c| c.is_ascii_digit())
        || [
            "验证码",
            "校验码",
            "验证",
            "提醒",
            "通知",
            "短信",
            "安全",
            "警告",
        ]
        .iter()
        .any(|label| name.contains(label))
    {
        return None;
    }
    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 明确的首尾签名可识别() {
        assert_eq!(
            extract_sms_source("【网易】验证码：123456"),
            Some("网易".into())
        );
        assert_eq!(
            extract_sms_source("验证码为123456【招商银行】"),
            Some("招商银行".into())
        );
        assert_eq!(
            extract_sms_source(" 【GitHub】Your code is 123456 "),
            Some("GitHub".into())
        );
    }

    #[test]
    fn 歧义与普通正文不猜测() {
        for text in [
            "Your Example verification code is 123456",
            "验证码【网易】123456",
            "【网易】验证码123456【其他服务】",
            "【验证码】123456",
            "【安全提醒】123456",
            "【123456】验证码",
            "【访问example.com】123456",
            "【网\n易】123456",
            "【A】123456",
            "【这是一段超过十二个字符的短信说明】123456",
            "【网易验证码123456",
        ] {
            assert_eq!(extract_sms_source(text), None, "{text}");
        }
    }
}
