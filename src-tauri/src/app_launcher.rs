use smspop_core::app_rules::AppRule;
use windows::core::PCWSTR;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub fn open(rule: &AppRule) -> Result<(), String> {
    rule.validate()?;
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    let target: Vec<u16> = rule.target.encode_utf16().chain(Some(0)).collect();
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
