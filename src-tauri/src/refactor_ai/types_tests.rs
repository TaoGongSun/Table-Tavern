use super::{EntryKind, GroupKind};

#[test]
fn entry_kind_parse_rejects_unknown_value() {
    assert!(EntryKind::parse("ghost").is_err());
    assert!(EntryKind::parse("person").is_err());
    assert!(EntryKind::parse("interface").is_ok());
    assert!(EntryKind::parse("interface_shell").is_ok());
}

#[test]
fn group_kind_parse_rejects_unknown_value() {
    assert!(GroupKind::parse("setting").is_ok());
    assert!(GroupKind::parse("mechanism").is_ok());
    assert!(GroupKind::parse("interface").is_err());
}
