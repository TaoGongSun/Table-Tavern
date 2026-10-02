//! `update_check` 的回傳。`none` 是確定沒有新版；`failed` 是這次沒查成，前端保留原本的更新資訊。

use serde::Serialize;

use super::UpdateOffer;

/// 下載或安裝進行中、槽裡卻沒有那份更新時的說明。前端在流程中本來就不採用檢查結果。
pub(crate) const CHECK_BUSY: &str = "更新進行中";

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub(crate) enum CheckResult {
    Available { offer: UpdateOffer },
    None,
    Failed { message: String },
}

impl CheckResult {
    /// 自動檢查的失敗只記 log；兩種都回 `failed`，由前端決定顯不顯示。
    pub(crate) fn failed(manual: bool, message: String) -> Self {
        if !manual {
            log::warn!("自動檢查更新失敗：{message}");
        }
        Self::Failed { message }
    }

    /// 流程進行中回槽裡那一份。槽是空的不能說成「沒有新版」，否則前端會清掉更新資訊。
    pub(crate) fn from_held(offer: Option<UpdateOffer>) -> Self {
        match offer {
            Some(offer) => Self::Available { offer },
            None => Self::Failed {
                message: CHECK_BUSY.to_owned(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::ReminderLevel;
    use serde_json::json;

    fn offer() -> UpdateOffer {
        UpdateOffer {
            version: "0.3.0".to_owned(),
            current_version: "0.2.0".to_owned(),
            notes: None,
            pub_date: None,
            level: ReminderLevel::Feature,
            skipped: false,
        }
    }

    #[test]
    fn serializes_status_with_offer_or_message() {
        let available = serde_json::to_value(CheckResult::Available { offer: offer() }).unwrap();
        assert_eq!(available["status"], "available");
        assert_eq!(available["offer"]["version"], "0.3.0");
        assert_eq!(available["offer"]["level"], "feature");
        assert_eq!(
            serde_json::to_value(CheckResult::None).unwrap(),
            json!({"status": "none"})
        );
        assert_eq!(
            serde_json::to_value(CheckResult::failed(true, "離線".to_owned())).unwrap(),
            json!({"status": "failed", "message": "離線"})
        );
    }

    #[test]
    fn automatic_failures_still_report_failed_with_the_reason() {
        let result =
            serde_json::to_value(CheckResult::failed(false, "rate limit".to_owned())).unwrap();
        assert_eq!(result, json!({"status": "failed", "message": "rate limit"}));
    }

    #[test]
    fn a_busy_slot_without_an_update_is_failed_not_none() {
        assert_eq!(
            serde_json::to_value(CheckResult::from_held(None)).unwrap(),
            json!({"status": "failed", "message": CHECK_BUSY})
        );
        let held = serde_json::to_value(CheckResult::from_held(Some(offer()))).unwrap();
        assert_eq!(held["status"], "available");
    }
}
