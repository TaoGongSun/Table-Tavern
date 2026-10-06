use crate::{config_root, data, transport};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct KeyTier {
    /// "free" | "paid" | "unknown"
    tier: &'static str,
    /// 實際查詢的 base（Rust 正規化後），前端拿來比對快取
    base: String,
}

/// 設定頁判斷 OpenRouter key 是否免費層：key 取草稿，base 一律讀已存檔 config。
#[tauri::command]
pub(crate) async fn openrouter_key_tier(
    app: tauri::AppHandle,
    api_key: String,
) -> Result<KeyTier, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let (base, tier) = transport::key_tier_for(&config, &api_key).await;
    let tier = match tier {
        Some(true) => "free",
        Some(false) => "paid",
        None => "unknown",
    };
    Ok(KeyTier { tier, base })
}
