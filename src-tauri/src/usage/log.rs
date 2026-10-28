//! 快取／花費紀錄（prompt-cache-optimization 包 4）：一次呼叫一行 JSONL，
//! 落在資料目錄的 `prompt-cache.jsonl`，供設定頁的額度分頁（包 6）直接讀。
//! 線的動作（重開／補丁／追平）與該次呼叫的用量寫在同一筆——分成兩種行時，
//! 「命中率為什麼掉」得靠時間戳自己接，接錯就誤診（三輪 0% 的誤判即出於此）。
//!
//! `diag`（這輪呼叫怎麼跑的）與 `cache_reporting`（這條路看不看得見快取）是**正交**的兩軸，
//! 分開記才不會拿「單發」去解釋「數字是 0」：
//! - `cache_reporting: "reported"`＝供應商回了快取欄位，`cached_tokens` 是真數字（0 就是真的沒中）。
//! - `cache_reporting: "absent"`＝這條路沒回報，`cached_tokens`／`hit_rate` 整個欄位不寫，
//!   額度分頁顯示「—」而非 0%（曾用 `unwrap_or(0)` 壓平，見 .ai/plans/api-cache-visibility.md）。
//!
//! 兩個標籤各管一軸，純規則判定（不靠 AI）。每個值對應一句給玩家看的話，額度分頁照這張表配 i18n：
//!
//! `mode`＝這通呼叫送出去的形狀：
//! - `resume`：claude 續聊線的劇情輪，只送新事件。
//! - `shared`：無狀態路徑的共線劇情輪，全角色名單當共同前綴。
//! - `solo`：只帶本輪角色的劇情輪。天然單角色桌與（日後）零命中退回都是這個形狀，
//!   靠 `roster_size` 分辨：等於 1＝這桌本來就一個人，大於 1＝策略退回。
//! - `oneshot`：不建立續輪期待的一次性呼叫（換幕摘要、開桌生成、卡重構）。
//! - 舊帳本另有 `ping`（已撤的保溫呼叫），報表只把它的花費算進合計。
//!
//! `cache`＝這通呼叫的快取結果。算得出「理論可中量」的路徑（只有 claude 續聊線）判得比較細：
//! - `hit`：中了，且達到理論可中量的九成以上；無理論值的路徑則是「中了」。
//! - `partial`：中了但遠低於理論可中量——只有 claude 續聊線產得出來。
//! - `zero`：供應商有回報，回報的值就是 0。
//! - `unknown`：這條路沒回報快取欄位，`cached_tokens`／`hit_rate` 整個欄位不寫，
//!   額度分頁顯示「—」而非 0%（曾用 `unwrap_or(0)` 壓平，見 .ai/plans/api-cache-visibility.md）。
//! - `not-expected`：新桌／新線，本來就沒有可中的東西（首輪、剛重開線）。
//!
//! `cache_reason` 只在 `partial`／`zero` 時補一句為什麼，同樣只有續聊線給得出；
//! `zero` 的原因只有 claude 續聊線給，agy／grok 續聊線讀 0 不帶原因（只說這次沒有快取）。
//! `not-expected` 不帶原因——標籤本身就是原因，再補一句只會在畫面上講兩次同一件事：
//! - `expired`：距上一句超過 app 的保守快取窗口。實測超時仍可能中，所以這只是「沒中滿的解釋」，
//!   不宣稱快取確定被清掉，畫面上也不當故障標紅。
//! - `below-expected`：低於理論值——程式只知道數字不對，不宣稱前綴一定被誰改過。
//! - `skipped`：claude 續聊線讀寫皆 0（claude CLI resume 已知毛病，2026-08 實證同一線隔輪出現、整句付全額）。
//!
//! 「這輪是不是單發」與「快取有沒有中」是**正交**的兩軸，分開記才不會拿呼叫模式去解釋數字是 0
//! ——舊版 `diag` 一欄兩用，非續聊的呼叫在碰到快取數字之前就被判成 `single`，
//! 於是 155 筆命中率過半的紀錄對玩家說「整包重新送出」。
//!
//! 來源欄（可選，舊行沒有、取不到就不寫）：`cli_version`＝CLI 的 `--version` 第一行，
//! `endpoint`＝API 實際打的 `主機:連接埠#路徑雜湊`（路徑只記雜湊，不含 query 與帳密；同一主機不同連接埠或路徑可能是不同服務）。額度分頁拿它們分組認自帶底線
//! （usage/floor.rs）——自帶段隨 CLI 版本變（codex 0.164.0 起 9,984→11,008）。
//!
//! `reason`（線為什麼重開）與 `cache_reason`（快取為什麼沒中）是不同的欄位，勿共用。
//! 沒有用量數字的線事件（丟線重來）走 `append_event`，只寫 `event`＋`reason`，
//! 不偽造 mode／cache，才不會在快取統計的分母裡憑空多一筆。

use crate::lanes::LEGACY_CACHE_TTL_SECS;
use crate::transport::PromptCacheUsage;
use serde_json::{json, Map, Value};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Resume,
    Shared,
    Solo,
    Oneshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cache {
    Hit,
    Partial,
    Zero,
    Unknown,
    NotExpected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheReason {
    Expired,
    BelowExpected,
    Skipped,
}

/// 沒有用量數字的線事件種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    DropLane,
    /// 診斷用：session 檔出現 CLI 的 total_tokens 提醒（關提醒的環境變數失效）。
    /// 報表一律跳過，不算輪數、不佔最近一輪
    CliReminderSeen,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Resume => "resume",
            Self::Shared => "shared",
            Self::Solo => "solo",
            Self::Oneshot => "oneshot",
        }
    }
}

impl Cache {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Partial => "partial",
            Self::Zero => "zero",
            Self::Unknown => "unknown",
            Self::NotExpected => "not-expected",
        }
    }
}

impl CacheReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Expired => "expired",
            Self::BelowExpected => "below-expected",
            Self::Skipped => "skipped",
        }
    }
}

impl Event {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DropLane => "drop-lane",
            Self::CliReminderSeen => "cli-reminder-seen",
        }
    }
}

/// 呼叫端交給落帳的唯讀情報：這通呼叫送出去的是什麼形狀。只用來標 `mode`，
/// 不建立跨輪記憶、不影響組裝。續聊線不必傳（形狀由 `LaneContext` 自己說明）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptShape {
    /// 劇情輪。`roster` ＝**套用策略之前**這桌的有效角色數，不是實際傳給組裝器的張數
    /// ——否則退回單角色之後就看不出原本有幾個人。
    Turn { roster: usize, solo: bool },
    /// 換幕摘要、開桌生成、卡重構這類不建立續輪期待的呼叫。
    Oneshot,
    /// 圖片生成。provider 可能回報固定工具／系統前綴命中，但它不是劇情對話快取，
    /// 額度頁仍記 token 與費用，對話命中率則排除。
    Image,
}

/// 這一輪 lane 呼叫的脈絡。診斷靠 app 自己的決策（有沒有重開、隔多久、上輪送了多少），
/// 不從 token 數字反推。
#[derive(Debug, Clone)]
pub struct LaneContext {
    /// 線名，例如 `chars:sonnet`
    pub lane: String,
    /// 重開原因；None＝續聊
    pub reopen: Option<&'static str>,
    /// 素材有變、走補丁保住快取
    pub patched: bool,
    /// 素材有變且快取已死、直接換上新版凍結素材
    pub rebased: bool,
    /// 距上輪呼叫幾秒（首輪 0）
    pub age_secs: u64,
    /// 理論可中量＝上輪的總輸入；首輪／重開為 0
    pub expected_cached: u64,
    pub system_tokens: u64,
    pub system_hash: String,
    /// 呼叫前採用的快取壽命（秒）；重開為 None。判斷「隔太久過期」只看它
    pub ttl_secs: Option<u64>,
    /// 呼叫前歷史裡的舊尾段世界書（重開那行是清理前的值）；計數未知的舊線為 None
    pub stale: Option<StaleTail>,
}

/// 舊尾段世界書的累計（tail-worldbook-erase）：份數、大小（容量單位）、容量與佔比；容量未知時後兩者 None。
#[derive(Debug, Clone, Copy)]
pub struct StaleTail {
    pub copies: u32,
    pub size: u64,
    pub capacity: Option<u64>,
    pub percent: Option<f64>,
}

/// 九成以上算正常：CLI 端 token 計數與分段邊界本來就會有零頭出入。
const HIT_TOLERANCE_NUM: u64 = 9;
const HIT_TOLERANCE_DEN: u64 = 10;

/// 這通呼叫送出去的是什麼形狀。續聊線自己說明；其餘由呼叫端隨行交代。
fn classify_mode(lane: Option<&LaneContext>, shape: PromptShape) -> Mode {
    match lane {
        Some(_) => Mode::Resume,
        None => match shape {
            PromptShape::Oneshot | PromptShape::Image => Mode::Oneshot,
            PromptShape::Turn { solo: true, .. } => Mode::Solo,
            PromptShape::Turn { solo: false, .. } => Mode::Shared,
        },
    }
}

/// 快取結果。**數字優先**：中了就是中了，結構上的假設（剛重開線、隔太久）只拿來解釋
/// 「為什麼比該中的少」，不能反過來蓋掉觀測值——本案修的就是這種蓋法。
/// 有 `LaneContext` 才算得出「理論可中量」，才判得出 `partial`；無狀態路徑只描述觀測到的數字。
fn classify_cache(
    lane: Option<&LaneContext>,
    usage: &PromptCacheUsage,
    transport: &str,
) -> (Cache, Option<CacheReason>) {
    if !usage.reported() {
        return (Cache::Unknown, None);
    }
    let cached = usage.cached_tokens.unwrap_or(0);
    let created = usage.created_tokens.unwrap_or(0);
    let Some(lane) = lane else {
        // 沒有理論值：中了就是中了，0 就是 0，不猜為什麼
        return match cached > 0 {
            true => (Cache::Hit, None),
            false => (Cache::Zero, None),
        };
    };
    // 剛重開線＝我方內容一個字都不該中；供應商自己的固定前綴還是可能中
    let expected = match lane.reopen.is_some() {
        true => 0,
        false => lane.expected_cached,
    };
    if cached > 0 {
        // 沒有理論值就不評斷「夠不夠多」，只說中了
        if expected == 0 || cached * HIT_TOLERANCE_DEN >= expected * HIT_TOLERANCE_NUM {
            return (Cache::Hit, None);
        }
        return (Cache::Partial, Some(short_reason(lane)));
    }
    if expected == 0 {
        return (Cache::NotExpected, None);
    }
    // 非 claude 的續聊線（agy、grok）讀到 0 只說「這次沒有快取」，不推測原因：
    // 它們不回報寫入量，缺欄當 0 會被誤判成 claude 的 skipped（2026-10-06 對帳實證）
    if transport != "claude" {
        return (Cache::Zero, None);
    }
    // 讀寫皆 0＝這句根本沒帶快取標記，與「隔太久過期」不同
    let reason = match created == 0 {
        true => CacheReason::Skipped,
        false => short_reason(lane),
    };
    (Cache::Zero, Some(reason))
}

/// 該中而沒中滿的解釋：超過呼叫前採用的快取壽命就歸給時間，否則只能說「低於理論值」
/// （程式看得到數字不對，看不到是誰動了前綴）。
fn short_reason(lane: &LaneContext) -> CacheReason {
    match lane.age_secs > lane.ttl_secs.unwrap_or(LEGACY_CACHE_TTL_SECS) {
        true => CacheReason::Expired,
        false => CacheReason::BelowExpected,
    }
}

/// 粗估 token 數，只給診斷比對用（不用於計費）：ASCII 約四字元一個 token，中日韓約一字一個。
pub fn estimate_tokens(text: &str) -> u64 {
    let (ascii, wide) = text
        .chars()
        .fold((0u64, 0u64), |(ascii, wide), ch| match ch.is_ascii() {
            true => (ascii + 1, wide),
            false => (ascii, wide + 1),
        });
    ascii / 4 + wide
}

/// FNV-1a 64：跨執行、跨版本皆穩定，用來一眼看出兩筆紀錄的凍結素材是不是同一份。
pub fn text_hash(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn call_fields(
    world: Option<&str>,
    transport: &str,
    model: &str,
    lane: Option<&LaneContext>,
    shape: PromptShape,
    usage: PromptCacheUsage,
) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("transport".to_owned(), json!(transport));
    if let Some(world) = world {
        fields.insert("world".to_owned(), json!(world));
    }
    fields.insert("model".to_owned(), json!(model));
    fields.insert(
        "mode".to_owned(),
        json!(classify_mode(lane, shape).as_str()),
    );
    if shape == PromptShape::Image {
        fields.insert("purpose".to_owned(), json!("image"));
    }
    let (cache, cache_reason) = classify_cache(lane, &usage, transport);
    fields.insert("cache".to_owned(), json!(cache.as_str()));
    if let Some(reason) = cache_reason {
        fields.insert("cache_reason".to_owned(), json!(reason.as_str()));
    }
    // 只有呼叫端真的交代過形狀才寫（續聊線的 shape 是佔位值，mode 由 LaneContext 決定）
    if let (None, PromptShape::Turn { roster, .. }) = (lane, shape) {
        fields.insert("roster_size".to_owned(), json!(roster));
    }
    fields.insert("prompt_tokens".to_owned(), json!(usage.prompt_tokens));
    // 沒回報的欄位一個都不寫：缺欄位就是「量不到」，寫 0 會被讀成「量到了、沒中」
    fields.insert(
        "cache_reporting".to_owned(),
        json!(match usage.reported() {
            true => "reported",
            false => "absent",
        }),
    );
    if let Some(cached) = usage.cached_tokens {
        fields.insert("cached_tokens".to_owned(), json!(cached));
    }
    if let Some(created) = usage.created_tokens {
        fields.insert("created_tokens".to_owned(), json!(created));
    }
    // 只有來源給了 1h 拆分才寫；缺欄＝時效未知，報表按 5 分鐘估算並標「約」
    if let Some(one_hour) = usage.created_1h_tokens {
        fields.insert("created_1h_tokens".to_owned(), json!(one_hour));
    }
    fields.insert("output_tokens".to_owned(), json!(usage.output_tokens));
    if let Some(rate) = usage.hit_rate() {
        fields.insert("hit_rate".to_owned(), json!((rate * 10.0).round() / 10.0));
    }
    if let Some(cost) = usage.cost_usd {
        fields.insert("cost_usd".to_owned(), json!(cost));
    }
    if let Some(lane) = lane {
        fields.insert("lane".to_owned(), json!(lane.lane));
        if let Some(reason) = lane.reopen {
            fields.insert("reason".to_owned(), json!(reason));
        }
        if lane.patched {
            fields.insert("patched".to_owned(), json!(true));
        }
        if lane.rebased {
            fields.insert("rebased".to_owned(), json!(true));
        }
        fields.insert("age_secs".to_owned(), json!(lane.age_secs));
        fields.insert("expected_cached".to_owned(), json!(lane.expected_cached));
        if let Some(ttl) = lane.ttl_secs {
            fields.insert("ttl_secs".to_owned(), json!(ttl));
        }
        fields.insert("system_tokens".to_owned(), json!(lane.system_tokens));
        fields.insert("system_hash".to_owned(), json!(lane.system_hash));
        if let Some(stale) = lane.stale {
            fields.insert("stale_copies".to_owned(), json!(stale.copies));
            fields.insert("stale_size".to_owned(), json!(stale.size));
            fields.insert("stale_capacity".to_owned(), json!(stale.capacity));
            fields.insert(
                "stale_ratio".to_owned(),
                json!(stale.percent.map(|percent| (percent * 10.0).round() / 10.0)),
            );
        }
    }
    fields
}

/// 來源欄：CLI 版本與 API 端點（`endpoint_id`），取不到（None 或空字串）就不寫，報表歸「未知」那組。
pub(crate) fn origin_fields(
    cli_version: Option<&str>,
    endpoint: Option<&str>,
) -> Map<String, Value> {
    let mut fields = Map::new();
    for (key, value) in [("cli_version", cli_version), ("endpoint", endpoint)] {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            fields.insert(key.to_owned(), json!(value));
        }
    }
    fields
}

/// API 端點識別：`主機:連接埠#路徑雜湊`。沒寫連接埠就補協定預設值（https://x 與 https://x:443 同一組）；
/// 同一閘道的不同路徑可能代理不同供應商，所以路徑要分組，但路徑本身不落帳（有些閘道把金鑰放在路徑裡），
/// 只記去掉 query、fragment、帳密後的路徑雜湊前 8 碼。
pub(crate) fn endpoint_id(url: &str) -> Option<String> {
    let url = reqwest::Url::parse(url).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let port = url.port_or_known_default()?;
    let path = text_hash(url.path());
    Some(format!("{host}:{port}#{}", &path[..8]))
}

/// 一次呼叫一行。`lane` 為 None＝無狀態路徑（API／codex／agy／grok）與 claude 的一次性呼叫；
/// 形狀由 `shape` 交代，快取結果照樣判（舊版在這裡短路成「單發」，見模組頂註解）。
/// world 為 None＝不屬於任何一桌（開桌生成）；加欄前的舊行同樣沒有，額度分頁歸「未標桌」。
/// `extra` 是附加欄：來源欄（`origin_fields`），以及 agy 的原始計數證據；皆為白名單，不寫 prompt 或 response。
#[allow(clippy::too_many_arguments)]
pub(crate) fn append_call(
    path: &Path,
    world: Option<&str>,
    transport: &str,
    model: &str,
    lane: Option<&LaneContext>,
    shape: PromptShape,
    usage: PromptCacheUsage,
    extra: &Map<String, Value>,
) {
    let mut fields = call_fields(world, transport, model, lane, shape, usage);
    for (key, value) in extra {
        fields.insert(key.clone(), value.clone());
    }
    append(path, fields);
}

/// 開桌生成的呼叫發生在桌建出來之前，落檔當下還不知道桌 id。桌一建好就把還沒認領的行
/// （沒有 `world` 欄的）補上——那些額度就是為這桌花的。前一次半途放棄的開桌嘗試若還留著
/// 未認領的行，會一起算進這桌：同樣是開桌花的錢，比留一個看不懂的分類好。
/// 暫存檔＋rename，中途失敗不會把 log 寫壞；任何一步失敗就放著不動。
pub fn assign_pending_world(path: &Path, world: &str) {
    // 這一步發生在建桌許可已經放下之後。開閘就留著未認領的行，不改用量紀錄。
    if crate::data::update_gate_raised() {
        return;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let mut changed = false;
    let claimed: Vec<String> = text
        .lines()
        .map(|line| match serde_json::from_str::<Value>(line) {
            Ok(Value::Object(mut fields)) if !fields.contains_key("world") => {
                fields.insert("world".to_owned(), json!(world));
                changed = true;
                serde_json::to_string(&Value::Object(fields)).unwrap_or_else(|_| line.to_owned())
            }
            _ => line.to_owned(),
        })
        .collect();
    if !changed {
        return;
    }
    let temporary = path.with_extension("jsonl.tmp");
    if std::fs::write(&temporary, claimed.join("\n") + "\n").is_ok() {
        let _ = std::fs::rename(&temporary, path);
    }
}

/// 沒有用量數字的線事件（目前只有抹寫失敗丟線）。不寫 mode／cache——
/// 它不是一通呼叫，混進快取統計會憑空多一筆「量不到」。
/// `transport` 是這條線的實際 provider（claude／grok／agy）。
pub fn append_event(
    path: &Path,
    transport: &str,
    world: Option<&str>,
    lane: &str,
    event: Event,
    reason: &str,
) {
    append(path, event_fields(transport, world, lane, event, reason));
}

fn event_fields(
    transport: &str,
    world: Option<&str>,
    lane: &str,
    event: Event,
    reason: &str,
) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("transport".to_owned(), json!(transport));
    if let Some(world) = world {
        fields.insert("world".to_owned(), json!(world));
    }
    fields.insert("lane".to_owned(), json!(lane));
    fields.insert("event".to_owned(), json!(event.as_str()));
    fields.insert("reason".to_owned(), json!(reason));
    fields
}

/// 抹寫／截尾失敗的丟線：比 `append_event` 多記壞在哪一步（`stage`）與原錯誤字串（`detail`，
/// 呼叫端已遮路徑、截斷）。`reason` 照舊，舊帳本與額度分頁的讀法不變。
pub fn append_drop(
    path: &Path,
    transport: &str,
    world: Option<&str>,
    lane: &str,
    reason: &str,
    stage: &str,
    detail: &str,
) {
    let mut fields = event_fields(transport, world, lane, Event::DropLane, reason);
    fields.insert("stage".to_owned(), json!(stage));
    fields.insert("detail".to_owned(), json!(detail));
    append(path, fields);
}

/// 每筆補上時間戳（serde_json 的 Map 是 BTreeMap，實際輸出照鍵的字母序）。
/// 寫檔失敗一律吞掉：診斷設施不該反過來中斷回覆。
fn append(path: &Path, fields: Map<String, Value>) {
    let timestamp =
        crate::data::local_timestamp_seconds().unwrap_or_else(|_| "unknown-time".to_owned());
    let mut line = Map::new();
    line.insert("ts".to_owned(), json!(timestamp));
    line.extend(fields);
    let Ok(text) = serde_json::to_string(&Value::Object(line)) else {
        return;
    };
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| writeln!(file, "{text}"));
}

#[cfg(test)]
mod tests;
