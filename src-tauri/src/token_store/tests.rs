use super::*;
#[test]
fn failed_persistence_preserves_old_credential() {
    let old = store("old-secret").unwrap();
    assert!(replace(&old, "new-secret", |_| Err("disk full".into())).is_err());
    assert_eq!(load(&old).unwrap(), "old-secret");
}
#[test]
fn successful_rotation_revokes_old_reference() {
    let old = store("old-secret").unwrap();
    let new = replace(&old, "new-secret", |_| Ok(())).unwrap();
    assert_eq!(load(&new).unwrap(), "new-secret");
    assert!(load(&old).is_err());
    remove(&new).unwrap();
}
#[test]
fn unknown_reference_is_fail_closed() {
    assert!(load(b"missing-keychain-account").is_err());
}
