//! ANCS 协议层：UUID、报文构造、分片响应解析。
//!
//! 这里刻意不碰任何蓝牙 API —— 纯字节处理。
//! 因为它是整个项目最容易写错的地方，必须能独立单测。
//!
//! 下面这些约束全都是从真机实测踩出来的（见 C# 版 `docs/ANCS-实现说明.md`）：
//!
//! * 属性长度是**字节**不是字符 —— 按字节切完再整体 UTF-8 解码，否则中文必乱码
//! * Data Source 的响应**可能分片** —— 收满 N 个 tuple 才算完整
//! * 请求了几个属性就一定会回几个 tuple（缺失的用 length=0 占位）
//! * `AppIdentifier` 请求里**不带**长度字段，其余属性都带
//! * `Removed` 事件和 `PreExisting` 标记的通知都不该去回读属性

use crate::uuid::Uuid;

// ---------------------------------------------------------------------------
// UUID
// ---------------------------------------------------------------------------

/// ANCS 服务。
pub const ANCS_SERVICE_UUID: Uuid = Uuid::from_bytes([
    0x79, 0x05, 0xF4, 0x31, 0xB5, 0xCE, 0x4E, 0x99, 0xA4, 0x0F, 0x4B, 0x1E, 0x12, 0x2D, 0x00, 0xD0,
]);

/// Notification Source：iPhone 往这里推 8 字节事件。
pub const NOTIFICATION_SOURCE_UUID: Uuid = Uuid::from_bytes([
    0x9F, 0xBF, 0x12, 0x0D, 0x63, 0x01, 0x42, 0xD9, 0x8C, 0x58, 0x25, 0xE6, 0x99, 0xA2, 0x1D, 0xBD,
]);

/// Control Point：我们往这里写请求。
pub const CONTROL_POINT_UUID: Uuid = Uuid::from_bytes([
    0x69, 0xD1, 0xD8, 0xF3, 0x45, 0xE1, 0x49, 0xA8, 0x98, 0x21, 0x9B, 0xBD, 0xFD, 0xAA, 0xD9, 0xD9,
]);

/// Data Source：iPhone 往这里推属性内容（可能分片）。
pub const DATA_SOURCE_UUID: Uuid = Uuid::from_bytes([
    0x22, 0xEA, 0xC6, 0xE9, 0x24, 0xD6, 0x4B, 0xB5, 0xBE, 0x44, 0xB3, 0x6A, 0xCE, 0x7C, 0x7B, 0xFB,
]);

/// 广播时"服务请求"的 AD 类型。
///
/// iPhone 就是靠这个认出"对面是个 ANCS 配件"。
pub const AD_TYPE_SERVICE_SOLICITATION_128BIT: u8 = 0x15;

// ---------------------------------------------------------------------------
// Notification Source（iPhone 推来的 8 字节事件）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventId {
    /// 新通知。
    Added,
    /// 已有通知被改了（比如短信又补了一条）。
    Modified,
    /// 通知被划掉了。
    Removed,
    Unknown(u8),
}

/// Notification Source 的载荷：`[EventID:1][Flags:1][Category:1][Count:1][UID:4 LE]`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationEvent {
    pub event_id: EventId,
    /// 事件标志。`0x01` = Silent，`0x02` = Important，`0x04` = PreExisting。
    pub flags: u8,
    /// iOS 的通知分类（短信是 4）。
    pub category: u8,
    /// 这条通知在 iPhone 上的唯一 ID。
    pub uid: u32,
}

impl NotificationEvent {
    /// 订阅瞬间 iOS 会补推一批"订阅之前就存在"的旧通知，用这个标记区分。
    pub fn is_pre_existing(&self) -> bool {
        self.flags & 0x04 != 0
    }

    /// 我们只关心"新来了"和"内容变了"，不关心"被划掉了"。
    pub fn should_fetch_attributes(&self) -> bool {
        matches!(self.event_id, EventId::Added | EventId::Modified) && !self.is_pre_existing()
    }
}

/// 解析 Notification Source 的载荷。长度不足 8 字节返回 `None`。
pub fn parse_notification_source(bytes: &[u8]) -> Option<NotificationEvent> {
    if bytes.len() < 8 {
        return None;
    }

    let event_id = match bytes[0] {
        0 => EventId::Added,
        1 => EventId::Modified,
        2 => EventId::Removed,
        other => EventId::Unknown(other),
    };

    Some(NotificationEvent {
        event_id,
        flags: bytes[1],
        category: bytes[2],
        uid: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
    })
}

// ---------------------------------------------------------------------------
// Control Point（我们写进去的请求）
// ---------------------------------------------------------------------------

pub const CMD_GET_NOTIFICATION_ATTRIBUTES: u8 = 0x00;

pub const ATTR_APP_IDENTIFIER: u8 = 0x00;
pub const ATTR_TITLE: u8 = 0x01;
pub const ATTR_SUBTITLE: u8 = 0x02;
pub const ATTR_MESSAGE: u8 = 0x03;

/// 单个属性最多请求多少字节。255 字节 ≈ 85 个汉字，短信足够。
pub const MAX_ATTRIBUTE_LENGTH: u16 = 255;

/// 我们固定请求的属性清单（顺序也是固定的）。
pub const REQUESTED_ATTRIBUTES: [u8; 4] =
    [ATTR_APP_IDENTIFIER, ATTR_TITLE, ATTR_SUBTITLE, ATTR_MESSAGE];

/// 请求了几个属性 —— Data Source 就会回几个 tuple。
pub const REQUESTED_ATTRIBUTE_COUNT: usize = REQUESTED_ATTRIBUTES.len();

/// 构造 `GetNotificationAttributes` 请求。
///
/// ★ 用**原始 4 字节**回传 UID —— iPhone 发来什么样就什么样。
///   实测：按小端原样回传是正确的，不要试图"规范化"它。
pub fn build_get_attributes_request(raw_uid: [u8; 4]) -> Vec<u8> {
    let mut pdu = Vec::with_capacity(16);

    pdu.push(CMD_GET_NOTIFICATION_ATTRIBUTES);
    pdu.extend_from_slice(&raw_uid);

    for attribute in REQUESTED_ATTRIBUTES {
        pdu.push(attribute);

        // AppIdentifier 后面不带长度字段，只有它例外
        if attribute != ATTR_APP_IDENTIFIER {
            pdu.push((MAX_ATTRIBUTE_LENGTH & 0xFF) as u8);
            pdu.push((MAX_ATTRIBUTE_LENGTH >> 8) as u8);
        }
    }

    pdu
}

// ---------------------------------------------------------------------------
// Data Source（iPhone 分片推回来的属性）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseStatus {
    /// 数据还不够，等下一个分片。
    Incomplete,
    /// 解析完成，tuple 已收满。
    Complete,
    /// 参数不合法（比如期望 tuple 数为 0）。
    Invalid,
}

/// 一次响应解析出来的属性内容。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Attributes {
    pub uid: u32,
    pub app_identifier: Option<String>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub message: Option<String>,
}

impl Attributes {
    /// 标题/副标题/正文拼起来（跳过空白），用于展示和关键词匹配。
    pub fn full_text(&self) -> String {
        [&self.title, &self.subtitle, &self.message]
            .iter()
            .filter_map(|value| value.as_deref())
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// 尝试解析 Data Source 累积到的字节。
///
/// 响应格式：`[CommandID:1][NotificationUID:4 LE]` 之后 N 个 `[AttributeID:1][Length:2 LE][Value]`。
///
/// 返回 `(状态, 解析结果, 消费掉的字节数)`。
/// 状态是 `Incomplete` 时，调用方应保留数据等下一个分片。
///
/// ★ 长度是**字节**：先按字节切，再整体 UTF-8 解码，这样切在多字节汉字中间也不会乱。
pub fn try_parse_response(
    data: &[u8],
    expected_tuples: usize,
) -> (ParseStatus, Option<Attributes>, usize) {
    if expected_tuples == 0 {
        return (ParseStatus::Invalid, None, 0);
    }

    if data.len() < 5 {
        return (ParseStatus::Incomplete, None, 0);
    }

    let mut attributes = Attributes {
        uid: u32::from_le_bytes([data[1], data[2], data[3], data[4]]),
        ..Default::default()
    };

    // CommandID 我们不校验，但 5 字节头必须跳过
    let mut index = 5usize;

    for _ in 0..expected_tuples {
        if index + 3 > data.len() {
            return (ParseStatus::Incomplete, None, 0);
        }

        let attribute_id = data[index];
        let length = u16::from_le_bytes([data[index + 1], data[index + 2]]) as usize;

        if index + 3 + length > data.len() {
            return (ParseStatus::Incomplete, None, 0);
        }

        let value = String::from_utf8_lossy(&data[index + 3..index + 3 + length]).into_owned();

        match attribute_id {
            ATTR_APP_IDENTIFIER => attributes.app_identifier = Some(value),
            ATTR_TITLE => attributes.title = Some(value),
            ATTR_SUBTITLE => attributes.subtitle = Some(value),
            ATTR_MESSAGE => attributes.message = Some(value),
            // iOS 理论上只回我们请求过的属性，别的直接忽略
            _ => {}
        }

        index += 3 + length;
    }

    (ParseStatus::Complete, Some(attributes), index)
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::otp::extract_otp;

    /// 按响应格式拼一条报文出来。
    fn build_response(uid: u32, tuples: &[(u8, &str)]) -> Vec<u8> {
        let mut buffer = vec![0x00];
        buffer.extend_from_slice(&uid.to_le_bytes());

        for (id, value) in tuples {
            let payload = value.as_bytes();
            buffer.push(*id);
            buffer.push((payload.len() & 0xFF) as u8);
            buffer.push((payload.len() >> 8) as u8);
            buffer.extend_from_slice(payload);
        }

        buffer
    }

    #[test]
    fn notification_source_解析正确() {
        // Added, 无 flags, category=4(短信), count=1, uid=39
        let event = parse_notification_source(&[0, 0x00, 4, 1, 39, 0, 0, 0]).unwrap();

        assert_eq!(event.event_id, EventId::Added);
        assert_eq!(event.category, 4);
        assert_eq!(event.uid, 39);
        assert!(event.should_fetch_attributes());
    }

    #[test]
    fn notification_source_长度不足时返回_none() {
        assert!(parse_notification_source(&[]).is_none());
        assert!(parse_notification_source(&[0, 0, 4, 1, 39, 0, 0]).is_none());
    }

    #[test]
    fn removed_和_preexisting_都不该去回读() {
        let removed = parse_notification_source(&[2, 0, 0, 0, 1, 0, 0, 0]).unwrap();
        assert_eq!(removed.event_id, EventId::Removed);
        assert!(!removed.should_fetch_attributes());

        let pre_existing = parse_notification_source(&[0, 0x04, 0, 0, 1, 0, 0, 0]).unwrap();
        assert!(pre_existing.is_pre_existing());
        assert!(!pre_existing.should_fetch_attributes());

        // Modified 是"内容变了"，要回读
        let modified = parse_notification_source(&[1, 0, 0, 0, 1, 0, 0, 0]).unwrap();
        assert!(modified.should_fetch_attributes());
    }

    #[test]
    fn 请求报文格式符合预期() {
        // uid = 38，按小端原样回传
        let request = build_get_attributes_request([0x26, 0x00, 0x00, 0x00]);

        assert_eq!(
            request,
            vec![
                0x00, // GetNotificationAttributes
                0x26, 0x00, 0x00, 0x00, // uid（原样）
                0x00, // AppIdentifier —— 后面不带长度
                0x01, 0xFF, 0x00, // Title
                0x02, 0xFF, 0x00, // Subtitle
                0x03, 0xFF, 0x00, // Message
            ]
        );
    }

    #[test]
    fn 请求的属性个数和常量一致() {
        let request = build_get_attributes_request([0, 0, 0, 0]);
        // 1 字节命令 + 4 字节 uid；AppIdentifier 占 1 字节；其余三个各占 3 字节
        assert_eq!(request.len(), 1 + 4 + 1 + 3 * 3);
        assert_eq!(REQUESTED_ATTRIBUTE_COUNT, 4);
    }

    #[test]
    fn 单包响应能解码中文并提取验证码() {
        let data = build_response(
            39,
            &[
                (ATTR_APP_IDENTIFIER, "com.apple.MobileSMS"),
                (ATTR_TITLE, "106835812921010"),
                (ATTR_SUBTITLE, ""),
                (
                    ATTR_MESSAGE,
                    "【网易】验证码：868740，您正在登录网易手机账号（若非本人操作，请删除本短信）",
                ),
            ],
        );

        let (status, attributes, consumed) = try_parse_response(&data, 4);

        assert_eq!(status, ParseStatus::Complete);
        assert_eq!(consumed, data.len());

        let attributes = attributes.unwrap();
        assert_eq!(attributes.uid, 39);
        assert_eq!(
            attributes.app_identifier.as_deref(),
            Some("com.apple.MobileSMS")
        );
        assert_eq!(attributes.title.as_deref(), Some("106835812921010"));
        assert_eq!(attributes.subtitle.as_deref(), Some(""));
        assert_eq!(
            extract_otp(&attributes.full_text()).as_deref(),
            Some("868740")
        );
    }

    #[test]
    fn 分片时先报不完整再拼接成功() {
        let data = build_response(
            7,
            &[
                (ATTR_TITLE, "106"),
                (ATTR_SUBTITLE, ""),
                (ATTR_MESSAGE, "【腾讯】验证码：123456"),
            ],
        );

        // 从每个前缀长度都试一遍：只要是真前缀就必须是 Incomplete，不能误判成完整
        for cut in 1..data.len() {
            let (status, _, _) = try_parse_response(&data[..cut], 3);
            assert_eq!(
                status,
                ParseStatus::Incomplete,
                "切在第 {cut} 字节时不应判为完整"
            );
        }

        let (status, attributes, consumed) = try_parse_response(&data, 3);
        assert_eq!(status, ParseStatus::Complete);
        assert_eq!(consumed, data.len());
        assert_eq!(
            attributes.unwrap().message.as_deref(),
            Some("【腾讯】验证码：123456")
        );
    }

    #[test]
    fn 切在多字节汉字中间也能正确拼接() {
        let message = "【腾讯】验证码：123456";
        let data = build_response(7, &[(ATTR_TITLE, "106"), (ATTR_MESSAGE, message)]);

        // 切点正好落在「【」的第二个字节之后 —— 多字节汉字的中间
        let cut = data.len() - message.len() + 2;

        assert_eq!(
            try_parse_response(&data[..cut], 2).0,
            ParseStatus::Incomplete
        );

        // 把两段拼起来（模拟真实的分片累积）再解析
        let (status, attributes, _) = try_parse_response(&data, 2);
        assert_eq!(status, ParseStatus::Complete);
        assert_eq!(attributes.unwrap().message.as_deref(), Some(message));
    }

    #[test]
    fn 期望数为零时明确报非法() {
        assert_eq!(try_parse_response(&[0, 1], 0).0, ParseStatus::Invalid);
        assert_eq!(try_parse_response(&[], 1).0, ParseStatus::Incomplete);
        assert_eq!(try_parse_response(&[0, 1, 2], 1).0, ParseStatus::Incomplete);
    }

    #[test]
    fn 字节长度大于字符数时按字节切() {
        // 「验证码」是 3 个字符 / 9 个字节。长度字段写 9，值就应该完整解出来。
        let data = build_response(1, &[(ATTR_MESSAGE, "验证码")]);
        let (status, attributes, _) = try_parse_response(&data, 1);

        assert_eq!(status, ParseStatus::Complete);
        assert_eq!(attributes.unwrap().message.as_deref(), Some("验证码"));
    }

    #[test]
    fn 未知的属性编号被忽略但不影响其它属性() {
        let data = build_response(5, &[(0x7F, "不认识"), (ATTR_MESSAGE, "验证码：123456")]);
        let (status, attributes, _) = try_parse_response(&data, 2);

        assert_eq!(status, ParseStatus::Complete);
        assert_eq!(
            attributes.unwrap().message.as_deref(),
            Some("验证码：123456")
        );
    }
}
