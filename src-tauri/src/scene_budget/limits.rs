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
/// grok 讀不到模型資訊時的預設（只提醒，不鎖）：1.0.46 各模型的 CLI 本機壓縮點 256000×80%。
const GROK_DEFAULT_CONTEXT: u64 = 204_800;
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
    /// grok 各模型的 CLI 本機壓縮點（`read_grok_windows`；讀不到為 None）
    pub grok_windows: Option<&'a GrokWindows>,
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
            // 沒覆寫就用模型目錄裡標 (default) 的那個（grok models 的輸出）
            let model = cli::tier_override(&config.tier_models, "grok", tier)
                .map(str::to_owned)
                .or_else(|| grok_default_model(sources.catalog));
            let known_window = model
                .as_deref()
                .and_then(|model| sources.grok_windows?.get(model).copied());
            let label = model.unwrap_or_else(|| "(CLI 預設)".to_owned());
            let known = entry(&label);
            Some(match known_window {
                Some(total) => token_limit(&transport, label, total, None, true, known),
                None => token_limit(&transport, label, GROK_DEFAULT_CONTEXT, None, false, known),
            })
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

/// grok 模型 id → CLI 本機自動壓縮點（tokens）。
pub type GrokWindows = BTreeMap<String, u64>;

/// `models_cache.json` 只取需要的欄位；同一筆裡的 api_key 等其餘欄位不進任何結構。
#[derive(serde::Deserialize)]
struct GrokCache {
    models: BTreeMap<String, GrokCacheModel>,
}

#[derive(serde::Deserialize)]
struct GrokCacheModel {
    #[serde(default)]
    info: Option<GrokCacheInfo>,
}

#[derive(serde::Deserialize)]
struct GrokCacheInfo {
    #[serde(default)]
    context_window: Option<u64>,
    #[serde(default)]
    auto_compact_threshold_percent: Option<u64>,
}

/// grok 的上限取 CLI 自己的壓縮點：`context_window × auto_compact_threshold_percent`。
/// 伺服器實際收到 500000（2026-10-07 grok-4.5 實測），但輸入超過壓縮點 CLI 會先在本機
/// 自動壓縮——共線因此丟線、單發可能被改動或延遲——所以以壓縮點為準。
/// 檔案直接串流解析，不整份讀成字串；缺檔、解析失敗一律 None，不記原文。
pub fn read_grok_windows(grok_home: &Path) -> Option<GrokWindows> {
    let file = std::fs::File::open(grok_home.join("models_cache.json")).ok()?;
    let cache: GrokCache = serde_json::from_reader(std::io::BufReader::new(file)).ok()?;
    Some(
        cache
            .models
            .into_iter()
            .filter_map(|(id, model)| {
                let info = model.info?;
                let window = info.context_window?;
                let percent = info.auto_compact_threshold_percent?;
                (window > 0 && percent > 0 && percent <= 100).then(|| (id, window * percent / 100))
            })
            .collect(),
    )
}

/// 模型目錄 grok 那組裡標 ` (default)` 的模型 id（`parse_grok_catalog` 保留原列為 label）。
fn grok_default_model(catalog: &BTreeMap<String, Vec<ModelOption>>) -> Option<String> {
    catalog
        .get("grok")?
        .iter()
        .find(|option| option.label.ends_with(" (default)"))
        .map(|option| option.id.clone())
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
            grok_windows: None,
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
        assert_eq!((grok.total, grok.reliable), (204_800, false));
    }

    /// 手寫假快取：api_key 欄放假值；壓縮點＝context_window×門檻%；
    /// 缺門檻、缺 info、壞檔、缺檔、預設模型解析不到都退回預設只提醒。
    #[test]
    fn grok_uses_cli_compaction_point_from_models_cache() {
        let dir = std::env::temp_dir().join(format!(
            "tt-grok-windows-{}-{}",
            std::process::id(),
            ulid::Ulid::generate()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(read_grok_windows(&dir).is_none());
        std::fs::write(dir.join("models_cache.json"), "{not json").unwrap();
        assert!(read_grok_windows(&dir).is_none());
        std::fs::write(
            dir.join("models_cache.json"),
            r#"{"fetched_at":"x","models":{
                "grok-a":{"info":{"id":"grok-a","context_window":256000,"auto_compact_threshold_percent":80},"api_key":"FAKE-NOT-A-KEY"},
                "grok-b":{"info":{"context_window":256000},"api_key":"FAKE-NOT-A-KEY"},
                "grok-c":{"api_key":"FAKE-NOT-A-KEY"},
                "grok-d":{"info":{"context_window":256000,"auto_compact_threshold_percent":0}}
            }}"#,
        )
        .unwrap();
        let windows = read_grok_windows(&dir).unwrap();
        assert_eq!(windows, GrokWindows::from([("grok-a".to_owned(), 204_800)]));
        std::fs::remove_dir_all(&dir).unwrap();

        let empty = capacity::Store::new();
        let mut catalog = BTreeMap::new();
        let mut found = sources(&empty, &catalog);
        found.grok_windows = Some(&windows);
        // 預設模型解析不到：只提醒
        let limit = resolve(&config("grok"), Tier::Best, &found).unwrap();
        assert_eq!(
            (limit.total, limit.reliable, limit.model.as_str()),
            (204_800, false, "(CLI 預設)")
        );
        catalog.insert(
            "grok".to_owned(),
            ["grok-a (default)", "grok-b"]
                .map(|label| ModelOption {
                    id: label.trim_end_matches(" (default)").to_owned(),
                    label: label.to_owned(),
                    ..Default::default()
                })
                .to_vec(),
        );
        let mut found = sources(&empty, &catalog);
        found.grok_windows = Some(&windows);
        let limit = resolve(&config("grok"), Tier::Best, &found).unwrap();
        assert_eq!(
            (limit.total, limit.reliable, limit.model.as_str()),
            (204_800, true, "grok-a")
        );
        // 檔位覆寫成快取裡沒有門檻的模型：退回預設只提醒
        let mut overridden = config("grok");
        overridden
            .tier_models
            .insert(format!("grok:{}", Tier::Best.as_str()), "grok-b".to_owned());
        let limit = resolve(&overridden, Tier::Best, &found).unwrap();
        assert_eq!(
            (limit.total, limit.reliable, limit.model.as_str()),
            (204_800, false, "grok-b")
        );
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
