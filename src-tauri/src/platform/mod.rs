use serde::Serialize;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;
#[derive(Serialize)]
pub struct PlatformCapabilities {
    pub os: &'static str,
    pub ble: bool,
    pub caret: bool,
    pub shortcut: bool,
}
#[tauri::command]
pub fn get_platform_capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        os: std::env::consts::OS,
        ble: cfg!(windows),
        caret: cfg!(windows),
        shortcut: cfg!(target_os = "macos"),
    }
}
