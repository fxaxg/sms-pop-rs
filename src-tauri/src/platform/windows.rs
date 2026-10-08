use windows::core::PCWSTR;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
pub fn open_url(url: &str) -> Result<(), String> {
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    let target: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
    // No shell command interpreter and no arguments derived from notification content.
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(target.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        return Err(format!("打开应用失败（系统错误 {}）", result.0 as isize));
    }
    Ok(())
}
pub fn open_path(path: &std::path::Path) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
pub fn open_bluetooth() -> Result<(), String> {
    open_url("ms-settings:bluetooth")
}
pub fn open_firewall() -> Result<(), String> {
    std::process::Command::new("mmc.exe")
        .arg("wf.msc")
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
