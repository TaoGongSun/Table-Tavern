//! 世界書設定：ST 預設（default/content/settings.json）與載 MVU 的卡用的 MVU 推薦值（web-version D33）。
//! 數值與網頁版 `ST_WI_SETTINGS`／`MVU_WI_SETTINGS` 相同（對拍案例直接帶完整設定）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WiSettings {
    pub depth: f64,
    pub budget_percent: f64,
    pub budget_cap: f64,
    pub recursive: bool,
    pub case_sensitive: bool,
    pub match_whole_words: bool,
    pub use_group_scoring: bool,
    /// 掃描的訊息前面帶「名字: 」（組訊息清單的呼叫端用）
    pub include_names: bool,
}

pub const ST_WI_SETTINGS: WiSettings = WiSettings {
    depth: 2.0,
    budget_percent: 25.0,
    budget_cap: 0.0,
    recursive: true,
    case_sensitive: false,
    match_whole_words: true,
    use_group_scoring: false,
    include_names: true,
};

/// MagVarUpdate 438f9ffc `updateLorebookSettings`：預算 100%、不含名字、不全字比對。
pub const MVU_WI_SETTINGS: WiSettings = WiSettings {
    budget_percent: 100.0,
    match_whole_words: false,
    include_names: false,
    ..ST_WI_SETTINGS
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mvu_settings_follow_d33() {
        assert_eq!(MVU_WI_SETTINGS.budget_percent, 100.0);
        assert!(!MVU_WI_SETTINGS.match_whole_words);
        assert!(!MVU_WI_SETTINGS.include_names);
        assert_eq!(MVU_WI_SETTINGS.depth, ST_WI_SETTINGS.depth);
    }
}
