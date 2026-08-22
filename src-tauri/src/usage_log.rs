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
//! - `ping`：保溫呼叫（包 7），不推進劇情、只為刷新快取壽命；花費與劇情輪分開統計。
//!
//! `cache`＝這通呼叫的快取結果。算得出「理論可中量」的路徑（只有 claude 續聊線）判得比較細：
//! - `hit`：中了，且達到理論可中量的九成以上；無理論值的路徑則是「中了」。
//! - `partial`：中了但遠低於理論可中量——只有 claude 續聊線產得出來。
//! - `zero`：供應商有回報，回報的值就是 0。
//! - `unknown`：這條路沒回報快取欄位，`cached_tokens`／`hit_rate` 整個欄位不寫，
//!   額度分頁顯示「—」而非 0%（曾用 `unwrap_or(0)` 壓平，見 .ai/plans/api-cache-visibility.md）。
//! - `not-expected`：這輪本來就沒有可中的東西（首輪、剛重開線）。
//!
//! `cache_reason` 只在 `partial`／`zero` 時補一句為什麼，同樣只有續聊線給得出。
//! `not-expected` 不帶原因——標籤本身就是原因，再補一句只會在畫面上講兩次同一件事：
//! - `expired`：距上一句超過 app 的保守快取窗口。實測超時仍可能中，所以這只是「沒中滿的解釋」，
//!   不宣稱快取確定被清掉，畫面上也不當故障標紅。
//! - `below-expected`：低於理論值——程式只知道數字不對，不宣稱前綴一定被誰改過。
//! - `skipped`：讀寫皆 0（claude CLI resume 已知毛病，2026-08 實證同一線隔輪出現、整句付全額）。
//!
//! 「這輪是不是單發」與「快取有沒有中」是**正交**的兩軸，分開記才不會拿呼叫模式去解釋數字是 0
//! ——舊版 `diag` 一欄兩用，非續聊的呼叫在碰到快取數字之前就被判成 `single`，
//! 於是 155 筆命中率過半的紀錄對玩家說「整包重新送出」。
//!
//! `reason`（線為什麼重開）與 `cache_reason`（快取為什麼沒中）是不同的欄位，勿共用。
//! 沒有用量數字的線事件（丟線重來）走 `append_event`，只寫 `event`＋`reason`，
//! 不偽造 mode／cache，才不會在快取統計的分母裡憑空多一筆。

use crate::lanes::CACHE_TTL_SECS;
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
    Ping,
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
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Resume => "resume",
            Self::Shared => "shared",
            Self::Solo => "solo",
            Self::Oneshot => "oneshot",
            Self::Ping => "ping",
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
    /// 保溫 ping（包 7）：不推進劇情，花費與劇情輪分開統計
    pub ping: bool,
}

/// 九成以上算正常：CLI 端 token 計數與分段邊界本來就會有零頭出入。
const HIT_TOLERANCE_NUM: u64 = 9;
const HIT_TOLERANCE_DEN: u64 = 10;

/// 這通呼叫送出去的是什麼形狀。續聊線自己說明；其餘由呼叫端隨行交代。
fn classify_mode(lane: Option<&LaneContext>, shape: PromptShape) -> Mode {
    match lane {
        Some(lane) if lane.ping => Mode::Ping,
        Some(_) => Mode::Resume,
        None => match shape {
            PromptShape::Oneshot => Mode::Oneshot,
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
    // 讀寫皆 0＝這句根本沒帶快取標記，與「隔太久過期」不同
    let reason = match created == 0 {
        true => CacheReason::Skipped,
        false => short_reason(lane),
    };
    (Cache::Zero, Some(reason))
}

/// 該中而沒中滿的解釋：超過保守窗口就歸給時間，否則只能說「低於理論值」
/// （程式看得到數字不對，看不到是誰動了前綴）。
fn short_reason(lane: &LaneContext) -> CacheReason {
    match lane.age_secs > CACHE_TTL_SECS {
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

/// 一次呼叫一行。`lane` 為 None＝無狀態路徑（API／codex／agy／grok）與 claude 的一次性呼叫；
/// 形狀由 `shape` 交代，快取結果照樣判（舊版在這裡短路成「單發」，見模組頂註解）。
/// world 為 None＝不屬於任何一桌（開桌生成）；加欄前的舊行同樣沒有，額度分頁歸「未標桌」。
#[allow(clippy::too_many_arguments)]
pub fn append_call(
    path: &Path,
    world: Option<&str>,
    transport: &str,
    model: &str,
    lane: Option<&LaneContext>,
    shape: PromptShape,
    usage: PromptCacheUsage,
) {
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
    let (cache, cache_reason) = classify_cache(lane, &usage);
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
        fields.insert("system_tokens".to_owned(), json!(lane.system_tokens));
        fields.insert("system_hash".to_owned(), json!(lane.system_hash));
    }
    append(path, fields);
}

/// 開桌生成的呼叫發生在桌建出來之前，落檔當下還不知道桌 id。桌一建好就把還沒認領的行
/// （沒有 `world` 欄的）補上——那些額度就是為這桌花的。前一次半途放棄的開桌嘗試若還留著
/// 未認領的行，會一起算進這桌：同樣是開桌花的錢，比留一個看不懂的分類好。
/// 暫存檔＋rename，中途失敗不會把 log 寫壞；任何一步失敗就放著不動。
pub fn assign_pending_world(path: &Path, world: &str) {
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
pub fn append_event(path: &Path, world: Option<&str>, lane: &str, event: Event, reason: &str) {
    let mut fields = Map::new();
    fields.insert("transport".to_owned(), json!("claude"));
    if let Some(world) = world {
        fields.insert("world".to_owned(), json!(world));
    }
    fields.insert("lane".to_owned(), json!(lane));
    fields.insert("event".to_owned(), json!(event.as_str()));
    fields.insert("reason".to_owned(), json!(reason));
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
mod tests {
    use super::*;

    fn usage(prompt: u64, cached: u64, created: u64) -> PromptCacheUsage {
        PromptCacheUsage {
            prompt_tokens: prompt,
            cached_tokens: Some(cached),
            created_tokens: Some(created),
            output_tokens: 20,
            cost_usd: Some(0.0031),
        }
    }

    fn lane(age_secs: u64, expected_cached: u64) -> LaneContext {
        LaneContext {
            lane: "chars:sonnet".to_owned(),
            reopen: None,
            patched: false,
            rebased: false,
            age_secs,
            expected_cached,
            system_tokens: 4_000,
            system_hash: text_hash("凍結素材"),
            ping: false,
        }
    }

    fn turn(roster: usize) -> PromptShape {
        PromptShape::Turn {
            roster,
            solo: roster <= 1,
        }
    }

    /// 快取軸是有限清單，判定只看 app 自己的決策＋數字，不猜。
    #[test]
    fn cache_axis_covers_each_label_by_rule() {
        // 續聊、幾乎全中
        assert_eq!(
            classify_cache(Some(&lane(30, 9_000)), &usage(9_300, 9_000, 300)),
            (Cache::Hit, None)
        );
        // 上輪送 9000，這輪只中 200＝中了但遠低於理論值
        assert_eq!(
            classify_cache(Some(&lane(30, 9_000)), &usage(9_300, 200, 9_100)),
            (Cache::Partial, Some(CacheReason::BelowExpected))
        );
        // 超過保守窗口且一個字都沒中＝歸給時間，不算故障
        assert_eq!(
            classify_cache(Some(&lane(600, 9_000)), &usage(9_300, 0, 9_300)),
            (Cache::Zero, Some(CacheReason::Expired))
        );
        // 讀寫皆 0 但上輪送過內容＝CLI 這句沒帶標記，優先於過期判定
        assert_eq!(
            classify_cache(Some(&lane(600, 9_000)), &usage(9_300, 0, 0)),
            (Cache::Zero, Some(CacheReason::Skipped))
        );
        // 上輪沒有可中量＝這輪本來就不該中
        assert_eq!(
            classify_cache(Some(&lane(30, 0)), &usage(9_300, 0, 0)),
            (Cache::NotExpected, None)
        );
        // 重開線＝我方內容不該中；一個字都沒中就是「本來就沒得中」
        let mut reopened = lane(30, 0);
        reopened.reopen = Some("scene-changed");
        assert_eq!(
            classify_cache(Some(&reopened), &usage(9_300, 0, 9_300)),
            (Cache::NotExpected, None)
        );
        // 但重開線照樣可能中到供應商自己的固定前綴——實測帳本有 11 筆 warmup 中了六到九成。
        // 數字說中了就說中了，不能因為「這輪不該中」把 95.7% 講成「本來就沒得中」
        assert_eq!(
            classify_cache(Some(&reopened), &usage(19_300, 18_499, 800)),
            (Cache::Hit, None)
        );
        // 隔了一小時但幾乎全中＝快取其實還活著，過期只是解釋沒中的理由，不是蓋章
        assert_eq!(
            classify_cache(Some(&lane(3_600, 30_184)), &usage(37_000, 30_178, 6_800)),
            (Cache::Hit, None)
        );
        // 沒回報＝量不到，與「量到了、是 0」不同
        let mut blind = usage(9_300, 0, 0);
        blind.cached_tokens = None;
        blind.created_tokens = None;
        assert_eq!(classify_cache(None, &blind), (Cache::Unknown, None));
    }

    /// 本案的病灶：無狀態路徑以前在碰到快取數字之前就被判成「單發」，
    /// 於是命中率過半的輪次照樣對玩家說「整包重新送出」。
    #[test]
    fn stateless_paths_report_real_cache_result_not_call_mode() {
        // 共線劇情輪中了八成——舊版標 single，現在快取軸誠實說 hit
        assert_eq!(
            classify_cache(None, &usage(9_300, 7_500, 1_800)),
            (Cache::Hit, None)
        );
        // 有回報、值就是 0：說 zero，但不宣稱原因（無狀態算不出理論值）
        assert_eq!(classify_cache(None, &usage(4_411, 0, 4_411)), (Cache::Zero, None));
    }

    /// mode 只描述形狀，與快取結果互不干涉。
    #[test]
    fn mode_separates_call_shape_from_cache_result() {
        assert_eq!(classify_mode(Some(&lane(30, 9_000)), turn(4)), Mode::Resume);
        let mut ping = lane(200, 9_000);
        ping.ping = true;
        assert_eq!(classify_mode(Some(&ping), turn(4)), Mode::Ping);
        assert_eq!(classify_mode(None, turn(4)), Mode::Shared);
        // 天然單角色桌與（日後）策略退回同樣是 solo，靠 roster_size 分辨
        assert_eq!(classify_mode(None, turn(1)), Mode::Solo);
        assert_eq!(
            classify_mode(None, PromptShape::Turn { roster: 4, solo: true }),
            Mode::Solo
        );
        assert_eq!(classify_mode(None, PromptShape::Oneshot), Mode::Oneshot);
    }

    /// 量不到的那一輪：cache_reporting 標 absent，快取數字與命中率**一個欄位都不寫**。
    /// 寫成 0 會被報表讀成「量到了、沒中」——API 路的假 0.0% 就是這樣來的。
    #[test]
    fn unreported_cache_writes_no_number_fields() {
        let dir = std::env::temp_dir().join(format!("tt-usage-absent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("prompt-cache.jsonl");

        append_call(
            &path,
            None,
            "api",
            "vendor/model",
            None,
            turn(3),
            PromptCacheUsage {
                prompt_tokens: 4_690,
                cached_tokens: None,
                created_tokens: None,
                output_tokens: 551,
                cost_usd: None,
            },
        );
        // 對照組：同一條路回報了、值是 0＝量到了沒中，欄位照寫
        append_call(
            &path,
            None,
            "api",
            "vendor/model",
            None,
            turn(3),
            PromptCacheUsage {
                prompt_tokens: 2_495,
                cached_tokens: Some(0),
                created_tokens: None,
                output_tokens: 16,
                cost_usd: None,
            },
        );
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let lines: Vec<&str> = text.lines().collect();

        let absent: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(absent["cache_reporting"], json!("absent"));
        assert!(absent.get("cached_tokens").is_none());
        assert!(absent.get("created_tokens").is_none());
        assert!(absent.get("hit_rate").is_none());
        assert_eq!(absent["prompt_tokens"], json!(4_690)); // 花費照記，只是快取看不見

        let reported: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(reported["cache_reporting"], json!("reported"));
        assert_eq!(reported["cached_tokens"], json!(0));
        assert_eq!(reported["hit_rate"], json!(0.0));
        assert!(reported.get("created_tokens").is_none()); // 寫入數沒回報就不寫
    }

    #[test]
    fn call_line_is_one_json_object_with_lane_and_cost() {
        let dir = std::env::temp_dir().join(format!("tt-usage-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("prompt-cache.jsonl");

        let mut patched = lane(42, 9_000);
        patched.patched = true;
        append_call(
            &path,
            Some("w1"),
            "claude",
            "sonnet",
            Some(&patched),
            turn(3),
            usage(9_300, 9_000, 300),
        );
        append_event(
            &path,
            Some("w1"),
            "chars:sonnet",
            Event::DropLane,
            "rewrite-failed",
        );

        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);

        let call: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(call["ts"].as_str().unwrap().len(), 19); // 秒級：分不出五分鐘過期線就沒用
        assert_eq!(call["transport"], json!("claude"));
        assert_eq!(call["world"], json!("w1"));
        assert_eq!(call["lane"], json!("chars:sonnet"));
        assert_eq!(call["mode"], json!("resume"));
        assert_eq!(call["cache"], json!("hit"));
        assert!(call.get("cache_reason").is_none()); // 中了就不必解釋為什麼沒中
        assert!(call.get("roster_size").is_none()); // 續聊線沒交代過角色數，就不寫
        assert_eq!(call["prompt_tokens"], json!(9_300));
        assert_eq!(call["cached_tokens"], json!(9_000));
        assert_eq!(call["output_tokens"], json!(20));
        assert_eq!(call["hit_rate"], json!(96.8));
        assert_eq!(call["cost_usd"], json!(0.0031));
        assert_eq!(call["expected_cached"], json!(9_000));
        assert_eq!(call["age_secs"], json!(42));
        assert_eq!(call["patched"], json!(true));
        assert!(call.get("rebased").is_none()); // 沒發生的旗標不佔位
        assert!(call.get("reason").is_none());

        let event: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(event["event"], json!("drop-lane"));
        // 事件行不偽造 mode／cache，否則快取統計會憑空多一筆
        assert!(event.get("mode").is_none() && event.get("cache").is_none());
        assert_eq!(event["reason"], json!("rewrite-failed"));
        assert!(event.get("prompt_tokens").is_none());
    }

    /// 開桌生成落檔時桌還沒建出來；桌一建好，那幾行要認到這桌名下，已經有桌的不受影響。
    #[test]
    fn pending_lines_are_claimed_by_the_table_they_created() {
        let dir = std::env::temp_dir().join(format!("tt-usage-claim-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("prompt-cache.jsonl");

        append_call(&path, None, "claude", "opus", None, PromptShape::Oneshot, usage(500, 0, 0)); // 開桌大綱
        append_call(&path, None, "claude", "opus", None, PromptShape::Oneshot, usage(900, 0, 0)); // 開桌展開
        append_call(&path, Some("w9"), "claude", "sonnet", None, turn(2), usage(100, 0, 0)); // 別桌，不該被動到
        assign_pending_world(&path, "w1");

        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let worlds: Vec<String> = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap()["world"].to_string())
            .collect();
        assert_eq!(worlds, ["\"w1\"", "\"w1\"", "\"w9\""]);
        // 其餘欄位原樣保留
        let first: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(first["prompt_tokens"], json!(500));
        assert_eq!(first["mode"], json!("oneshot"));
    }

    /// 中日韓一字約一 token、英數約四字元一 token；只求同一個數量級。
    #[test]
    fn token_estimate_separates_wide_and_ascii() {
        assert_eq!(estimate_tokens("你好世界"), 4);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
        assert_eq!(estimate_tokens(""), 0);
        assert_ne!(text_hash("凍結A"), text_hash("凍結B"));
        assert_eq!(text_hash("凍結A"), text_hash("凍結A"));
    }
}
