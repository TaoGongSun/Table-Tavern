//! 各後端的容量上限（計畫 §3.4）。上限一律當「總 context」：容量計算保留輸出空間，
//! 不設純輸入上限這一類（Claude 新模型接受請求不等於能完整生成）。agy 是單則輸入 bytes。
use super::capacity::{self, ModelCapacity};
use super::estimate::Unit;
use crate::cli;
use crate::cli::ModelOption;
use crate::data::{AppConfig, Tier};
use std::collections::BTreeMap;
use std::path::Path;

/// agy 單則訊息截尾門檻（實測截在 192–201KB，留餘裕）。
pub const AGY_BODY_BYTES: u64 = 190_000;
/// claude 拿不到回報前的預設總 context（只提醒，不鎖）。
const CLAUDE_DEFAULT_CONTEXT: u64 = 200_000;
/// grok 實測前的保守預設（只提醒，不鎖）。
const GROK_DEFAULT_CONTEXT: u64 = 128_000;
/// codex 快取缺項時的預設（只提醒，不鎖）。
const CODEX_DEFAULT_CONTEXT: u64 = 200_000;
/// 完整生成所需的輸出空間：正文預留＋思考預留。供應商的輸出上限把思考包在內時（Claude），
/// 取 min(輸出上限, 這個值)，不再另加思考。
pub const OUTPUT_RESERVE: u64 = 4_096 + 8_192;

#[derive(Debug, Clone, PartialEq)]
pub struct Limit {
    pub unit: Unit,
    /// 總 context（bytes 時是單則輸入上限）
    pub total: u64,
    /// 輸出空間 R（bytes 時 0：截尾只看輸入）
    pub reserve: u64,
    /// 上限來源可靠（實報、快取、實測常數）才可能鎖；猜的預設值只提醒
    pub reliable: bool,
    /// 送給 CLI／API 的模型字串，查校正用（capacity 的鍵）
    pub model: String,
    pub transport: String,
    /// 校正資料（claude 是實報身分＋各路徑種類的比率）
    pub capacity: Option<ModelCapacity>,
}

impl Limit {
    /// 某路徑種類的倍率：bytes 是精確值不需校正；token 沒有校正就回 None。
    pub fn ratio(&self, kind: &str) -> Option<f64> {
        match self.unit {
            Unit::Bytes => Some(1.0),
            Unit::Tokens => self.capacity.as_ref().and_then(|entry| entry.ratio(kind)),
        }
    }
}

fn token_limit(
    transport: &str,
    model: String,
    total: u64,
    max_output: Option<u64>,
    reliable: bool,
    capacity: Option<ModelCapacity>,
) -> Limit {
    Limit {
        unit: Unit::Tokens,
        total,
        reserve: max_output.map_or(OUTPUT_RESERVE, |cap| cap.min(OUTPUT_RESERVE)),
        reliable,
        model,
        transport: transport.to_owned(),
        capacity,
    }
}

/// 需要讀的外部資料，呼叫端一次備齊（測試可直接造）。
pub struct Sources<'a> {
    pub capacity: &'a capacity::Store,
    /// 設定根的模型目錄快取（API 的 context 取自 OpenRouter 目錄）
    pub catalog: &'a BTreeMap<String, Vec<ModelOption>>,
    /// codex 自己的 models_cache.json 與 config.toml 內容（讀不到為 None）
    pub codex_cache: Option<&'a str>,
    pub codex_config: Option<&'a str>,
    /// 穩定免費目前排得到的候選中最大的 context（None＝沒有候選或沒啟用）
    pub smart_free_context: Option<u64>,
}

/// 某檔位實際會用的模型與上限。None＝拿不到上限，不提醒也不鎖（自訂 base_url）。
pub fn resolve(config: &AppConfig, tier: Tier, sources: &Sources) -> Option<Limit> {
    let transport = crate::transport::dispatch::chat_transport(config);
    let entry = |model: &str| {
        sources
            .capacity
            .get(&capacity::key(&transport, model))
            .cloned()
    };
    match transport.as_str() {
        "claude" => {
            let model = cli::tier_override(&config.tier_models, "claude", tier)
                .unwrap_or_else(|| cli::claude_model_for(tier))
                .to_owned();
            let known = entry(&model);
            Some(
                match known.as_ref().and_then(|known| known.context_window) {
                    Some(total) => {
                        let max_output = known.as_ref().and_then(|known| known.max_output);
                        token_limit(&transport, model, total, max_output, true, known)
                    }
                    None => token_limit(
                        &transport,
                        model,
                        CLAUDE_DEFAULT_CONTEXT,
                        None,
                        false,
                        known,
                    ),
                },
            )
        }
        "codex" => {
            let model = cli::tier_override(&config.tier_models, "codex", tier)
                .map(str::to_owned)
                .or_else(|| sources.codex_config.and_then(codex_default_model));
            let known_window = model.as_deref().and_then(|model| {
                sources
                    .codex_cache
                    .and_then(|cache| codex_window(cache, model))
            });
            // 校正綁實際模型：預設模型也用解析出來的 id 當鍵，改了 config.toml 就換一組校正；
            // 解析不到才落到「(CLI 預設)」，那時上限也不可靠、不會鎖
            let label = model.clone().unwrap_or_else(|| "(CLI 預設)".to_owned());
            let known = entry(&label);
            Some(match known_window {
                Some(total) => token_limit(&transport, label, total, None, true, known),
                None => token_limit(&transport, label, CODEX_DEFAULT_CONTEXT, None, false, known),
            })
        }
        "agy" => {
            let model = cli::tier_override(&config.tier_models, "agy", tier)
                .unwrap_or("(CLI 預設)")
                .to_owned();
            Some(Limit {
                unit: Unit::Bytes,
                total: AGY_BODY_BYTES,
                reserve: 0,
                reliable: true,
                capacity: entry(&model),
                model,
                transport,
            })
        }
        "grok" => {
            let model = cli::tier_override(&config.tier_models, "grok", tier)
                .unwrap_or("(CLI 預設)")
                .to_owned();
            let known = entry(&model);
            Some(token_limit(
                &transport,
                model,
                GROK_DEFAULT_CONTEXT,
                None,
                false,
                known,
            ))
        }
        _ => {
            // API：穩定免費取候選最大 context；OpenRouter 取目錄；自訂 base_url 拿不到
            if crate::smart_free::is_active(config) {
                let total = sources.smart_free_context?;
                return Some(token_limit(
                    &transport,
                    "smart-free".to_owned(),
                    total,
                    Some(crate::smart_free::RESERVED_OUTPUT_TOKENS),
                    false,
                    None,
                ));
            }
            if crate::transport::base_url(config) != crate::transport::DEFAULT_BASE_URL {
                return None;
            }
            let model = crate::transport::resolve_model(tier, config).ok()?;
            let option = sources
                .catalog
                .get("api")?
                .iter()
                .find(|option| option.id == model)?;
            let total = option.context_tokens?;
            Some(token_limit(
                &transport,
                model.clone(),
                total,
                option.max_output_tokens,
                false,
                entry(&model),
            ))
        }
    }
}

/// codex `config.toml` 頂層的 `model = "..."`（只認最簡單的寫法；認不出來＝未解析，不猜）。
fn codex_default_model(config: &str) -> Option<String> {
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            break; // 進了別的 table，頂層結束
        }
        if let Some(rest) = line.strip_prefix("model") {
            let rest = rest.trim_start();
            if let Some(value) = rest.strip_prefix('=') {
                let value = value.trim().trim_matches('"').trim();
                if !value.is_empty() {
                    return Some(value.to_owned());
                }
            }
        }
    }
    None
}

/// codex `models_cache.json` 中該 slug 的 `context_window × effective_context_window_percent / 100`。
/// 缺欄位、壞檔、找不到 slug 一律 None。
fn codex_window(cache: &str, model: &str) -> Option<u64> {
    let value: serde_json::Value = serde_json::from_str(cache).ok()?;
    let entry = value
        .get("models")?
        .as_array()?
        .iter()
        .find(|entry| entry.get("slug").and_then(|slug| slug.as_str()) == Some(model))?;
    let window = entry.get("context_window")?.as_u64()?;
    let percent = entry.get("effective_context_window_percent")?.as_u64()?;
    (percent > 0 && percent <= 100).then(|| window * percent / 100)
}

/// 讀 codex 的兩個檔（app 跑 codex 沿用使用者自己的 `~/.codex`，沒有另設 CODEX_HOME）。
pub fn read_codex_files() -> (Option<String>, Option<String>) {
    let home = std::env::var_os("CODEX_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".codex")));
    match home {
        Some(home) => (
            std::fs::read_to_string(home.join("models_cache.json")).ok(),
            std::fs::read_to_string(home.join("config.toml")).ok(),
        ),
        None => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::AppConfig;

    fn config(transport: &str) -> AppConfig {
        let mut config = AppConfig::default();
        config
            .preferences
            .insert("transport".into(), serde_json::json!(transport));
        config
    }

    fn sources<'a>(
        capacity: &'a capacity::Store,
        catalog: &'a BTreeMap<String, Vec<ModelOption>>,
    ) -> Sources<'a> {
        Sources {
            capacity,
            catalog,
            codex_cache: None,
            codex_config: None,
            smart_free_context: None,
        }
    }

    #[test]
    fn claude_uses_reported_window_else_unreliable_default() {
        let empty = capacity::Store::new();
        let catalog = BTreeMap::new();
        let limit = resolve(&config("claude"), Tier::Fast, &sources(&empty, &catalog)).unwrap();
        assert_eq!(
            (limit.total, limit.reliable, limit.model.as_str()),
            (200_000, false, "haiku")
        );
        assert_eq!(limit.reserve, OUTPUT_RESERVE);

        let mut store = capacity::Store::new();
        store.insert(
            capacity::key("claude", "haiku"),
            ModelCapacity {
                model_id: Some("claude-haiku-4-5".into()),
                context_window: Some(200_000),
                max_output: Some(8_000),
                ..Default::default()
            },
        );
        let limit = resolve(&config("claude"), Tier::Fast, &sources(&store, &catalog)).unwrap();
        assert!(limit.reliable);
        // 輸出上限把思考包在內：取小，不重加
        assert_eq!(limit.reserve, 8_000);
        assert_eq!(limit.ratio("summary"), None);
    }

    #[test]
    fn agy_is_exact_bytes_and_grok_is_hint_only() {
        let empty = capacity::Store::new();
        let catalog = BTreeMap::new();
        let agy = resolve(&config("agy"), Tier::Best, &sources(&empty, &catalog)).unwrap();
        assert_eq!(
            (agy.unit, agy.total, agy.reserve, agy.reliable),
            (Unit::Bytes, 190_000, 0, true)
        );
        assert_eq!(agy.ratio("summary"), Some(1.0));
        let grok = resolve(&config("grok"), Tier::Best, &sources(&empty, &catalog)).unwrap();
        assert!(!grok.reliable);
    }

    #[test]
    fn codex_needs_matching_slug_and_resolved_default_model() {
        let empty = capacity::Store::new();
        let catalog = BTreeMap::new();
        let cache = r#"{"models":[{"slug":"gpt-x","context_window":272000,"effective_context_window_percent":95}]}"#;
        let mut found = sources(&empty, &catalog);
        found.codex_cache = Some(cache);
        found.codex_config = Some("model = \"gpt-x\"\nmodel_reasoning_effort = \"medium\"\n");
        let limit = resolve(&config("codex"), Tier::Best, &found).unwrap();
        assert_eq!((limit.total, limit.reliable), (258_400, true));
        // 校正鍵綁解析出來的實際模型，不共用「(CLI 預設)」
        assert_eq!(limit.model, "gpt-x");
        // 預設模型解析不到、slug 對不上、壞檔：未知，不可鎖
        for (cache, conf) in [
            (cache, "[profiles]\nmodel = \"gpt-x\"\n"),
            (cache, "model = \"gpt-other\"\n"),
            ("{broken", "model = \"gpt-x\"\n"),
            (
                r#"{"models":[{"slug":"gpt-x","context_window":272000}]}"#,
                "model = \"gpt-x\"\n",
            ),
        ] {
            let mut partial = sources(&empty, &catalog);
            partial.codex_cache = Some(cache);
            partial.codex_config = Some(conf);
            let limit = resolve(&config("codex"), Tier::Best, &partial).unwrap();
            assert!(!limit.reliable, "{cache} / {conf}");
        }
    }

    #[test]
    fn api_reads_openrouter_catalog_and_custom_base_url_has_no_limit() {
        let empty = capacity::Store::new();
        let mut catalog = BTreeMap::new();
        catalog.insert(
            "api".to_owned(),
            vec![ModelOption {
                id: "vendor/model".into(),
                label: "Model".into(),
                context_tokens: Some(131_072),
                max_output_tokens: Some(4_000),
            }],
        );
        let mut api = config("api");
        api.tier_models.insert("best".into(), "vendor/model".into());
        let limit = resolve(&api, Tier::Best, &sources(&empty, &catalog)).unwrap();
        assert_eq!(
            (limit.total, limit.reserve, limit.reliable),
            (131_072, 4_000, false)
        );
        api.preferences.insert(
            "base_url".into(),
            serde_json::json!("http://localhost:1234/v1"),
        );
        assert_eq!(resolve(&api, Tier::Best, &sources(&empty, &catalog)), None);
    }
}
