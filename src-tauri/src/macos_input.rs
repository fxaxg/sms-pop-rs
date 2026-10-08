//! macOS input boundary. Never reads AXValue or clipboard contents.
use smspop_core::input_action::{perform, InputBackend};
use std::{
    ffi::c_void,
    ptr,
    time::{Duration, Instant},
};
type Ref = *const c_void;
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXUIElementCreateSystemWide() -> Ref;
    fn AXUIElementCopyAttributeValue(element: Ref, attribute: Ref, value: *mut Ref) -> i32;
    fn AXUIElementIsAttributeSettable(element: Ref, attribute: Ref, settable: *mut u8) -> i32;
    fn AXUIElementGetPid(element: Ref, pid: *mut i32) -> i32;
    fn AXUIElementSetMessagingTimeout(element: Ref, seconds: f32) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(value: Ref);
    fn CFEqual(a: Ref, b: Ref) -> u8;
    fn CFStringCreateWithBytes(
        allocator: Ref,
        bytes: *const u8,
        count: isize,
        encoding: u32,
        external: u8,
    ) -> Ref;
    fn CFBooleanGetTypeID() -> usize;
    fn CFGetTypeID(value: Ref) -> usize;
    fn CFBooleanGetValue(value: Ref) -> u8;
}
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreateKeyboardEvent(source: Ref, key: u16, down: bool) -> Ref;
    fn CGEventKeyboardSetUnicodeString(event: Ref, length: usize, text: *const u16);
    fn CGEventSetFlags(event: Ref, flags: u64);
    fn CGEventPostToPid(pid: i32, event: Ref);
    fn CGEventSourceFlagsState(state: i32) -> u64;
}
struct Owned(Ref);
impl Owned {
    fn new(value: Ref) -> Result<Self, String> {
        if value.is_null() {
            Err("系统接口不可用，请手动复制".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.0);
        }
    }
}
fn string(text: &str) -> Result<Owned, String> {
    // UTF-8, copied by CoreFoundation; no user input is interpreted as a command.
    Owned::new(unsafe {
        CFStringCreateWithBytes(
            ptr::null(),
            text.as_ptr(),
            text.len() as isize,
            0x08000100,
            0,
        )
    })
}
fn attribute(element: Ref, name: &str) -> Result<Owned, String> {
    let name = string(name)?;
    let mut value = ptr::null();
    if unsafe { AXUIElementCopyAttributeValue(element, name.0, &mut value) } != 0 {
        return Err("无法确认当前输入框，请点击输入框重试或手动复制".into());
    }
    Owned::new(value)
}
pub fn is_trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}
pub fn request_access() -> Result<bool, String> {
    if !is_trusted() {
        crate::platform::open_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        )?;
    }
    Ok(is_trusted())
}
struct Target {
    element: Owned,
    pid: i32,
}
impl PartialEq for Target {
    fn eq(&self, other: &Self) -> bool {
        self.pid == other.pid && unsafe { CFEqual(self.element.0, other.element.0) != 0 }
    }
}
fn focus() -> Result<Target, String> {
    let system = Owned::new(unsafe { AXUIElementCreateSystemWide() })?;
    if unsafe { AXUIElementSetMessagingTimeout(system.0, 0.25) } != 0 {
        return Err("辅助功能接口超时设置失败".into());
    }
    let element = attribute(system.0, "AXFocusedUIElement")?;
    let mut pid = 0;
    if unsafe { AXUIElementGetPid(element.0, &mut pid) } != 0
        || pid <= 0
        || pid == std::process::id() as i32
    {
        return Err("请先将焦点放在其他应用的输入框中".into());
    }
    unsafe {
        AXUIElementSetMessagingTimeout(element.0, 0.25);
    }
    let role = attribute(element.0, "AXRole")?;
    let mut editable_role = false;
    for name in ["AXTextField", "AXTextArea", "AXComboBox"] {
        let expected = string(name)?;
        editable_role |= unsafe { CFEqual(role.0, expected.0) != 0 };
    }
    let enabled = attribute(element.0, "AXEnabled")?;
    let enabled = unsafe {
        CFGetTypeID(enabled.0) == CFBooleanGetTypeID() && CFBooleanGetValue(enabled.0) != 0
    };
    let key = string("AXValue")?;
    let mut settable = 0;
    let writable = unsafe {
        AXUIElementIsAttributeSettable(element.0, key.0, &mut settable) == 0 && settable != 0
    };
    if !editable_role || !enabled || !writable {
        return Err("当前控件不可写或不支持辅助功能，请手动复制".into());
    }
    Ok(Target { element, pid })
}
struct Backend<'a> {
    valid: &'a dyn Fn() -> bool,
}
impl InputBackend for Backend<'_> {
    type Target = Target;
    fn trusted(&self) -> bool {
        is_trusted()
    }
    fn target(&self) -> Result<Target, String> {
        focus()
    }
    fn send(&mut self, target: &Target, code: &str) -> Result<(), String> {
        // Wait for shortcut modifiers to be released so they cannot turn digits into commands.
        let deadline = Instant::now() + Duration::from_millis(800);
        let modifiers = (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20);
        while unsafe { CGEventSourceFlagsState(1) } & modifiers != 0 {
            if Instant::now() >= deadline {
                return Err("请松开快捷键后重试".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if !is_trusted() || focus()? != *target || !(self.valid)() {
            return Err("焦点、权限或验证码已变化，请重新操作".into());
        }
        let text: Vec<u16> = code.encode_utf16().collect();
        if text.is_empty() || text.len() > 32 {
            return Err("无有效验证码".into());
        }
        let down = Owned::new(unsafe { CGEventCreateKeyboardEvent(ptr::null(), 0, true) })?;
        let up = Owned::new(unsafe { CGEventCreateKeyboardEvent(ptr::null(), 0, false) })?;
        for event in [&down, &up] {
            unsafe {
                CGEventSetFlags(event.0, 0);
                CGEventKeyboardSetUnicodeString(event.0, text.len(), text.as_ptr());
            }
        }
        // Target the checked process, rather than broadcasting to a newly foregrounded app.
        unsafe {
            CGEventPostToPid(target.pid, down.0);
            CGEventPostToPid(target.pid, up.0);
        }
        Ok(())
    }
}
pub fn insert(code: &str, valid: impl Fn() -> bool) -> Result<(), String> {
    perform(&mut Backend { valid: &valid }, code, &valid)
}
