// 桌面端入口。移动端不考虑（这个项目只有 Windows）。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    smspop_app_lib::run()
}
