//! 应用数据路径。
//!
//! 全部落在 Tauri 约定的 `%LOCALAPPDATA%\<identifier>` 下：
//! - `config.json` —— 配置（设置界面读写的就是它）
//! - `logs\` —— 日志（tauri-plugin-log 写入）

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

pub fn data_dir(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_local_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("com.smspop.app"));

    let _ = std::fs::create_dir_all(&dir);

    dir
}

pub fn config_path(app: &AppHandle) -> PathBuf {
    data_dir(app).join("config.json")
}

pub fn log_dir(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_log_dir()
        .unwrap_or_else(|_| data_dir(app).join("logs"));

    let _ = std::fs::create_dir_all(&dir);

    dir
}
