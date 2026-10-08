pub fn open_url(url: &str) -> Result<(), String> {
    std::process::Command::new("/usr/bin/open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
pub fn open_path(path: &std::path::Path) -> Result<(), String> {
    std::process::Command::new("/usr/bin/open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
pub fn open_bluetooth() -> Result<(), String> {
    open_url("x-apple.systempreferences:com.apple.BluetoothSettings")
}
pub fn open_firewall() -> Result<(), String> {
    open_url("x-apple.systempreferences:com.apple.Network-Settings.extension?Firewall")
}
