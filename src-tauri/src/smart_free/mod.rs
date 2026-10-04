//! 動態穩定免費：用玩家 OpenRouter OAuth key 取得帳號可用免費模型，依角色扮演排行排出最多 4 支、
//! 上游分散的穩定名單。每次派送只帶一支；目前模型由試打與「連續 2 次失敗換下一支」決定，
//! 換模一定明講（計畫 .ai/plans/stable-free-failover.md）。

mod api;
mod call;
mod failover;
mod probe;
mod select;
mod store;

pub(crate) use call::{CallEnv, CallOutcome, CallPlan};

use crate::data::AppConfig;
use crate::transport::{base_url, ChatMessage, DEFAULT_BASE_URL};
use crate::ui_msg::UiMsg;
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
/// 帳號清單篩選規則的版本；改了 `select::parse_model` 的入選條件就 +1。
/// 1：輸出模態必須只有文字〔作者裁決 2026-10-04〕。
const CATALOG_SCHEMA: u32 = 1;
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
            .saturating_add(crate::usage::log::estimate_tokens(&message.content))
            .saturating_add(4)
    });
    input.saturating_add(select::RESERVED_OUTPUT_TOKENS)
}

/// 模式／金鑰指紋／base_url：任何一項變了，選模世代就要推進（晚回的派送與試打因此作廢）。
fn config_key(config: &AppConfig) -> String {
    format!(
        "{}|{}|{}",
        config
            .preferences
            .get(MODE_KEY)
            .and_then(|value| value.as_str())
            .unwrap_or(""),
        api_key(config).map(fingerprint).unwrap_or_default(),
        base_url(config)
    )
}

/// 寫完設定（設定頁存檔、OAuth、手貼金鑰）後呼叫：設定鍵變了就推進選模世代。
fn observe_config(root: &Path, config: &AppConfig) {
    let key = config_key(config);
    failover::with_runtime(root, |runtime| runtime.observe_config(&key));
}

/// 穩定名單＋名單外的其他穩定候選（依名次）。
fn ranking<'a>(
    cache: &store::Cache,
    catalog: &'a [select::FreeModel],
    now: u64,
) -> (select::Lineup<'a>, Vec<String>) {
    let ranked = select::stable_candidates(
        catalog,
        cache.roleplay_slugs.as_deref().unwrap_or(&[]),
        cache.weekly_ids.as_deref().unwrap_or(&[]),
        select::RESERVED_OUTPUT_TOKENS,
        now,
    );
    let lineup = select::build_lineup(&ranked, &cache.upstreams);
    let ids = lineup.ids();
    let others = ranked
        .iter()
        .map(|(model, _)| model.id.clone())
        .filter(|id| !ids.contains(id))
        .collect();
    (lineup, others)
}

/// 目前模型：本帳號、仍在名單內的記錄；沒有就名單第 1 名。不寫檔。
fn current_model(root: &Path, account: &str, lineup: &[String]) -> Option<String> {
    let saved = store::read_current(root);
    if saved.account == account && lineup.contains(&saved.model) {
        return Some(saved.model);
    }
    lineup.first().cloned()
}

pub struct PreparedCall {
    pub expiry_warning: Option<ExpiryWarning>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ExpiryWarning {
    pub model: String,
    pub expires_at: u64,
}

/// 每次真正送出前走這裡（可以 await）：先確認免費日額度、刷新必要資料、處理到期提醒。
/// 選模素材不在這裡備，取票當下才由 `plan_from_disk` 讀權威資料。
pub async fn prepare_call(
    root: &Path,
    config: &AppConfig,
    world: Option<&str>,
    messages: &[ChatMessage],
) -> Result<PreparedCall, String> {
    let key = api_key(config).ok_or_else(|| UiMsg::OpenrouterApiKeyMissing.to_string())?;
    if let Some(client) = api::client() {
        if api::free_daily_remaining(&client, key)
            .await
            .is_some_and(|remaining| remaining <= 0)
        {
            // 前綴留在起首給前端分流（errQuotaApi），說明改代碼接在後面
            return Err(format!(
                "AI_HTTP_STATUS_429: {}",
                UiMsg::SmartFreeDailyExhausted
            ));
        }
    }

    refresh_if_stale(root, config).await;
    // 先用同一套規則檢查一次，讓「沒有免費模型」這類錯誤在送出前就回報
    plan_from_disk(root, config, messages)?;
    let cache = store::read_cache(root);
    let catalog = cache.catalog_for(&fingerprint(key)).unwrap_or(&[]);
    let pins = store::read_pins(root);
    let previous = world.and_then(|world| pins.get(world)).map(String::as_str);
    Ok(PreparedCall {
        expiry_warning: take_expiry_warning(root, world, previous, catalog, now_secs()),
    })
}

/// 取票當下的權威選模素材：重讀磁碟上的設定與快取（不連網、不 await）。`sending` 是這次呼叫
/// 實際用來送出的設定；與磁碟最新設定不同時 `matches_sender = false`，結果一律不計。
pub fn plan_from_disk(
    root: &Path,
    sending: &AppConfig,
    messages: &[ChatMessage],
) -> Result<CallPlan, String> {
    let config = crate::data::read_config(root).unwrap_or_else(|_| sending.clone());
    let key = api_key(&config).ok_or_else(|| UiMsg::OpenrouterApiKeyMissing.to_string())?;
    let account = fingerprint(key);
    let cache = store::read_cache(root);
    let catalog = cache.catalog_for(&account).unwrap_or(&[]);
    let now = now_secs();
    let eligible = select::eligible_models(catalog, required_context(messages), now, &[]);
    if eligible.is_empty() {
        return Err(UiMsg::NoFreeModels.into());
    }
    let (lineup, others) = ranking(&cache, catalog, now);
    if lineup.entries.is_empty() {
        return Err(UiMsg::NoStableFreeModel.into());
    }
    Ok(CallPlan {
        config_key: config_key(&config),
        matches_sender: config_key(&config) == config_key(sending) && is_active(&config),
        lineup: lineup.ids(),
        others,
        fits: eligible.into_iter().map(|model| model.id).collect(),
        names: catalog
            .iter()
            .map(|model| (model.id.clone(), model.name.clone()))
            .collect(),
        account,
    })
}

/// 跑一次邏輯呼叫（最多兩發）；狀態落盤到 root 的 `smart_free_current.json`。
pub(crate) async fn run_call(root: &Path, env: &mut impl CallEnv) -> Result<CallOutcome, String> {
    let owned = root.to_owned();
    let mut persist = move |state: &store::CurrentState| store::write_current(&owned, state);
    call::run_call(root, env, &mut persist, now_secs()).await
}

/// `smart-free-failover` 事件（§2.4）：每次新 eventId；from／to 是顯示名。
pub fn failover_payload(
    world: Option<&str>,
    turn_id: Option<&str>,
    from: &str,
    to: &str,
    retried: bool,
) -> serde_json::Value {
    serde_json::json!({
        "eventId": ulid::Ulid::generate().to_string(),
        "world": world,
        "turnId": turn_id,
        "from": from,
        "to": to,
        "retried": retried,
    })
}

/// `smart-free-model-switched` 事件（§2.4）：每次新 eventId；model 是顯示名。
pub fn switched_payload(
    world: Option<&str>,
    turn_id: Option<&str>,
    model: &str,
) -> serde_json::Value {
    serde_json::json!({
        "eventId": ulid::Ulid::generate().to_string(),
        "world": world,
        "turnId": turn_id,
        "model": model,
    })
}

/// 每日免費剩餘次數（`/key`）；查不到回 None。
pub async fn daily_remaining(config: &AppConfig) -> Option<i64> {
    let key = api_key(config)?;
    api::free_daily_remaining(&api::client()?, key).await
}

/// 測試通道的 route 預覽：不查額度、不刷新快取、不發請求，以「空訊息＋保留輸出」的最小長度估；
/// 回目前模型（實際送出可能因訊息較長而改用本句替代）。
#[cfg(feature = "test-harness")]
pub fn preview_primary(root: &Path, config: &AppConfig) -> Result<Option<String>, String> {
    let key = api_key(config).ok_or_else(|| UiMsg::OpenrouterApiKeyMissing.to_string())?;
    let cache = store::read_cache(root);
    let account = fingerprint(key);
    let catalog = cache.catalog_for(&account).unwrap_or(&[]);
    let now = now_secs();
    if select::eligible_models(catalog, required_context(&[]), now, &[]).is_empty() {
        return Err(UiMsg::NoFreeModels.into());
    }
    let (lineup, _) = ranking(&cache, catalog, now);
    Ok(current_model(root, &account, &lineup.ids()))
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
    /// 穩定名單（最多 4 支、上游分散）：第 1 名是自動模式首選，其餘依序是擁擠時的換模順序，也可手動固定。
    pub stable: Vec<Recommendation>,
    /// false＝名單含上游未知或因同上游不相鄰而少列；設定頁要揭露降級。
    pub diversified: bool,
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
    let (lineup, _) = ranking(&cache, catalog, now);
    RecommendationList {
        limited: select::limited_models(catalog, select::RESERVED_OUTPUT_TOKENS, now)
            .into_iter()
            .map(|model| recommendation(model, now, None))
            .collect(),
        stable: lineup
            .entries
            .iter()
            .map(|(model, source)| recommendation(model, now, Some(*source)))
            .collect(),
        diversified: lineup.diversified,
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
    let (lineup, _) = ranking(&cache, catalog, now_secs());
    Status {
        model: current_model(root, &fingerprint(key), &lineup.ids()).unwrap_or_default(),
        free_daily: None,
    }
}

/// 設定頁用：在 status() 之上補上每日免費額度。
/// `:free` 模型會計入帳號的每日免費池（`/key` 的 free_model_daily_requests，有回報延遲）；
/// stealth／促銷等非 `:free` 模型不計入，視為無限（free_daily = None，UI 顯示「無限」）。
pub async fn status_with_quota(root: &Path, config: &AppConfig) -> Status {
    let mut status = status(root, config);
    // 每日免費額度是全帳號共用池，動態穩定免費與推薦模式都要顯示；
    // 推薦模式看玩家固定的那支（三 tier 同一支），動態穩定免費看目前模型。
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

/// 帳號清單要不要重抓：換帳號、篩選規則版本較舊、沒資料、舊格式缺 canonical_slug、或過了 TTL。
fn user_catalog_stale(cache: &store::Cache, account: &str, now: u64) -> bool {
    cache.account_fingerprint != account
        || cache.catalog_schema < CATALOG_SCHEMA
        || cache.user_catalog.as_ref().is_none_or(Vec::is_empty)
        || cache
            .user_catalog
            .as_ref()
            .is_some_and(|models| models.iter().any(|model| model.canonical_slug.is_empty()))
        || now.saturating_sub(cache.user_catalog_fetched_at) >= TTL_SECS
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
    let user_stale = user_catalog_stale(&cache, &account, now);
    let weekly_stale =
        cache.weekly_ids.is_none() || now.saturating_sub(cache.weekly_fetched_at) >= TTL_SECS;
    let roleplay_stale =
        cache.roleplay_slugs.is_none() || now.saturating_sub(cache.roleplay_fetched_at) >= TTL_SECS;
    let mut changed = false;

    if user_stale {
        if let Some(catalog) = api::fetch_user_catalog(&client, key).await {
            cache.account_fingerprint = account.clone();
            cache.user_catalog = Some(catalog);
            cache.user_catalog_fetched_at = now;
            cache.catalog_schema = CATALOG_SCHEMA;
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
    let upstreams_stale = changed || now.saturating_sub(cache.upstreams_fetched_at) >= TTL_SECS;
    if upstreams_stale {
        if let Some(catalog) = cache.catalog_for(&account) {
            // 全部穩定候選都抓（數量本來就少、不佔次數）；單支失敗保留舊值，沒有舊值＝上游未知。
            let ids: Vec<String> = select::stable_candidates(
                catalog,
                cache.roleplay_slugs.as_deref().unwrap_or(&[]),
                cache.weekly_ids.as_deref().unwrap_or(&[]),
                select::RESERVED_OUTPUT_TOKENS,
                now,
            )
            .into_iter()
            .map(|(model, _)| model.id.clone())
            .collect();
            let mut upstreams = std::collections::BTreeMap::new();
            for id in ids {
                let fetched = api::fetch_upstreams(&client, key, &id).await;
                if let Some(providers) = fetched.or_else(|| cache.upstreams.get(&id).cloned()) {
                    upstreams.insert(id, providers);
                }
            }
            cache.upstreams = upstreams;
            cache.upstreams_fetched_at = now;
            changed = true;
        }
    }
    if changed && store::write_cache(root, &cache).is_ok() {
        sync_lineup(root, &account, &cache);
    }
    changed
}

/// 快取換新後立刻對齊名單世代：名單變了就重建目前模型並推進 epoch，舊名單發出的票全部作廢。
fn sync_lineup(root: &Path, account: &str, cache: &store::Cache) {
    let lineup = cache
        .catalog_for(account)
        .map(|catalog| ranking(cache, catalog, now_secs()).0.ids())
        .unwrap_or_default();
    let owned = root.to_owned();
    let mut persist = move |state: &store::CurrentState| store::write_current(&owned, state);
    failover::with_runtime(root, |runtime| {
        // 只對齊同一帳號；換帳號留給取票時用權威設定處理
        if !runtime.saved.account.is_empty() && runtime.saved.account != account {
            return;
        }
        if lineup.is_empty() {
            // 名單變空也是世代變更：作廢在途的舊票，舊結果不得再計入或照舊名單換模
            runtime.invalidate();
        } else {
            runtime.reconcile(account, &lineup, now_secs(), &mut persist);
        }
    });
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
    observe_config(root, config);
    if tracks_openrouter(config) {
        let app = app.clone();
        let root = root.to_owned();
        let config = config.clone();
        tauri::async_runtime::spawn(async move {
            let refreshed = refresh_if_stale(&root, &config).await;
            let probed = probe_if_active(&root, probe::Trigger::Startup).await;
            if refreshed || probed {
                let _ = app.emit(CACHE_UPDATED_EVENT, ());
            }
        });
    }
}

pub fn spawn_background_refresh(app: tauri::AppHandle, config_root: std::path::PathBuf) {
    tauri::async_runtime::spawn(async move {
        // 第一圈就是 App 啟動：照啟動規則試打；之後每小時照背景規則。
        let mut trigger = probe::Trigger::Startup;
        loop {
            if let Ok(config) = crate::data::read_config(&config_root) {
                if tracks_openrouter(&config) {
                    let refreshed = refresh_if_stale(&config_root, &config).await;
                    let probed = probe_if_active(&config_root, trigger).await;
                    if refreshed || probed {
                        let _ = app.emit(CACHE_UPDATED_EVENT, ());
                    }
                }
            }
            trigger = probe::Trigger::Background;
            tokio::time::sleep(RECHECK_EVERY).await;
        }
    });
}

/// 正式的試打環境：每發前重讀設定、試打走官方（或測試通道覆寫的）OpenRouter。
struct LiveProbe {
    root: std::path::PathBuf,
    account: String,
    key: String,
}

impl probe::ProbeEnv for LiveProbe {
    async fn daily_remaining(&mut self) -> Option<i64> {
        api::free_daily_remaining(&api::client()?, &self.key).await
    }

    async fn probe(&mut self, model: &str) -> Result<(), crate::transport::ApiFailure> {
        #[cfg(feature = "test-harness")]
        let dispatch = crate::harness::ai_dispatch(
            "api-smart-free-probe",
            &format!("試打：{model}"),
            serde_json::json!({}),
        );
        let result = api::probe(&self.key, model).await;
        #[cfg(feature = "test-harness")]
        crate::harness::ai_event(
            &dispatch,
            "probe",
            serde_json::json!({ "ok": result.is_ok(), "status": result.as_ref().err().and_then(|failure| failure.status) }),
        );
        result
    }

    /// 權威資料：重讀磁碟設定與快取。已不是穩定免費、或磁碟上的金鑰已不是這輪用來試打的那把，就回 None。
    fn authority(&mut self) -> Option<probe::Authority> {
        let config = crate::data::read_config(&self.root).ok()?;
        if !is_active(&config)
            || api_key(&config).map(fingerprint).as_deref() != Some(&self.account)
        {
            return None;
        }
        let cache = store::read_cache(&self.root);
        let catalog = cache.catalog_for(&self.account)?;
        let lineup = ranking(&cache, catalog, now_secs()).0.ids();
        (!lineup.is_empty()).then(|| probe::Authority {
            config_key: config_key(&config),
            account: self.account.clone(),
            lineup,
        })
    }
}

/// 穩定免費模式才試打；回傳是否跑了（設定頁要重查目前模型）。
async fn probe_if_active(root: &Path, trigger: probe::Trigger) -> bool {
    let Ok(config) = crate::data::read_config(root) else {
        return false;
    };
    if !is_active(&config) {
        return false;
    }
    let Some(key) = api_key(&config) else {
        return false;
    };
    let mut env = LiveProbe {
        root: root.to_owned(),
        account: fingerprint(key),
        key: key.to_owned(),
    };
    let owned = root.to_owned();
    let mut persist = move |state: &store::CurrentState| store::write_current(&owned, state);
    probe::run_probe(root, trigger, &mut env, &mut persist, now_secs())
        .await
        .is_some()
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
        config
            .preferences
            .insert("base_url".to_owned(), "https://example.com/v1".into());
        assert!(!is_active(&config));
    }

    #[test]
    fn migrate_legacy_config_rewrites_only_smart_free() {
        let root = test_root("migrate");
        crate::data::update_config(
            &root,
            &serde_json::json!({
                "preferences": { MODE_KEY: MODE_SMART_LEGACY, "language": "zh-TW" },
                "tier_models": { "claude:fast": "haiku" }
            }),
        )
        .unwrap();
        let saved = crate::data::migrate_legacy_config(&root).unwrap();
        assert_eq!(
            saved
                .preferences
                .get(MODE_KEY)
                .and_then(|value| value.as_str()),
            Some(MODE_STABLE)
        );
        assert_eq!(
            saved
                .preferences
                .get("language")
                .and_then(|value| value.as_str()),
            Some("zh-TW")
        );
        assert_eq!(
            saved.tier_models.get("claude:fast").map(String::as_str),
            Some("haiku")
        );
        assert!(is_active(&saved));

        crate::data::update_config(
            &root,
            &serde_json::json!({ "preferences": { MODE_KEY: MODE_RECOMMENDED } }),
        )
        .unwrap();
        let kept = crate::data::migrate_legacy_config(&root).unwrap();
        assert_eq!(
            kept.preferences
                .get(MODE_KEY)
                .and_then(|value| value.as_str()),
            Some(MODE_RECOMMENDED)
        );
        let _ = std::fs::remove_dir_all(root);
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
                    ..store::Cache::default()
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

    #[test]
    fn event_payloads_carry_fresh_event_ids_world_turn_and_display_names() {
        let one = failover_payload(Some("w1"), Some("turn-9"), "Model A", "Model B", true);
        let two = failover_payload(Some("w1"), Some("turn-9"), "Model A", "Model B", true);
        assert_eq!(one["world"], "w1");
        assert_eq!(one["turnId"], "turn-9");
        assert_eq!(one["from"], "Model A");
        assert_eq!(one["to"], "Model B");
        assert_eq!(one["retried"], true);
        assert_eq!(one["eventId"].as_str().unwrap().len(), 26);
        assert_ne!(one["eventId"], two["eventId"]);
        let switched = switched_payload(None, None, "Model C");
        assert_eq!(switched["model"], "Model C");
        assert!(switched["world"].is_null());
        assert!(switched["turnId"].is_null());
        assert_ne!(
            switched["eventId"],
            switched_payload(None, None, "Model C")["eventId"]
        );
    }

    fn stable(id: &str, name: &str) -> select::FreeModel {
        select::FreeModel {
            id: id.to_owned(),
            canonical_slug: id.to_owned(),
            name: name.to_owned(),
            created: 1,
            context_length: 64_000,
            expiration_at: None,
            supported_parameters: Vec::new(),
        }
    }

    fn cache_with(account: &str, models: Vec<select::FreeModel>, weekly: &[&str]) -> store::Cache {
        store::Cache {
            account_fingerprint: account.to_owned(),
            user_catalog: Some(models),
            user_catalog_fetched_at: now_secs(),
            weekly_ids: Some(weekly.iter().map(|id| (*id).to_owned()).collect()),
            weekly_fetched_at: now_secs(),
            roleplay_slugs: Some(Vec::new()),
            roleplay_fetched_at: now_secs(),
            ..store::Cache::default()
        }
    }

    #[test]
    fn cache_refresh_advances_the_lineup_generation_and_voids_old_tickets() {
        let root = test_root("sync-lineup");
        let account = fingerprint("sk-or-test");
        let models = vec![stable("x/a:free", "A"), stable("x/b:free", "B")];
        let old_cache = cache_with(&account, models.clone(), &["x/a:free", "x/b:free"]);
        store::write_cache(&root, &old_cache).unwrap();
        sync_lineup(&root, &account, &old_cache);
        let (ticket, old_gen) = failover::with_runtime(&root, |runtime| {
            (
                runtime.ticket("x/a:free", "x/a:free", false),
                runtime.saved.lineup_gen.clone(),
            )
        });
        // 排行換了：b 變第一名
        let new_cache = cache_with(&account, models, &["x/b:free", "x/a:free"]);
        store::write_cache(&root, &new_cache).unwrap();
        sync_lineup(&root, &account, &new_cache);
        failover::with_runtime(&root, |runtime| {
            assert_ne!(runtime.saved.lineup_gen, old_gen);
            assert_eq!(runtime.saved.model, "x/b:free");
            let lineup = vec!["x/b:free".to_owned(), "x/a:free".to_owned()];
            let mut persist = |_: &store::CurrentState| Ok(());
            assert_eq!(
                runtime.record_chat(
                    &ticket,
                    Some(failover::FailureClass::Model),
                    true,
                    &lineup,
                    1,
                    &mut persist
                ),
                failover::ChatRecord::Ignored
            );
        });
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn plan_reads_authoritative_disk_config_and_flags_a_stale_sender() {
        let root = test_root("plan-disk");
        crate::data::update_config(
            &root,
            &serde_json::json!({
                "preferences": { MODE_KEY: MODE_STABLE },
                "api_keys": { "openrouter": "sk-or-new" }
            }),
        )
        .unwrap();
        let account = fingerprint("sk-or-new");
        store::write_cache(
            &root,
            &cache_with(&account, vec![stable("x/a:free", "Model A")], &["x/a:free"]),
        )
        .unwrap();
        let disk = crate::data::read_config(&root).unwrap();
        let plan = plan_from_disk(&root, &disk, &[]).unwrap();
        assert!(plan.matches_sender);
        assert_eq!(plan.account, account);
        assert_eq!(plan.lineup, ["x/a:free"]);
        assert_eq!(plan.name("x/a:free"), "Model A");
        // 送出用的是舊金鑰：權威素材仍是新帳號，但標成不可計數
        let mut old = disk.clone();
        old.api_keys
            .insert("openrouter".to_owned(), "sk-or-old".to_owned());
        let stale = plan_from_disk(&root, &old, &[]).unwrap();
        assert_eq!(stale.account, account);
        assert!(!stale.matches_sender);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_to_an_empty_lineup_voids_in_flight_tickets() {
        let root = test_root("sync-empty");
        let account = fingerprint("sk-or-test");
        let models = vec![stable("x/a:free", "A"), stable("x/b:free", "B")];
        let full = cache_with(&account, models, &["x/a:free", "x/b:free"]);
        store::write_cache(&root, &full).unwrap();
        sync_lineup(&root, &account, &full);
        let ticket = failover::with_runtime(&root, |runtime| {
            runtime.ticket("x/a:free", "x/a:free", false)
        });
        // 刷新後一支合格穩定模型都沒有
        let empty = cache_with(&account, Vec::new(), &[]);
        store::write_cache(&root, &empty).unwrap();
        sync_lineup(&root, &account, &empty);
        failover::with_runtime(&root, |runtime| {
            let old_lineup = vec!["x/a:free".to_owned(), "x/b:free".to_owned()];
            let mut persist = |_: &store::CurrentState| Ok(());
            for _ in 0..2 {
                assert_eq!(
                    runtime.record_chat(
                        &ticket,
                        Some(failover::FailureClass::Model),
                        true,
                        &old_lineup,
                        1,
                        &mut persist
                    ),
                    failover::ChatRecord::Ignored
                );
            }
            assert_eq!(runtime.saved.model, "x/a:free");
            assert_eq!(runtime.count, 0);
        });
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn catalog_from_an_older_filter_rule_is_refetched_before_ttl() {
        let account = fingerprint("sk-or-test");
        let mut cache = cache_with(&account, vec![stable("x/a:free", "A")], &[]);
        let now = cache.user_catalog_fetched_at;
        // 剛抓、但還是舊規則（輸出含音訊的模型可能還在裡面）→ 重抓
        assert!(user_catalog_stale(&cache, &account, now));
        cache.catalog_schema = CATALOG_SCHEMA;
        assert!(!user_catalog_stale(&cache, &account, now));
        assert!(user_catalog_stale(&cache, &account, now + TTL_SECS));
    }
}
