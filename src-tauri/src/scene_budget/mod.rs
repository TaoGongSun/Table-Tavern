//! 換幕容量（long-prompt-scene-hint 範圍 3，計畫 §3）：這一幕還塞不塞得下。
//! - 提醒：任一路徑 `H ≥ 0.8 × (L − F − R)`；換幕呼叫與每條聊天路徑各自在自己的單位裡算。
//! - 鎖（只看換幕）：`H_S ＋ 當次本句 ＋ G_reply > L_S − F_S − R_S`，且上限來源可靠、本模型有校正值。
//! - `G_reply` 是預測不是證明：猜錯由換幕的分段摘要接住。
mod capacity;
mod estimate;
mod gate;
mod limits;
mod measure;
pub mod summarize;

pub use capacity::{claude_model_usage, forget_model, record_model};
pub use estimate::{budget_tokens, Unit};

use crate::data::{self, AppConfig, TranscriptKind};
use measure::Raw;
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

/// 換幕摘要那次的回報：實報總輸入與（claude）辨識出的實際模型 id。
#[derive(Default)]
pub struct SummaryProbe {
    pub prompt_tokens: AtomicU64,
    pub identity: std::sync::Mutex<Option<String>>,
}

tokio::task_local! {
    /// dispatch 的 CLI 單發把它掛進 UsageLog（prompt_tokens_out／identity_out），
    /// 換幕流程拿它和同一次請求的估計配成校正。
    static SUMMARY_PROBE: Arc<SummaryProbe>;
}

/// dispatch 用：目前這個 task 若在量換幕摘要，回探針。
pub fn summary_probe() -> Option<Arc<SummaryProbe>> {
    SUMMARY_PROBE.try_with(Arc::clone).ok()
}

/// 在探針範圍內跑一次換幕摘要呼叫；回 (結果, 實報總輸入（0＝沒回報）, 辨識出的模型 id)。
pub async fn with_summary_probe<T>(
    call: impl std::future::Future<Output = T>,
) -> (T, u64, Option<String>) {
    let probe = Arc::new(SummaryProbe::default());
    let result = SUMMARY_PROBE.scope(probe.clone(), call).await;
    let identity = probe.identity.lock().ok().and_then(|id| id.clone());
    (
        result,
        probe
            .prompt_tokens
            .load(std::sync::atomic::Ordering::Relaxed),
        identity,
    )
}

/// 記一筆校正（lanes 重開全量那次、換幕摘要那次）。
pub fn record_calibration(
    root: &Path,
    transport: &str,
    model: &str,
    kind: &str,
    estimate: u64,
    actual: u64,
    model_id: Option<&str>,
) {
    capacity::record_calibration(
        &capacity::path(root),
        transport,
        model,
        kind,
        estimate,
        actual,
        model_id,
    );
}

/// 提醒門檻 80%：`used ≥ 0.8 × cap`（含等號）；cap 為 0 時必提醒。
fn reaches_hint(used: u64, cap: u64) -> bool {
    used.saturating_mul(10) >= cap.saturating_mul(8)
}

/// 一條路徑（換幕或某條聊天）的容量。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathBudget {
    pub unit: Unit,
    /// 本幕紀錄在這次呼叫裡的量 H（已乘校正）
    pub used: u64,
    /// `L − F − R`
    pub cap: u64,
    pub hint: bool,
    /// 校正倍率（bytes 恆 1；token 沒校正為 None）
    pub ratio: Option<f64>,
    /// 上限來源可靠
    pub reliable: bool,
}

fn scale(value: u64, ratio: f64) -> u64 {
    (value as f64 * ratio).ceil() as u64
}

/// 原始量＋上限 → 這條路徑的容量。`reported_total`：lane 上一輪實報的整段輸入，比組裝估得大就用它。
pub fn path_budget(
    limit: &limits::Limit,
    kind: &str,
    raw: Raw,
    reported_total: Option<u64>,
) -> PathBudget {
    let ratio = limit.ratio(kind);
    let factor = ratio.unwrap_or(1.0);
    let fixed = scale(raw.fixed, factor);
    let mut full = scale(raw.full, factor).max(fixed);
    if let Some(reported) = reported_total {
        full = full.max(reported);
    }
    let used = full - fixed;
    let cap = limit
        .total
        .saturating_sub(fixed)
        .saturating_sub(limit.reserve);
    PathBudget {
        unit: limit.unit,
        used,
        cap,
        hint: reaches_hint(used, cap),
        ratio,
        reliable: limit.reliable,
    }
}

/// 歷史裡每個「動作」全部回覆的量（不含玩家句）：每次送出、旁白、推進、點名各是一個動作，靠事件上的
/// `action_id` 切段〔作者裁決 2026-10-06〕。舊事件沒有 id 時退回以玩家句切段（併進前一段，只會高估）。
const G_REPLY_WINDOW: usize = 10;
const G_REPLY_FLOOR_TOKENS: u64 = 1_500;
const G_REPLY_FLOOR_BYTES: u64 = 4_500;

/// 預測用的一則事件：是不是玩家句、屬於哪個動作、量。
#[derive(Debug, Clone, Copy)]
pub struct ReplySize<'a> {
    pub player: bool,
    pub action: Option<&'a str>,
    pub size: u64,
}

/// 下一輪回覆增量的預測（唯一算法，前後端共用）：最近 10 個動作的全部回覆量取最大 ×1.2，有下限。
/// 新段：玩家句、或帶了跟目前這段不同的 `action_id`；沒帶 id 的回覆併進目前這段。
pub fn predict_reply(sizes: &[ReplySize], unit: Unit) -> u64 {
    let mut actions: Vec<u64> = Vec::new();
    let mut current: Option<(Option<&str>, u64)> = None;
    for event in sizes {
        let starts = match &current {
            None => true,
            Some((open, _)) => event.player || (event.action.is_some() && event.action != *open),
        };
        let reply = if event.player { 0 } else { event.size };
        if starts {
            actions.extend(current.take().map(|(_, total)| total));
            current = Some((event.action, reply));
        } else if let Some((_, total)) = current.as_mut() {
            *total += reply;
        }
    }
    actions.extend(current.map(|(_, total)| total));
    let peak = actions
        .iter()
        .rev()
        .filter(|total| **total > 0)
        .take(G_REPLY_WINDOW)
        .max()
        .copied()
        .unwrap_or(0);
    let predicted = (peak as f64 * 1.2).ceil() as u64;
    predicted.max(match unit {
        Unit::Tokens => G_REPLY_FLOOR_TOKENS,
        Unit::Bytes => G_REPLY_FLOOR_BYTES,
    })
}

/// 玩家句在換幕摘要裡多出的包裝（名字前綴、分隔）：保守固定值，前後端同一個數。
pub const DRAFT_OVERHEAD_TOKENS: u64 = 16;
pub const DRAFT_OVERHEAD_BYTES: u64 = 64;

/// 當次本句在換幕單位下的量（空字串＝無玩家句的動作，0）。前端 `scene-budget.ts` 同一算法。
pub fn draft_size(text: &str, unit: Unit, ratio: f64) -> u64 {
    if text.is_empty() {
        return 0;
    }
    match unit {
        Unit::Tokens => scale(budget_tokens(text), ratio) + DRAFT_OVERHEAD_TOKENS,
        Unit::Bytes => text.len() as u64 + DRAFT_OVERHEAD_BYTES,
    }
}

/// 換幕那條的容量：比聊天多了「能不能鎖」與下一輪預測。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryBudget {
    #[serde(flatten)]
    pub path: PathBudget,
    /// 可鎖：來源可靠且有校正（bytes 精確不需校正）
    pub lockable: bool,
    pub g_reply: u64,
    /// 換幕本身已放不下（交給分段摘要）
    pub over: bool,
}

impl SummaryBudget {
    /// 鎖的判定（後端 gate 與前端即時判定同一式）：再送這一句就會讓換幕一次送不出去。
    pub fn would_overflow(&self, draft: u64) -> bool {
        self.lockable && self.path.used + draft + self.g_reply > self.path.cap
    }

    pub fn draft(&self, text: &str) -> u64 {
        draft_size(text, self.path.unit, self.path.ratio.unwrap_or(1.0))
    }
}

/// 一桌目前的容量（`scene_budget` 指令的回傳主體）。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneBudget {
    pub scene: u64,
    /// None＝拿不到上限（自訂 base_url）：不提醒、不鎖
    pub summary: Option<SummaryBudget>,
    /// 任一聊天路徑到了提醒門檻
    pub chat_hint: bool,
}

/// 讀檔、組裝、算容量。`config_root` 放設定、模型目錄與穩定免費快取；`root` 是資料根（容量快取、lane 線）。
/// `with_chat`：關卡只需要換幕那條，不必組每條聊天路徑。
/// `config` 是呼叫端讀好的同一份設定快照：容量、世代與指紋都從它來，不分開讀檔。
pub fn compute(
    config_root: &Path,
    root: &Path,
    world_id: &str,
    config: &AppConfig,
    with_chat: bool,
) -> Result<SceneBudget, String> {
    let config = config.clone();
    let lang = crate::transport::ui_language(&config);
    let materials = measure::load(root, world_id)?;
    let store = capacity::read(&capacity::path(root));
    let catalog = data::read_model_catalog(config_root).unwrap_or_default();
    let (codex_cache, codex_config) =
        match crate::transport::dispatch::chat_transport(&config).as_str() {
            "codex" => limits::read_codex_files(),
            _ => (None, None),
        };
    let grok_windows = match crate::transport::dispatch::chat_transport(&config).as_str() {
        "grok" => {
            limits::read_grok_windows(&crate::transport::dispatch::grok_home_dir(config_root))
        }
        _ => None,
    };
    let sources = limits::Sources {
        capacity: &store,
        catalog: &catalog,
        codex_cache: codex_cache.as_deref(),
        codex_config: codex_config.as_deref(),
        grok_windows: grok_windows.as_ref(),
        smart_free_context: crate::smart_free::is_active(&config)
            .then(|| crate::smart_free::candidate_max_context(config_root, &config))
            .flatten(),
    };
    Ok(compute_with(
        &config, &lang, root, world_id, &materials, &sources, with_chat,
    ))
}

fn compute_with(
    config: &AppConfig,
    lang: &str,
    root: &Path,
    world_id: &str,
    materials: &measure::Materials,
    sources: &limits::Sources,
    with_chat: bool,
) -> SceneBudget {
    let transport = crate::transport::dispatch::chat_transport(config);
    let gm_tier = crate::transport::gm_tier(config);
    let summary = limits::resolve(config, gm_tier, sources).map(|limit| {
        let takeover =
            data::is_interface_takeover(root, world_id, materials.state.refactor_mode.as_deref());
        let raw = measure::summary_raw(&materials.events, lang, &transport, takeover, limit.unit);
        let path = path_budget(&limit, "summary", raw, None);
        let factor = path.ratio.unwrap_or(1.0);
        let sizes: Vec<ReplySize> = materials
            .events
            .iter()
            .map(|event| ReplySize {
                player: event.kind == TranscriptKind::Player,
                action: event.action_id.as_deref(),
                size: scale(measure::event_summary_size(event, lang, limit.unit), factor),
            })
            .collect();
        let g_reply = predict_reply(&sizes, limit.unit);
        SummaryBudget {
            lockable: limit.reliable && path.ratio.is_some(),
            over: path.used > path.cap,
            g_reply,
            path,
        }
    });
    let provider = crate::transport::dispatch::lane_provider(config);
    let chat_hint = with_chat
        && measure::chat_paths(
            root, world_id, materials, gm_tier, provider, lang, &transport,
        )
        .into_iter()
        .any(|chat| {
            let Some(limit) = limits::resolve(config, chat.tier, sources) else {
                return false;
            };
            let raw = Raw {
                full: measure::size(&chat.request_full, limit.unit),
                fixed: measure::size(&chat.request_fixed, limit.unit),
            };
            let reported = match (provider, &chat.lane, limit.unit) {
                (Some(provider), Some((lane, scope)), Unit::Tokens) => {
                    crate::lanes::last_prompt_tokens(
                        root,
                        world_id,
                        *lane,
                        provider,
                        &limit.model,
                        scope.as_deref(),
                        materials.state.current_scene,
                    )
                }
                _ => None,
            };
            path_budget(&limit, chat.kind, raw, reported).hint
        });
    SceneBudget {
        scene: materials.state.current_scene,
        summary,
        chat_hint,
    }
}

/// 換幕容量鎖的後端關卡（呼叫端必須已持有桌級寫入許可）。滿了回 `SceneCapacityFull`。
pub fn check_capacity(
    config_root: &Path,
    root: &Path,
    world_id: &str,
    action_id: Option<&str>,
    draft: &str,
) -> Result<(), String> {
    // 世代先讀：之後才改的設定會推進世代，這張收據就作廢
    let epoch = data::capacity_epoch();
    let config = data::read_config(config_root).map_err(|error| error.to_string())?;
    let config_gen = config_generation(&config);
    let scene = data::read_state(root, world_id)
        .map_err(|error| error.to_string())?
        .current_scene;
    if let Some(action_id) = action_id {
        if gate::has_receipt(world_id, action_id, epoch, scene, &config_gen) {
            return Ok(());
        }
    }
    let budget = compute(config_root, root, world_id, &config, false)?;
    let snapshot = gate::Snapshot {
        epoch,
        scene: budget.scene,
        config_gen,
        summary: budget.summary,
    };
    match gate::admit(world_id, action_id, draft, &snapshot) {
        true => Ok(()),
        false => Err(crate::ui_msg::UiMsg::SceneCapacityFull.into()),
    }
}

/// 換幕摘要用的模型字串（capacity 的鍵，與容量判定同一個解析）；拿不到上限的後端回 None。
pub fn summary_model(config_root: &Path, root: &Path, config: &AppConfig) -> Option<String> {
    summary_limit(config_root, root, config).map(|limit| limit.model)
}

fn summary_limit(config_root: &Path, root: &Path, config: &AppConfig) -> Option<limits::Limit> {
    let store = capacity::read(&capacity::path(root));
    let catalog = data::read_model_catalog(config_root).unwrap_or_default();
    let (codex_cache, codex_config) =
        match crate::transport::dispatch::chat_transport(config).as_str() {
            "codex" => limits::read_codex_files(),
            _ => (None, None),
        };
    let grok_windows = match crate::transport::dispatch::chat_transport(config).as_str() {
        "grok" => {
            limits::read_grok_windows(&crate::transport::dispatch::grok_home_dir(config_root))
        }
        _ => None,
    };
    let sources = limits::Sources {
        capacity: &store,
        catalog: &catalog,
        codex_cache: codex_cache.as_deref(),
        codex_config: codex_config.as_deref(),
        grok_windows: grok_windows.as_ref(),
        smart_free_context: crate::smart_free::is_active(config)
            .then(|| crate::smart_free::candidate_max_context(config_root, config))
            .flatten(),
    };
    limits::resolve(config, crate::transport::gm_tier(config), &sources)
}

/// 換幕摘要能用的容量（分段摘要用）：上限未知時 total 為 None，先整段送，收到「太長」才縮。
/// 猜的預設值（不可靠）也拿來當切塊起點——猜錯有「太長」縮塊接住。
pub fn summary_capacity(
    config_root: &Path,
    root: &Path,
    config: &AppConfig,
) -> summarize::Capacity {
    let transport = crate::transport::dispatch::chat_transport(config);
    let limit = summary_limit(config_root, root, config);
    summarize::Capacity {
        unit: limit.as_ref().map_or(Unit::Tokens, |limit| limit.unit),
        total: limit.as_ref().map(|limit| limit.total),
        reserve: limit
            .as_ref()
            .map_or(limits::OUTPUT_RESERVE, |limit| limit.reserve),
        ratio: limit
            .as_ref()
            .and_then(|limit| limit.ratio("summary"))
            .unwrap_or(1.0),
        transport,
        lang: crate::transport::ui_language(config),
    }
}

/// 一次換幕摘要類請求的估計（送出前算、與實報配成摘要種類的校正）。
pub fn summary_request_estimate(
    messages: &[crate::transport::ChatMessage],
    lang: &str,
    transport: &str,
) -> u64 {
    measure::size(
        &measure::summary_request_of(messages.to_vec(), lang, transport),
        Unit::Tokens,
    )
}

/// 設定世代：設定檔整份的指紋。換傳輸、換模型、換檔位都會變；前端據此丟掉舊世代的容量。
pub fn config_generation(config: &AppConfig) -> String {
    crate::usage::log::text_hash(&serde_json::to_string(config).unwrap_or_default())
}

/// 前端認定的「目前設定」是不是就是這次量測用的那份快照（前端樂觀更新還沒寫成時就不是）。
pub fn snapshot_matches(snapshot: &AppConfig, expected: &serde_json::Value) -> bool {
    serde_json::to_value(snapshot).is_ok_and(|value| &value == expected)
}

#[cfg(test)]
mod tests;
