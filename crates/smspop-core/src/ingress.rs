//! 网络通知的纯逻辑校验与转换。外部设备不能提供执行目标或验证码。
use crate::{model::PhoneNotification, otp};
use serde::Deserialize;
use std::time::SystemTime;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingNotification {
    #[serde(default)]
    pub device_name: Option<String>,
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default = "sms")]
    pub kind: String,
    #[serde(default, alias = "from")]
    pub sender: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(alias = "content")]
    pub body: String,
    #[serde(default)]
    pub app_identifier: Option<String>,
}
fn sms() -> String {
    "sms".into()
}

/// 地址选择是启发式，不保证手机可达；所有地址仍允许手动选择。
pub fn address_rank(name: &str, address: std::net::Ipv4Addr) -> u8 {
    if address.is_loopback() {
        return 90;
    }
    if address.is_link_local() {
        return 80;
    }
    let name = name.to_lowercase();
    let virtual_adapter = [
        "virtual",
        "vethernet",
        "vmware",
        "virtualbox",
        "vpn",
        "tailscale",
        "wireguard",
        "zerotier",
        "wsl",
        "docker",
        "hyper-v",
        "tunnel",
        "tap-",
        "tun",
        "虚拟",
    ]
    .iter()
    .any(|s| name.contains(s));
    if virtual_adapter {
        return 60;
    }
    if address.octets()[0..2] == [192, 168] {
        return 0;
    }
    if address.is_private() {
        return 10;
    }
    30
}
impl IncomingNotification {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !matches!(self.kind.as_str(), "sms" | "notification") {
            return Err("invalid_kind");
        }
        if self.body.trim().is_empty() || self.body.len() > 16384 {
            return Err("invalid_body");
        }
        if self
            .message_id
            .as_ref()
            .is_some_and(|id| id.trim().is_empty())
        {
            return Err("invalid_message_id");
        }
        for field in [
            &self.device_name,
            &self.sender,
            &self.title,
            &self.app_identifier,
            &self.message_id,
        ]
        .into_iter()
        .flatten()
        {
            if field.len() > 256 || field.contains('\0') {
                return Err("invalid_field");
            }
        }
        Ok(())
    }
    pub fn into_notification(self, device_id: String, name: String, uid: u32) -> PhoneNotification {
        let mut n = PhoneNotification {
            device_id,
            device_name: Some(name.clone()),
            uid,
            app_identifier: Some(if self.kind == "sms" {
                "smspop.network.sms".into()
            } else {
                // 网络 app 标识独立命名，避免冒充 BLE 来源规则。
                format!(
                    "network:{}",
                    self.app_identifier.unwrap_or_else(|| "notification".into())
                )
            }),
            title: Some(format!(
                "[网络转发 · {name}] {}",
                self.sender.or(self.title).unwrap_or_default()
            )),
            subtitle: None,
            message: Some(self.body),
            code: None,
            received_at: SystemTime::now(),
        };
        n.code = otp::extract_otp(&n.full_text());
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_lan_preferred_over_virtual_and_loopback() {
        let rank = |name, ip: &str| address_rank(name, ip.parse().unwrap());
        assert!(rank("Wi-Fi", "192.168.31.25") < rank("VMware Virtual Ethernet", "192.168.1.1"));
        assert!(rank("Ethernet", "10.0.0.20") < rank("vEthernet WSL", "172.20.1.1"));
        assert!(rank("Wi-Fi", "192.168.31.25") < rank("loopback", "127.0.0.1"));
    }
    #[test]
    fn validates_and_extracts_locally() {
        let p: IncomingNotification =
            serde_json::from_str(r#"{"from":"1069","content":"验证码 123456"}"#).unwrap();
        assert!(p.validate().is_ok());
        let n = p.into_notification("http:test".into(), "Android".into(), 1);
        assert_eq!(n.code.as_deref(), Some("123456"));
        assert!(n.origin().contains("网络转发"));
    }
    #[test]
    fn rejects_empty_oversized_and_executable_fields() {
        assert!(
            serde_json::from_str::<IncomingNotification>(r#"{"body":"hello","target":"cmd"}"#)
                .is_err()
        );
        for body in [" ".into(), "x".repeat(16385)] {
            let p = IncomingNotification {
                device_name: None,
                message_id: None,
                kind: sms(),
                sender: None,
                title: None,
                body,
                app_identifier: None,
            };
            assert!(p.validate().is_err());
        }
    }
}
