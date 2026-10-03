//! 系统托盘：图标 + 菜单 + 状态 tooltip。

use log::warn;
use smspop_ble::LinkState;
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::commands;
use crate::state::AppState;

const MENU_OPEN: &str = "open";
const MENU_TEST: &str = "test";
const MENU_AUTOSTART: &str = "autostart";
const MENU_QUIT: &str = "quit";

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let autostart_enabled = {
        use tauri_plugin_autostart::ManagerExt;
        app.autolaunch().is_enabled().unwrap_or(false)
    };

    let autostart_item = CheckMenuItemBuilder::with_id(MENU_AUTOSTART, "开机自启")
        .checked(autostart_enabled)
        .build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id(MENU_OPEN, "打开 SmsPop").build(app)?)
        .item(&MenuItemBuilder::with_id(MENU_TEST, "发送测试通知").build(app)?)
        .separator()
        .item(&autostart_item)
        .separator()
        .item(&MenuItemBuilder::with_id(MENU_QUIT, "退出").build(app)?)
        .build()?;

    // 菜单项句柄存起来，切换时要同步勾选状态
    app.state::<AppState>()
        .autostart_item
        .lock()
        .unwrap()
        .replace(autostart_item);

    let mut tray = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("SmsPop");

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.on_menu_event(|app, event| match event.id().as_ref() {
        MENU_OPEN => open_main_window(app),
        MENU_TEST => commands::send_test_notification_from(app.clone()),
        MENU_AUTOSTART => {
            if let Err(error) = toggle_autostart(app) {
                warn!("切换开机自启失败：{error}");
            }
        }
        MENU_QUIT => app.exit(0),
        _ => {}
    })
    .on_tray_icon_event(|tray, event| {
        // 左键点图标 = 打开主窗口
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            open_main_window(tray.app_handle());
        }
    })
    .build(app)?;

    Ok(())
}

/// 显示并聚焦主窗口（托盘点击 / 第二实例启动都走这里）。
pub fn open_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 链路状态变了 → 更新 tooltip。
pub fn on_link_state(app: &AppHandle, state: LinkState) {
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(&format!("SmsPop — {}", state.describe())));
    }
}

fn toggle_autostart(app: &AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;

    let autostart = app.autolaunch();
    let enabled = autostart.is_enabled().map_err(|error| error.to_string())?;

    if enabled {
        autostart.disable().map_err(|error| error.to_string())?;
    } else {
        autostart.enable().map_err(|error| error.to_string())?;
    }

    // 菜单勾选状态同步
    let state = app.state::<AppState>();
    if let Some(item) = state.autostart_item.lock().unwrap().as_ref() {
        let _ = item.set_checked(!enabled);
    }

    Ok(())
}
