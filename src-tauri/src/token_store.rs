//! OS credential storage. Config contains an opaque reference, never a plaintext token.
#[cfg(all(target_os = "macos", not(test)))]
mod macos;
#[cfg(all(windows, not(test)))]
mod windows;
#[cfg(all(target_os = "macos", not(test)))]
pub use macos::{load, remove, store};
#[cfg(all(windows, not(test)))]
pub fn store(token: &str) -> Result<Vec<u8>, String> {
    windows::protect(token.as_bytes(), false)
}
#[cfg(all(windows, not(test)))]
pub fn load(reference: &[u8]) -> Result<String, String> {
    String::from_utf8(windows::protect(reference, true)?).map_err(|_| "Invalid credential".into())
}
#[cfg(all(windows, not(test)))]
pub fn remove(_: &[u8]) -> Result<(), String> {
    Ok(())
}

pub fn replace(
    old: &[u8],
    token: &str,
    persist: impl FnOnce(&[u8]) -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    let next = store(token)?;
    if let Err(e) = persist(&next) {
        let _ = remove(&next);
        return Err(e);
    }
    if !old.is_empty() {
        let _ = remove(old);
    }
    Ok(next)
}

#[cfg(test)]
mod memory {
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicU64, Ordering},
            Mutex, OnceLock,
        },
    };
    static ITEMS: OnceLock<Mutex<HashMap<Vec<u8>, String>>> = OnceLock::new();
    static SEQ: AtomicU64 = AtomicU64::new(0);
    fn items() -> &'static Mutex<HashMap<Vec<u8>, String>> {
        ITEMS.get_or_init(Default::default)
    }
    pub fn store(token: &str) -> Result<Vec<u8>, String> {
        let id = SEQ.fetch_add(1, Ordering::SeqCst).to_le_bytes().to_vec();
        items().lock().unwrap().insert(id.clone(), token.into());
        Ok(id)
    }
    pub fn load(reference: &[u8]) -> Result<String, String> {
        items()
            .lock()
            .unwrap()
            .get(reference)
            .cloned()
            .ok_or("Credential unavailable".into())
    }
    pub fn remove(reference: &[u8]) -> Result<(), String> {
        items().lock().unwrap().remove(reference);
        Ok(())
    }
}
#[cfg(test)]
pub use memory::{load, remove, store};
#[cfg(test)]
#[path = "token_store/tests.rs"]
mod tests;
