//! 動態穩定免費：用玩家 OpenRouter OAuth key 取得帳號可用免費模型，依角色扮演排行挑當下穩定首選，
//! 每次送出只帶這一支（不送備援陣列），品質不穩的免費模型才不會在對話中途被靜默換掉。

mod api;
mod select;
mod store;

use crate::data::AppConfig;
use crate::transport::{base_url, ChatMessage, DEFAULT_BASE_URL};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Emitter;

pub const MODE_KEY: &str = "api_model_mode";
/// 第 3 階段的新預設：每次送出依最新 RP 穩定榜重建主模型與備援。
pub const MODE_STABLE: &str = "stable_free";
/// 舊第 2 階段值，只保留作升級相容；讀到時會遷移成 stable_free。
pub const MODE_SMART_LEGACY: &str = "smart_free";
/// 玩家點推薦模型後固定使用該支（三 tier 同一支），前端存的模式值。
pub const MODE_RECOMMENDED: &str = "recommended";
/// §15 新限免提示的整體開關；預設開，只有明確存 false 才關閉。
pub const NOTIFY_KEY: &str = "smart_free_notify";

const TTL_SECS: u64 = 12 * 3600;
const RECHECK_EVERY: Duration = Duration::from_secs(3600);
const EXPIRY_NOTICE_WINDOW_SECS: u64 = 72 * 3600;
const DISPLAYABLE_EXPIRY_WINDOW_SECS: u64 = 90 * 24 * 3600;
const RECENT_MODEL_WINDOW_SECS: u64 = 7 * 24 * 3600;
const LONG_CONTEXT_TOKENS: u64 = 128_000;

static PINS_LOCK: Mutex<()> = Mutex::new(());
static EXPIRY_NOTICE_LOCK: Mutex<()> = Mutex::new(());
static REFRESH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub fn is_active(config: &AppConfig) -> bool {
    matches!(
        config
            .preferences
            .get(MODE_KEY)
            .and_then(|value| value.as_str()),
        Some(MODE_STABLE | MODE_SMART_LEGACY)
    ) && base_url(config) == DEFAULT_BASE_URL
}

/// 舊 smart_free 玩家升級後直接進入新的動態穩定免費預設；手動與明確推薦選擇完全不動。
pub fn migrate_legacy_mode(config: &mut AppConfig) -> bool {
    if config
        .preferences
        .get(MODE_KEY)
        .and_then(|value| value.as_str())
        != Some(MODE_SMART_LEGACY)
    {
        return false;
    }
    config
        .preferences
        .insert(MODE_KEY.to_owned(), MODE_STABLE.into());
    true
}

fn api_key(config: &AppConfig) -> Option<&str> {
    config
        .api_keys
        .get("openrouter")
        .map(String::as_str)
        .map(str::trim)
        .filter(|key| !key.is_empty())
}

fn fingerprint(key: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn required_context(messages: &[ChatMessage]) -> u64 {
    let input = messages.iter().fold(2u64, |tokens, message| {
        tokens
            .saturating_add(crate::usage_log::estimate_tokens(&message.content))
            .saturating_add(4)
    });
    input.saturating_add(select::RESERVED_OUTPUT_TOKENS)
}

/// 每次真正送出前走這裡：先確認免費日額度，再刷新必要資料，最後依當下 prompt 挑穩定首選一支送出。
pub struct PreparedCall {
    pub models: Vec<String>,
    pub expiry_warning: Option<ExpiryWarning>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExpiryWarning {
    pub model: String,
    pub expires_at: u64,
}

pub async fn prepare_call(
    root: &Path,
    config: &AppConfig,
    world: Option<&str>,
    messages: &[ChatMessage],
) -> Result<PreparedCall, String> {
    let key =
        api_key(config).ok_or_else(|| "尚未設定 OpenRouter API key，請先完成連線".to_owned())?;
    if let Some(client) = api::client() {
        if api::free_daily_remaining(&client, key)
            .await
            .is_some_and(|remaining| remaining <= 0)
        {
            return Err(
                "AI_HTTP_STATUS_429: OpenRouter 免費模型今日可用次數已用完，請等額度重置後再試"
                    .to_owned(),
            );
        }
    }

    refresh_if_stale(root, config).await;
    let cache = store::read_cache(root);
    let account = fingerprint(key);
    let catalog = cache.catalog_for(&account).unwrap_or(&[]);
    let now = now_secs();
    let eligible = select::eligible_models(catalog, required_context(messages), now, &[]);
    if eligible.is_empty() {
        return Err("目前沒有可用免費模型".to_owned());
    }
    // OpenRouter 免費 RP 模型品質不穩，靜默備援會讓句子中途換模、風格突變，玩家又不一定換得回來；
    // 現狀下「不換模型」較好〔作者裁決 2026-09-18〕。因此只送當下穩定首選一支、不送備援陣列：
    // 該支短暫失敗就回報這次請求失敗，由玩家再送一次（cache 未變、下次仍是同一支，不會彈跳）。
    // stable_fallback_models 仍照算保留備援名次，供日後「連續失敗才問玩家換備用」的視窗接手。
    let pins = store::read_pins(root);
    let previous = world.and_then(|world| pins.get(world)).map(String::as_str);
    let expiry_warning = take_expiry_warning(root, world, previous, catalog, now);
    let ranked = select::stable_fallback_models(
        &eligible,
        cache.roleplay_slugs.as_deref().unwrap_or(&[]),
        cache.weekly_ids.as_deref().unwrap_or(&[]),
        now,
    );
    match ranked.into_iter().next() {
        Some(primary) => Ok(PreparedCall {
            models: vec![primary],
            expiry_warning,
        }),
        None => Err("目前沒有可用的穩定免費模型，請到設定改用其他免費模型或手動選模".to_owned()),
    }
}

/// 已知到期的綁定模型在到期前三天提醒一次；marker 含到期時間，若官方延長後再縮短仍可重新提醒。
fn take_expiry_warning(
    root: &Path,
    world: Option<&str>,
    pinned: Option<&str>,
    catalog: &[select::FreeModel],
    now: u64,
) -> Option<ExpiryWarning> {
    let world = world?;
    let pinned = pinned?;
    let model = catalog.iter().find(|model| model.id == pinned)?;
    let expires_at = model.expiration_at?;
    if expires_at <= now || expires_at > now.saturating_add(EXPIRY_NOTICE_WINDOW_SECS) {
        return None;
    }

    let marker = format!("{}@{expires_at}", model.id);
    let _guard = EXPIRY_NOTICE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut notices = store::read_expiry_notices(root);
    if notices
        .get(world)
        .is_some_and(|previous| previous == &marker)
    {
        return None;
    }
    notices.insert(world.to_owned(), marker);
    store::write_expiry_notices(root, &notices).ok()?;
    Some(ExpiryWarning {
        model: model.name.clone(),
        expires_at,
    })
}

/// OpenRouter 回傳的 top-level model 才是真正回答者。新桌首次綁定不算「換手」；
/// 已有綁定而回答者不同時，成功落盤後回 true 給 UI 告知玩家。
pub fn record_responder(root: &Path, world: Option<&str>, model: &str) -> bool {
    let Some(world) = world else {
        return false;
    };
    let _guard = PINS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut pins = store::read_pins(root);
    let switched = pins.get(world).is_some_and(|previous| previous != model);
    if pins.get(world).is_some_and(|previous| previous == model) {
        return false;
    }
    pins.insert(world.to_owned(), model.to_owned());
    store::write_pins(root, &pins).is_ok() && switched
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub model: String,
    /// `:free` 模型才有每日額度；stealth/促銷或查不到時為 None，UI 顯示「無限」。
    pub free_daily: Option<api::FreeDaily>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationList {
    pub limited: Vec<Recommendation>,
    /// 穩定免費前兩名：第一名是自動模式送出的那支，第二名供玩家在第一名失效時手動改用。
    pub stable: Vec<Recommendation>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub model: String,
    pub label: String,
    pub expires_at: Option<u64>,
    pub show_expiry_date: bool,
    pub expiring_soon: bool,
    pub reason: String,
    pub provider: String,
    pub recent: bool,
    pub context_length: u64,
    pub long_context: bool,
}

fn recommendation(
    model: &select::FreeModel,
    now: u64,
    stable: Option<select::StableSource>,
) -> Recommendation {
    let expires_at = model.expiration_at;
    let provider = if model.id.starts_with("stealth/") {
        String::new()
    } else {
        select::model_namespace(model).to_owned()
    };
    let lower = format!("{} {}", model.id, model.name).to_ascii_lowercase();
    let test_named = ["preview", "experimental", "experiment", "beta", "alpha"]
        .iter()
        .any(|marker| lower.contains(marker));
    let reason = match stable {
        // 穩定保底照排名來源標理由；退回七日榜時如實標 weekly，不誤稱角色扮演熱門。
        Some(select::StableSource::Roleplay) => "stable_roleplay",
        Some(select::StableSource::Weekly) => "stable_weekly",
        Some(select::StableSource::Available) => "stable_available",
        None if model.id.starts_with("stealth/") => "anonymous_test",
        None if test_named && !provider.is_empty() => "provider_test",
        None if test_named => "limited_test",
        None if !provider.is_empty() => "provider_limited",
        None => "limited",
    }
    .to_owned();
    Recommendation {
        model: model.id.clone(),
        label: model.name.clone(),
        expires_at,
        show_expiry_date: expires_at.is_some_and(|expires| {
            expires > now && expires <= now.saturating_add(DISPLAYABLE_EXPIRY_WINDOW_SECS)
        }),
        expiring_soon: expires_at.is_some_and(|expires| {
            expires > now && expires <= now.saturating_add(EXPIRY_NOTICE_WINDOW_SECS)
        }),
        reason,
        provider,
        recent: model.created > 0
            && model.created <= now
            && now.saturating_sub(model.created) <= RECENT_MODEL_WINDOW_SECS,
        context_length: model.context_length,
        long_context: model.context_length >= LONG_CONTEXT_TOKENS,
    }
}

/// 設定頁推薦清單只讀既有快取，不觸發任何 OpenRouter 連線。
pub fn recommendations(root: &Path, config: &AppConfig) -> RecommendationList {
    let Some(key) = api_key(config) else {
        return RecommendationList::default();
    };
    let cache = store::read_cache(root);
    let Some(catalog) = cache.catalog_for(&fingerprint(key)) else {
        return RecommendationList::default();
    };
    let now = now_secs();
    let (limited, stable) = select::recommendation_models(
        catalog,
        cache.roleplay_slugs.as_deref().unwrap_or(&[]),
        cache.weekly_ids.as_deref().unwrap_or(&[]),
        select::RESERVED_OUTPUT_TOKENS,
        now,
    );
    RecommendationList {
        limited: limited
            .into_iter()
            .map(|model| recommendation(model, now, None))
            .collect(),
        stable: stable
            .into_iter()
            .map(|(model, source)| recommendation(model, now, Some(source)))
            .collect(),
    }
}

fn notifications_enabled(config: &AppConfig) -> bool {
    // 預設開；只有明確存 false 才關閉。
    config
        .preferences
        .get(NOTIFY_KEY)
        .and_then(serde_json::Value::as_bool)
        != Some(false)
}

/// §15：只讀既有快取，回傳「上次看過之後才出現」的限時／實驗性推薦，供非阻塞 banner 用。
/// 穩定免費保底屬固定保底、不主動提醒。整體開關關閉或尚無金鑰時回空。
pub fn new_recommendations(root: &Path, config: &AppConfig) -> Vec<Recommendation> {
    if !notifications_enabled(config) {
        return Vec::new();
    }
    let Some(key) = api_key(config) else {
        return Vec::new();
    };
    let account = fingerprint(key);
    // 還沒抓到任何帳號快取時不動基準：等真有資料再建，避免把「暫時的空」當永久基準。
    if store::read_cache(root).catalog_for(&account).is_none() {
        return Vec::new();
    }
    let live = recommendations(root, config).limited;
    let seen = store::read_seen(root);
    if seen.account_fingerprint != account {
        // 首次啟動（或換帳號）尚無比較基準：靜默把當下推薦建為基準，不對既有清單跳提示；
        // 清單為空也要建基準（否則第一支真正新出現的限時模型會被誤當首次基準吃掉），
        // 之後才真正「進入推薦清單」的限時模型才提醒。既有模型玩家仍能在設定頁看到。
        let baseline: Vec<String> = live.iter().map(|item| item.model.clone()).collect();
        mark_recommendations_seen(root, config, &baseline);
        return Vec::new();
    }
    live.into_iter()
        .filter(|item| !seen.ids.iter().any(|id| id == &item.model))
        .collect()
}

/// 玩家對 banner 按了改用／查看／略過任一個都算「看過」，寫進本機基準，之後不再重複提醒。
pub fn mark_recommendations_seen(root: &Path, config: &AppConfig, ids: &[String]) {
    let Some(key) = api_key(config) else {
        return;
    };
    let account = fingerprint(key);
    let mut seen = store::read_seen(root);
    if seen.account_fingerprint != account {
        seen.account_fingerprint = account;
        seen.ids.clear();
    }
    for id in ids {
        if !seen.ids.iter().any(|existing| existing == id) {
            seen.ids.push(id.clone());
        }
    }
    let _ = store::write_seen(root, &seen);
}

pub fn status(root: &Path, config: &AppConfig) -> Status {
    let Some(key) = api_key(config) else {
        return Status::default();
    };
    let cache = store::read_cache(root);
    let Some(catalog) = cache.catalog_for(&fingerprint(key)) else {
        return Status::default();
    };
    let now = now_secs();
    let eligible = select::eligible_models(catalog, select::RESERVED_OUTPUT_TOKENS, now, &[]);
    let ranked = select::stable_fallback_models(
        &eligible,
        cache.roleplay_slugs.as_deref().unwrap_or(&[]),
        cache.weekly_ids.as_deref().unwrap_or(&[]),
        now,
    );
    let Some(primary_id) = ranked.first() else {
        return Status::default();
    };
    let Some(model) = eligible.iter().find(|model| &model.id == primary_id) else {
        return Status::default();
    };
    Status {
        model: model.id.clone(),
        free_daily: None,
    }
}

/// 設定頁用：在 status() 之上補上每日免費額度。
/// `:free` 模型會計入帳號的每日免費池（`/key` 的 free_model_daily_requests，有回報延遲）；
/// stealth／促銷等非 `:free` 模型不計入，視為無限（free_daily = None，UI 顯示「無限」）。
pub async fn status_with_quota(root: &Path, config: &AppConfig) -> Status {
    let mut status = status(root, config);
    // 每日免費額度是全帳號共用池，動態穩定免費與推薦模式都要顯示；
    // 推薦模式看玩家固定的那支（三 tier 同一支），動態穩定免費看下一輪的 RP 首選。
    let current = if config
        .preferences
        .get(MODE_KEY)
        .and_then(|value| value.as_str())
        == Some(MODE_RECOMMENDED)
    {
        config
            .tier_models
            .get("best")
            .cloned()
            .unwrap_or_else(|| status.model.clone())
    } else {
        status.model.clone()
    };
    if current.ends_with(":free") {
        if let Some(key) = api_key(config) {
            if let Some(client) = api::client() {
                status.free_daily = api::fetch_free_daily(&client, key).await;
            }
        }
    }
    status
}

pub fn models_in_use(root: &Path, config: &AppConfig) -> Vec<String> {
    let mut models: Vec<String> = store::read_pins(root).into_values().collect();
    let current = status(root, config).model;
    if !current.is_empty() {
        models.push(current);
    }
    models.sort();
    models.dedup();
    models
}

/// 帳號清單、七天榜與角色扮演排行各自更新；任何抓取或解析失敗都保留上一份好資料。
/// 回傳是否寫入了新快取，供呼叫端通知前端重查推薦與 §15 提示。
pub async fn refresh_if_stale(root: &Path, config: &AppConfig) -> bool {
    let Some(key) = api_key(config) else {
        return false;
    };
    let _guard = REFRESH_LOCK.lock().await;
    let now = now_secs();
    let account = fingerprint(key);
    let mut cache = store::read_cache(root);
    let Some(client) = api::client() else {
        return false;
    };
    let user_stale = cache.account_fingerprint != account
        || cache.user_catalog.as_ref().is_none_or(Vec::is_empty)
        || cache
            .user_catalog
            .as_ref()
            .is_some_and(|models| models.iter().any(|model| model.canonical_slug.is_empty()))
        || now.saturating_sub(cache.user_catalog_fetched_at) >= TTL_SECS;
    let weekly_stale =
        cache.weekly_ids.is_none() || now.saturating_sub(cache.weekly_fetched_at) >= TTL_SECS;
    let roleplay_stale =
        cache.roleplay_slugs.is_none() || now.saturating_sub(cache.roleplay_fetched_at) >= TTL_SECS;
    let mut changed = false;

    if user_stale {
        if let Some(catalog) = api::fetch_user_catalog(&client, key).await {
            cache.account_fingerprint = account;
            cache.user_catalog = Some(catalog);
            cache.user_catalog_fetched_at = now;
            changed = true;
        }
    }
    if weekly_stale {
        if let Some(ids) = api::fetch_weekly_ids(&client).await {
            cache.weekly_ids = Some(ids);
            cache.weekly_fetched_at = now;
            changed = true;
        }
    }
    if roleplay_stale {
        if let Some(slugs) = api::fetch_roleplay_slugs(&client).await {
            cache.roleplay_slugs = Some(slugs);
            cache.roleplay_fetched_at = now;
            changed = true;
        }
    }
    if changed {
        let _ = store::write_cache(root, &cache);
    }
    changed
}

/// 背景刷新換到新快取後廣播，前端據此重查推薦與 §15 新限免提示（設定頁與 banner 都聽這個）。
pub const CACHE_UPDATED_EVENT: &str = "smart-free-cache-updated";

/// 在 OpenRouter 官方站台走 API 直連且有金鑰＝推薦清單與 §15 新限免提示都需要新鮮資料，
/// 不限智慧免費模式：手動／推薦模式玩家一樣要看得到推薦與被提醒。實際選模仍由 is_active 控。
fn tracks_openrouter(config: &AppConfig) -> bool {
    config
        .preferences
        .get("transport")
        .and_then(|value| value.as_str())
        .unwrap_or("api")
        == "api"
        && base_url(config) == DEFAULT_BASE_URL
        && api_key(config).is_some()
}

pub fn warm(app: &tauri::AppHandle, root: &Path, config: &AppConfig) {
    if tracks_openrouter(config) {
        let app = app.clone();
        let root = root.to_owned();
        let config = config.clone();
        tauri::async_runtime::spawn(async move {
            if refresh_if_stale(&root, &config).await {
                let _ = app.emit(CACHE_UPDATED_EVENT, ());
            }
        });
    }
}

pub fn spawn_background_refresh(app: tauri::AppHandle, config_root: std::path::PathBuf) {
    tauri::async_runtime::spawn(async move {
        loop {
            if let Ok(config) = crate::data::read_config(&config_root) {
                if tracks_openrouter(&config) && refresh_if_stale(&config_root, &config).await {
                    let _ = app.emit(CACHE_UPDATED_EVENT, ());
                }
            }
            tokio::time::sleep(RECHECK_EVERY).await;
        }
    });
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
pub(crate) fn test_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "table-tavern-smart-free-{label}-{}",
        ulid::Ulid::generate()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_in_stable_mode_and_accepts_legacy_smart_mode_on_openrouter() {
        let mut config = AppConfig::default();
        assert!(!is_active(&config));
        config
            .preferences
            .insert(MODE_KEY.to_owned(), MODE_STABLE.into());
        assert!(is_active(&config));
        config
            .preferences
            .insert(MODE_KEY.to_owned(), MODE_SMART_LEGACY.into());
        assert!(is_active(&config));
        assert!(migrate_legacy_mode(&mut config));
        assert_eq!(
            config
                .preferences
                .get(MODE_KEY)
                .and_then(|value| value.as_str()),
            Some(MODE_STABLE)
        );
        config
            .preferences
            .insert("base_url".to_owned(), "https://example.com/v1".into());
        assert!(!is_active(&config));
    }

    #[test]
    fn context_budget_includes_message_overhead_and_output_reserve() {
        let messages = [ChatMessage {
            role: "user".to_owned(),
            content: "abcdefgh".to_owned(),
        }];
        assert_eq!(
            required_context(&messages),
            2 + 4 + 2 + select::RESERVED_OUTPUT_TOKENS
        );
    }

    #[test]
    fn responder_switch_updates_existing_pin_but_first_binding_is_silent() {
        let root = test_root("pins");
        assert!(!record_responder(&root, Some("table-a"), "alpha/one:free"));
        assert!(!record_responder(&root, Some("table-a"), "alpha/one:free"));
        assert!(record_responder(&root, Some("table-a"), "beta/two:free"));
        assert_eq!(
            store::read_pins(&root).get("table-a").map(String::as_str),
            Some("beta/two:free")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn empty_status_is_safe_before_first_catalog_refresh() {
        let root = test_root("empty-status");
        let mut config = AppConfig::default();
        config
            .api_keys
            .insert("openrouter".to_owned(), "sk-or-test".to_owned());
        assert_eq!(status(&root, &config), Status::default());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn expiry_warning_starts_three_days_early_and_deduplicates_per_expiry() {
        let root = test_root("expiry-warning");
        let now = 2_000_000_000;
        let mut model = select::FreeModel {
            id: "preview/model:free".to_owned(),
            canonical_slug: "preview/model-20260101".to_owned(),
            name: "Preview Model".to_owned(),
            created: now - 100,
            context_length: 128_000,
            expiration_at: Some(now + 72 * 3600),
            supported_parameters: Vec::new(),
        };

        assert_eq!(
            take_expiry_warning(
                &root,
                Some("table-a"),
                Some("preview/model:free"),
                &[model.clone()],
                now,
            ),
            Some(ExpiryWarning {
                model: "Preview Model".to_owned(),
                expires_at: now + 72 * 3600,
            })
        );
        assert!(take_expiry_warning(
            &root,
            Some("table-a"),
            Some("preview/model:free"),
            &[model.clone()],
            now,
        )
        .is_none());

        model.expiration_at = Some(now + 71 * 3600);
        assert!(take_expiry_warning(
            &root,
            Some("table-a"),
            Some("preview/model:free"),
            &[model.clone()],
            now,
        )
        .is_some());

        model.expiration_at = Some(now + 73 * 3600);
        assert!(take_expiry_warning(
            &root,
            Some("table-b"),
            Some("preview/model:free"),
            &[model],
            now,
        )
        .is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn new_recommendations_seeds_baseline_silently_then_flags_only_later_additions() {
        let root = test_root("new-recs");
        let mut config = AppConfig::default();
        config
            .api_keys
            .insert("openrouter".to_owned(), "sk-or-test".to_owned());
        let now = now_secs();
        let limited = |id: &str, slug: &str| select::FreeModel {
            id: id.to_owned(),
            canonical_slug: slug.to_owned(),
            name: id.to_owned(),
            created: now.saturating_sub(100),
            context_length: 256_000,
            expiration_at: Some(now + 40 * 24 * 3600),
            supported_parameters: Vec::new(),
        };
        let write_catalog = |models: Vec<select::FreeModel>| {
            store::write_cache(
                &root,
                &store::Cache {
                    account_fingerprint: fingerprint("sk-or-test"),
                    user_catalog: Some(models),
                    user_catalog_fetched_at: now,
                    weekly_ids: Some(Vec::new()),
                    weekly_fetched_at: now,
                    roleplay_slugs: Some(Vec::new()),
                    roleplay_fetched_at: now,
                },
            )
            .unwrap();
        };

        // 首次啟動時限時清單為空：也要建立空基準，不能之後把第一支真正新出現的吃掉。
        write_catalog(Vec::new());
        assert!(new_recommendations(&root, &config).is_empty());

        // 空基準之後第一支限時模型仍要被提示。
        write_catalog(vec![limited("preview/a:free", "preview/a-1")]);
        assert_eq!(
            new_recommendations(&root, &config)
                .iter()
                .map(|item| item.model.as_str())
                .collect::<Vec<_>>(),
            ["preview/a:free"]
        );
        mark_recommendations_seen(&root, &config, &["preview/a:free".to_owned()]);
        assert!(new_recommendations(&root, &config).is_empty());

        // 再有一支新的照樣提示、已看過的不重跳。
        write_catalog(vec![
            limited("preview/a:free", "preview/a-1"),
            limited("preview/b:free", "preview/b-1"),
        ]);
        assert_eq!(
            new_recommendations(&root, &config)
                .iter()
                .map(|item| item.model.as_str())
                .collect::<Vec<_>>(),
            ["preview/b:free"]
        );
        mark_recommendations_seen(&root, &config, &["preview/b:free".to_owned()]);
        assert!(new_recommendations(&root, &config).is_empty());

        // 整體開關關閉後一律回空。
        config
            .preferences
            .insert(NOTIFY_KEY.to_owned(), false.into());
        assert!(new_recommendations(&root, &config).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn recommendation_reason_uses_immediate_structural_signals() {
        let now = 2_000_000_000;
        let stealth = select::FreeModel {
            id: "stealth/new".to_owned(),
            canonical_slug: "stealth/new".to_owned(),
            name: "Stealth".to_owned(),
            created: now - 60,
            context_length: 256_000,
            expiration_at: None,
            supported_parameters: Vec::new(),
        };
        let preview = select::FreeModel {
            id: "google/example-preview:free".to_owned(),
            canonical_slug: "google/example-preview-20260101".to_owned(),
            name: "Example Preview".to_owned(),
            created: now - 60,
            context_length: 128_000,
            expiration_at: Some(now + 48 * 3600),
            supported_parameters: Vec::new(),
        };
        let anonymous = recommendation(&stealth, now, None);
        assert_eq!(anonymous.reason, "anonymous_test");
        assert!(anonymous.recent);
        assert!(anonymous.long_context);
        assert!(anonymous.provider.is_empty());

        let provider = recommendation(&preview, now, None);
        assert_eq!(provider.reason, "provider_test");
        assert_eq!(provider.provider, "google");
        assert!(provider.show_expiry_date);
        assert!(provider.expiring_soon);
    }
}
