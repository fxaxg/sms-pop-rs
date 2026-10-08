//! 已知 iPhone 注册表：持久化、选择偏好和 UI 负载。

use std::path::PathBuf;
#[cfg(windows)]
use std::time::{SystemTime, UNIX_EPOCH};

use log::warn;
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use smspop_core::devices::sort_candidate_ids;
use smspop_core::devices::KnownDevice;
#[cfg(windows)]
use smspop_core::link::DeviceInfo;

#[derive(Debug, Default, Serialize, Deserialize)]
struct StoredDevices {
    #[serde(default)]
    devices: Vec<KnownDevice>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedDevicePayload {
    pub id: String,
    pub name: String,
    pub address_hint: String,
    pub preferred: bool,
    pub enabled: bool,
    pub current: bool,
    pub connected: bool,
    pub online: bool,
    pub verified: bool,
    pub last_connected_at: Option<u64>,
    pub last_verified_at: Option<u64>,
    pub battery_level: Option<u8>,
}

pub struct DeviceRegistry {
    path: PathBuf,
    devices: Vec<KnownDevice>,
    active_id: Option<String>,
    active_connected: bool,
    battery_level: Option<u8>,
    online_ids: Vec<String>,
}

impl DeviceRegistry {
    pub fn load(path: PathBuf) -> Self {
        let devices = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<StoredDevices>(&text) {
                Ok(stored) => stored.devices,
                Err(error) => {
                    warn!("设备注册表损坏（{error}），备份后重新建立");
                    let backup = path.with_extension("json.bak");
                    let _ = std::fs::copy(&path, backup);
                    Vec::new()
                }
            },
            Err(_) => Vec::new(),
        };
        Self {
            path,
            devices,
            active_id: None,
            active_connected: false,
            battery_level: None,
            online_ids: Vec::new(),
        }
    }

    #[cfg(windows)]
    pub fn discover(&mut self, discovered: &[DeviceInfo]) -> Vec<String> {
        let now = now_ms();
        self.online_ids = discovered.iter().map(|device| device.id.clone()).collect();
        for item in discovered {
            if let Some(device) = self.find_mut(&item.id) {
                device.name = item.name.clone();
                device.last_seen_unix_ms = Some(now);
            } else {
                self.devices.push(KnownDevice {
                    id: item.id.clone(),
                    name: item.name.clone(),
                    enabled: true,
                    preferred: self.devices.is_empty(),
                    last_seen_unix_ms: Some(now),
                    last_connected_unix_ms: None,
                    last_verified_unix_ms: None,
                });
            }
        }
        self.save();
        sort_candidate_ids(
            discovered.iter().map(|device| device.id.as_str()),
            &self.devices,
        )
    }

    #[cfg(windows)]
    pub fn set_active(&mut self, device: Option<&DeviceInfo>) {
        let previous_id = self.active_id.clone();
        self.active_id = device.map(|device| device.id.clone());
        self.active_connected = device.is_some_and(|device| device.connected);
        if previous_id.as_deref() != self.active_id.as_deref() || device.is_none() {
            self.battery_level = None;
        }
        if let Some(device) = device {
            let now = now_ms();
            if let Some(known) = self.find_mut(&device.id) {
                known.name = device.name.clone();
                if device.connected {
                    known.last_connected_unix_ms = Some(now);
                }
            }
            self.save();
        }
    }

    #[cfg(windows)]
    pub fn set_battery_level(&mut self, id: &str, level: u8) {
        if self
            .active_id
            .as_deref()
            .is_some_and(|active| active.eq_ignore_ascii_case(id))
        {
            self.battery_level = Some(level.min(100));
        }
    }

    #[cfg(windows)]
    pub fn mark_verified(&mut self, id: &str) {
        if let Some(device) = self.find_mut(id) {
            device.last_verified_unix_ms = Some(now_ms());
            self.save();
        }
    }

    pub fn set_preferred(&mut self, id: &str) -> Result<(), String> {
        if !self
            .devices
            .iter()
            .any(|device| device.id.eq_ignore_ascii_case(id))
        {
            return Err("设备不存在".into());
        }
        for device in &mut self.devices {
            device.preferred = device.id.eq_ignore_ascii_case(id);
        }
        self.save();
        Ok(())
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        let device = self.find_mut(id).ok_or("设备不存在")?;
        device.enabled = enabled;
        self.save();
        Ok(())
    }

    pub fn forget(&mut self, id: &str) -> Result<(), String> {
        if self
            .active_id
            .as_deref()
            .is_some_and(|active| active.eq_ignore_ascii_case(id))
        {
            return Err("当前连接的设备不能移除，请先让它断开".into());
        }
        let old_len = self.devices.len();
        self.devices
            .retain(|device| !device.id.eq_ignore_ascii_case(id));
        if self.devices.len() == old_len {
            return Err("设备不存在".into());
        }
        if !self.devices.iter().any(|device| device.preferred) {
            if let Some(first) = self.devices.iter_mut().find(|device| device.enabled) {
                first.preferred = true;
            }
        }
        self.save();
        Ok(())
    }

    pub fn payloads(&self) -> Vec<ManagedDevicePayload> {
        let mut payloads: Vec<_> = self
            .devices
            .iter()
            .map(|device| ManagedDevicePayload {
                id: device.id.clone(),
                name: if device.name.is_empty() {
                    "iPhone".into()
                } else {
                    device.name.clone()
                },
                address_hint: address_hint(&device.id),
                preferred: device.preferred,
                enabled: device.enabled,
                current: self
                    .active_id
                    .as_deref()
                    .is_some_and(|id| id.eq_ignore_ascii_case(&device.id)),
                connected: self.active_connected
                    && self
                        .active_id
                        .as_deref()
                        .is_some_and(|id| id.eq_ignore_ascii_case(&device.id)),
                online: self
                    .online_ids
                    .iter()
                    .any(|id| id.eq_ignore_ascii_case(&device.id)),
                verified: device.last_verified_unix_ms.is_some(),
                last_connected_at: device.last_connected_unix_ms,
                last_verified_at: device.last_verified_unix_ms,
                battery_level: self
                    .active_id
                    .as_deref()
                    .filter(|id| id.eq_ignore_ascii_case(&device.id))
                    .and(self.battery_level),
            })
            .collect();
        payloads.sort_by_key(|device| (!device.current, !device.preferred, device.name.clone()));
        payloads
    }

    #[cfg(windows)]
    pub fn active_id(&self) -> Option<String> {
        self.active_id.clone()
    }

    fn find_mut(&mut self, id: &str) -> Option<&mut KnownDevice> {
        self.devices
            .iter_mut()
            .find(|device| device.id.eq_ignore_ascii_case(id))
    }

    fn save(&self) {
        let stored = StoredDevices {
            devices: self.devices.clone(),
        };
        let result = serde_json::to_string_pretty(&stored)
            .map_err(|error| error.to_string())
            .and_then(|text| std::fs::write(&self.path, text).map_err(|error| error.to_string()));
        if let Err(error) = result {
            warn!("保存设备注册表失败：{error}");
        }
    }
}

#[cfg(windows)]
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn address_hint(id: &str) -> String {
    let tail = id
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("••••{tail}")
}
