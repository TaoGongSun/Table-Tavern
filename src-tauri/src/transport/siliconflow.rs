//! 中國供應商（矽基流動）：走既有 OpenAI 相容通道，`base_url` 指到這裡就算選了它。
//! 金鑰另存 `api_keys.siliconflow`（切回 OpenRouter 不丟金鑰）；檔位模型存 `siliconflow:<檔位>`，
//! 沒設就用免費清單第一支。只挑免費模型、失敗不換付費：智慧免費只在 OpenRouter 預設站台啟動，這裡不接。

use crate::cli::ModelOption;
use crate::data::{AppConfig, Tier};

pub const BASE_URL: &str = "https://api.siliconflow.cn/v1";

/// 免費對話模型的寫死排序〔作者裁決 2026-11-02〕。來源：官方價格頁 SKU 的 realTimePriceCnyUnit
/// 輸入輸出皆 0（2026-11-02）；排序＝官方長期主推的 Qwen3-8B 先，其餘依參數量與上下文，推理模型殿後。
pub const FREE_MODELS: &[&str] = &[
    "Qwen/Qwen3-8B",
    "XingChenAGI/Xing4.0-29B",
    "Qwen/Qwen3.5-4B",
    "THUDM/GLM-4-9B-0414",
    "Qwen/Qwen2.5-7B-Instruct",
    "deepseek-ai/DeepSeek-R1-0528-Qwen3-8B",
    "THUDM/GLM-Z1-9B-0414",
];

pub fn active(config: &AppConfig) -> bool {
    super::base_url(config) == BASE_URL
}

/// 這條 API 路線實際要帶的金鑰：矽基流動用自己的槽，其餘照舊用 OpenRouter 槽。
pub(crate) fn api_key(config: &AppConfig) -> Option<&String> {
    let slot = if active(config) {
        "siliconflow"
    } else {
        "openrouter"
    };
    config.api_keys.get(slot).filter(|key| !key.is_empty())
}

pub(crate) fn tier_model(config: &AppConfig, tier: Tier) -> String {
    crate::cli::tier_override(&config.tier_models, "siliconflow", tier)
        .unwrap_or(FREE_MODELS[0])
        .to_owned()
}

pub fn model_options() -> Vec<ModelOption> {
    FREE_MODELS
        .iter()
        .map(|id| ModelOption {
            id: (*id).to_owned(),
            label: (*id).to_owned(),
            ..ModelOption::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 選了矽基流動：檔位沒設就用免費清單第一支、金鑰取自己的槽、強制 Chat Completions、
    /// 智慧免費不啟動；房間切路線在兩條 API 之間來回時 base_url 跟著換
    #[test]
    fn siliconflow_route_resolves_free_default_and_own_key() {
        let mut config = AppConfig::default();
        config
            .preferences
            .insert("base_url".into(), json!(BASE_URL));
        config
            .preferences
            .insert("api_mode".into(), json!("responses"));
        config
            .preferences
            .insert("api_model_mode".into(), json!("stable_free"));
        config.api_keys.insert("openrouter".into(), "sk-or".into());
        config.api_keys.insert("siliconflow".into(), "sk-sf".into());
        config
            .tier_models
            .insert("best".into(), "anthropic/x".into());
        assert_eq!(
            crate::transport::resolve_model(Tier::Best, &config).unwrap(),
            FREE_MODELS[0]
        );
        config
            .tier_models
            .insert("siliconflow:best".into(), "Qwen/Qwen3.5-4B".into());
        assert_eq!(
            crate::transport::resolve_model(Tier::Best, &config).unwrap(),
            "Qwen/Qwen3.5-4B"
        );
        assert_eq!(api_key(&config).map(String::as_str), Some("sk-sf"));
        assert_eq!(
            crate::transport::responses::api_mode(&config),
            crate::transport::responses::ApiMode::ChatCompletions
        );
        assert!(!crate::smart_free::is_active(&config));

        crate::data::apply_play_override(&mut config, "api", "vendor/m");
        assert!(!active(&config));
        assert_eq!(api_key(&config).map(String::as_str), Some("sk-or"));
        assert_eq!(
            crate::transport::resolve_model(Tier::Best, &config).unwrap(),
            "vendor/m"
        );
        crate::data::apply_play_override(&mut config, "siliconflow", "THUDM/GLM-4-9B-0414");
        assert_eq!(config.preferences["transport"], "api");
        assert_eq!(
            crate::transport::resolve_model(Tier::Best, &config).unwrap(),
            "THUDM/GLM-4-9B-0414"
        );
    }

    /// OpenRouter 金鑰等級查詢不跟著 base_url 跑到矽基流動（金鑰不外送）
    #[tokio::test]
    async fn openrouter_key_lookup_never_targets_siliconflow() {
        let mut config = AppConfig::default();
        config
            .preferences
            .insert("base_url".into(), json!(BASE_URL));
        let (base, tier) = crate::transport::key_tier_for(&config, "").await;
        assert_eq!(
            (base.as_str(), tier),
            (crate::transport::DEFAULT_BASE_URL, None)
        );
    }
}
