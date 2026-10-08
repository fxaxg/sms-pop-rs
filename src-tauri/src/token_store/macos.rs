use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};
const SERVICE: &str = "com.smspop.app.lan-receiver";
fn account(reference: &[u8]) -> Result<&str, String> {
    let value = std::str::from_utf8(reference).map_err(|_| "Invalid Keychain reference")?;
    uuid::Uuid::parse_str(value).map_err(|_| "Invalid Keychain reference")?;
    Ok(value)
}
pub fn store(token: &str) -> Result<Vec<u8>, String> {
    let id = uuid::Uuid::new_v4().to_string();
    set_generic_password(SERVICE, &id, token.as_bytes())
        .map_err(|_| "Cannot save receiver token in Keychain")?;
    Ok(id.into_bytes())
}
pub fn load(reference: &[u8]) -> Result<String, String> {
    let bytes = get_generic_password(SERVICE, account(reference)?)
        .map_err(|_| "Cannot access receiver token in Keychain")?;
    String::from_utf8(bytes).map_err(|_| "Invalid receiver credential".into())
}
pub fn remove(reference: &[u8]) -> Result<(), String> {
    delete_generic_password(SERVICE, account(reference)?)
        .map_err(|_| "Cannot remove previous Keychain credential".into())
}
