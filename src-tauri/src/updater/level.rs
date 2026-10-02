//! 提醒等級與略過此版。等級只看遠端 `format_version` 與 SemVer 前三位，不碰執行緒上的格式覆寫。

use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReminderLevel {
    Format,
    Patch,
    Feature,
}

/// 遠端格式大於本版 → `format`。否則最左邊不同的那一位是第三位 → `patch`，其餘 → `feature`。
/// 版本相同、只差預發布，或任一邊不是 SemVer，都算「其餘」。
pub(crate) fn reminder_level(
    current: &str,
    remote: &str,
    remote_format: Option<u64>,
    current_format: u64,
) -> ReminderLevel {
    if remote_format.is_some_and(|format| format > current_format) {
        return ReminderLevel::Format;
    }
    let (Ok(current), Ok(remote)) = (
        semver::Version::parse(current),
        semver::Version::parse(remote),
    ) else {
        return ReminderLevel::Feature;
    };
    if current.major != remote.major || current.minor != remote.minor {
        return ReminderLevel::Feature;
    }
    if current.patch != remote.patch {
        return ReminderLevel::Patch;
    }
    ReminderLevel::Feature
}

/// `raw_json.format_version` 只接受 JSON 整數（含 0）。字串、小數、缺欄都當沒有格式轉換。
pub(crate) fn remote_format(raw: &Value) -> Option<u64> {
    raw.get("format_version").and_then(Value::as_u64)
}

/// 空字串不算略過。偏好缺了、不是字串，或對不上遠端版本，都不是略過。
pub(crate) fn is_skipped(pref: Option<&str>, remote: &str) -> bool {
    matches!(pref, Some(value) if !value.is_empty() && value == remote)
}

/// 缺鍵或不是 bool 都當開。只有明確的 `false` 才關。
pub(crate) fn auto_check_enabled(preferences: &Map<String, Value>) -> bool {
    match preferences.get("update_auto_check") {
        Some(Value::Bool(enabled)) => *enabled,
        _ => true,
    }
}

/// 自動檢查讀不到設定就不要連網。讀得到再看開關。
pub(crate) fn allow_auto_check(preferences: Option<&Map<String, Value>>) -> bool {
    preferences.is_some_and(auto_check_enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn level_follows_format_then_the_leftmost_semver_component() {
        assert_eq!(
            reminder_level("1.2.3", "1.2.4", None, 1),
            ReminderLevel::Patch
        );
        assert_eq!(
            reminder_level("1.2.3", "1.3.0", None, 1),
            ReminderLevel::Feature
        );
        assert_eq!(
            reminder_level("1.2.3", "2.0.0", None, 1),
            ReminderLevel::Feature
        );
        assert_eq!(
            reminder_level("1.2.3", "1.2.3", None, 1),
            ReminderLevel::Feature
        );
        assert_eq!(
            reminder_level("1.2.3", "1.2.3-rc.1", None, 1),
            ReminderLevel::Feature
        );
        assert_eq!(
            reminder_level("1.0.0", "1.0.1", Some(2), 1),
            ReminderLevel::Format
        );
        assert_eq!(
            reminder_level("1.0.0", "1.0.1", Some(1), 1),
            ReminderLevel::Patch
        );
        assert_eq!(
            reminder_level("1.0.0", "1.0.1", Some(0), 1),
            ReminderLevel::Patch
        );
        assert_eq!(
            reminder_level("nope", "1.0.1", None, 1),
            ReminderLevel::Feature
        );
        assert_eq!(
            reminder_level("0.9.0", "0.10.0", None, 1),
            ReminderLevel::Feature
        );
    }

    #[test]
    fn remote_format_accepts_only_json_integers() {
        assert_eq!(remote_format(&json!({"format_version": 2})), Some(2));
        assert_eq!(remote_format(&json!({"format_version": 0})), Some(0));
        assert_eq!(remote_format(&json!({"format_version": 1.5})), None);
        assert_eq!(remote_format(&json!({"format_version": "2"})), None);
        assert_eq!(remote_format(&json!({})), None);
    }

    #[test]
    fn skipped_matches_the_remote_version_only() {
        assert!(is_skipped(Some("1.2.3"), "1.2.3"));
        assert!(!is_skipped(Some("1.2.3"), "1.2.4"));
        assert!(!is_skipped(Some(""), "1.2.3"));
        assert!(!is_skipped(None, "1.2.3"));
    }

    #[test]
    fn auto_check_defaults_on_unless_the_bool_is_false() {
        let mut prefs = Map::new();
        assert!(auto_check_enabled(&prefs));
        assert!(!allow_auto_check(None));
        prefs.insert("update_auto_check".to_owned(), json!(false));
        assert!(!auto_check_enabled(&prefs));
        assert!(!allow_auto_check(Some(&prefs)));
        prefs.insert("update_auto_check".to_owned(), json!("yes"));
        assert!(auto_check_enabled(&prefs));
        prefs.insert("update_auto_check".to_owned(), json!(true));
        assert!(allow_auto_check(Some(&prefs)));
    }
}
