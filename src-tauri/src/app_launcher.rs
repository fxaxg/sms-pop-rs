use smspop_core::app_rules::AppRule;
pub fn open(rule: &AppRule) -> Result<(), String> {
    rule.validate()?;
    crate::platform::open_url(&rule.target)
}
