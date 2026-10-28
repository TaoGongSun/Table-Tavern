//! 額度分頁（prompt-cache-optimization 包 6）：把 `prompt-cache.jsonl` 彙總成
//! 「一桌一份、桌內按模型分行」的報表給設定頁顯示。
//!
//! 兩條規矩：
//! - **token 是主軸**：四家 CLI 的 token 語意已在 cli.rs 換算成同一把尺，可以直接相加比較。
//! - **第一眼只講省下多少**：收合處出「已省幾成、省了多少錢」（總花費看了只會焦慮），
//!   花費留在細項。
//! - **金額只轉述**：app 不自算牌價、不建價目表，`cost_usd` 照舊是各 CLI 自己回報值的加總；
//!   省下的錢再拿它反推該輪的輸入單價乘回省下的 token，估不出的輪次標 `saved_partial`。
//!
//! 診斷標籤與原因代碼原樣送到前端配 i18n（字典見 usage/log.rs 模組頂註解）。
//!
//! 自帶底線（usage/floor.rs）在讀帳時套上：先掃全檔分組算底線，再逐輪判定，舊帳本自動回溯、
//! 選哪一桌結果都一樣。只中到底線的輪次另成一格 `vendor-only`（API 為 `vendor-only-api`）。
//! 「已省」每輪先扣掉 min(讀到, 底線)，只算我方內容讀到的部分（作者裁決 2026-10-28，B2）；
//! 原始用量、花費與明細數字照列。claude 通道不認底線（見 `floors`）。

use super::floor::{Floor, FloorKey, FloorSamples, Floors};
use serde::Serialize;
use serde_json::Value;

/// 下拉選單的一項：app 現有的每一桌都在（沒紀錄就 0 輪）。
/// 另有兩種沒有名字的項目——`id` 空字串＝開桌生成（那時桌還沒建出來），
/// `id` 有值但 `name` 空＝這桌已經被刪掉，紀錄還在。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WorldOption {
    pub id: String,
    pub name: String,
    pub rounds: u64,
}

/// 一列小計。總計列的 `source`／`model` 為空字串。
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct UsageRow {
    pub source: String,
    pub model: String,
    pub rounds: u64,
    /// 可列入對話快取判讀的呼叫數；生圖仍算 rounds，但不進這個分母。
    pub cache_rounds: u64,
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub output_tokens: u64,
    /// 命中率；這一列沒有任何可判讀輪次＝None（前端顯示「—」，不是 0%）
    pub hit_rate: Option<f64>,
    /// 供應商有回報快取欄位的輪數。少於 rounds 時前端加註「可判讀 N/M 輪」
    pub observed_rounds: u64,
    /// 上面那些輪次的輸入量＝命中率的分母。量不到的輪次不進來，才不會稀釋命中率
    pub observed_prompt_tokens: u64,
    /// 各 CLI 官方回報值的加總；一筆都沒有＝None（前端顯示「—」）
    pub cost_usd: Option<f64>,
    /// 有輪次沒回報金額，加總只是部分
    pub cost_partial: bool,
    /// 快取省下的輸入等值 token：沒快取要付的（全額）減掉實際付的
    pub saved_tokens: f64,
    /// 上面那筆的分母：算得出省下多少的輸入 token（計價不明的來源不進來）
    pub priced_tokens: u64,
    /// 省下的錢；一筆都估不出＝None（前端顯示「—」）
    pub saved_usd: Option<f64>,
    /// 有輪次估不出（沒回報金額或計價不明），加總只是部分
    pub saved_partial: bool,
    /// 省下金額裡用估計係數算的輪數（claude 舊列沒有 1h 拆分、或帳本只記別名看不出世代）。
    /// 大於 0 時前端在省額前標「約」
    pub estimated_rounds: u64,
    /// 完全不回報用量的輪數（agy）
    pub unreported: u64,
    /// 這個模型是目前設定在用的
    pub in_use: bool,
    /// 已辨識底線的組裡，可判讀輪次讀到的快取中屬於自帶段的量（每輪 min(讀到, 底線)）
    pub vendor_cached_tokens: u64,
    /// 只中到自帶底線的輪數（含 API 的那幾輪）
    pub vendor_only_rounds: u64,
    /// 上面那些輪次裡走 API 的輪數；全部都是 API 時頭條改用「供應商固定開頭」的說法
    pub vendor_only_api_rounds: u64,
    /// 讀到快取、且不是只中到自帶底線的輪數（我方內容有中，或底線未辨識）
    pub own_hit_rounds: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CacheCount {
    pub cache: String,
    pub rounds: u64,
}

/// 統計用的分類：把「快取結果」與「沒中的原因」壓成一個玩家看得懂的格子。
/// `missed` 是唯一該亮紅燈的——它代表**證明得了照理該中**（算得出理論可中量、且沒中滿），
/// 其餘一律不是故障：`zero` 只是這次沒有快取、`expired` 只代表超過 app 的保守窗口。
/// 零命中的原因（含 `skipped`）只有 claude 續聊線帶得到，非 claude 已在 `classify` 濾掉。
fn chip_state(cache: &str, cache_reason: Option<&str>) -> String {
    let faulty = matches!(cache_reason, Some("below-expected") | Some("skipped"));
    match (cache, faulty) {
        ("partial" | "zero", true) => "missed".to_owned(),
        _ => cache.to_owned(),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LatestCall {
    pub ts: String,
    /// 這通送出去的形狀；舊行推不出來就是 None（前端不顯示，不猜）
    pub mode: Option<String>,
    /// 快取結果；線事件沒有這個東西，改由 `event` 說明
    pub cache: Option<String>,
    pub cache_reason: Option<String>,
    /// 線事件種類（目前只有 drop-lane）；一般呼叫是 None
    pub event: Option<String>,
    /// 線為什麼重開／丟線，與 cache_reason 不同軸
    pub reason: Option<String>,
    /// 這一輪的快取數字看不看得見；false＝前端要在診斷句旁講明「沒回報快取資料」
    pub reported: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UsageReport {
    /// 全檔有紀錄的桌（不受選桌影響，供下拉用）
    pub worlds: Vec<WorldOption>,
    pub rows: Vec<UsageRow>,
    pub total: UsageRow,
    /// 快取結果統計。事件行（丟線重來）沒有快取結果，不進這個分母
    pub caches: Vec<CacheCount>,
    /// 線事件的次數（目前只有丟線重來）
    pub events: u64,
    /// 最近一筆劇情紀錄，供燈號與原因句
    pub latest: Option<LatestCall>,
}

/// 一行帳本 →（形狀, 快取結果, 沒中的原因）。新行直接讀 `mode`／`cache`；
/// 舊行只有一欄兩用的 `diag`，但**續聊線的舊行帶著 `expected_cached`／`age_secs`**，
/// 拿它們套同一條規則就好，不必查 diag 對照表——那張表當年正是把數字蓋掉的元凶
/// （帳本實測：11 筆 `warmup` 中了六到九成、10 筆 `expired` 中了八成以上）。
/// 形狀回推不出來就交白卷（None），不拿「單發」冒充。
///
/// 兩種格式最後都過同一道濾網：讀到 0 的原因只留給 claude 續聊線。agy／grok 不回報寫入量，
/// 舊碼把缺欄當 0 而標成 claude 的 `skipped`；帳本不改寫，讀的時候丟掉，顯示「這次沒有快取」。
fn classify(line: &Value) -> (Option<String>, String, Option<String>) {
    let (mode, cache, reason) = classify_recorded(line);
    let claude = line.get("transport").and_then(Value::as_str) == Some("claude");
    match (cache.as_str(), claude) {
        ("zero", false) => (mode, cache, None),
        _ => (mode, cache, reason),
    }
}

/// 只中到自帶底線：帳本記 `hit`、算不出理論可中量（無 lane，或重開／首輪的有效期望為 0），
/// 而且 0 < 讀到量 ≤ 底線。快取有順序，自帶段排最前面，沒中滿它，後面的我方內容就不可能中，
/// 所以低於底線（例：換角色暖的 6,912）同樣是我方 0 命中。底線之上不設容差。
/// `partial`／`missed` 與算得出理論量的續聊線判得比這細，不走這條。
fn vendor_only(line: &Value, cache: &str, floor: Floor) -> bool {
    let Floor::Known(floor) = floor else {
        return false;
    };
    let cached = number(line, "cached_tokens");
    cache == "hit" && no_expected(line) && cached > 0 && cached <= floor
}

/// 這一輪算不出理論可中量：無狀態路徑，或續聊線的有效期望為 0（重開那輪一律 0，同 log.rs 的規則）。
fn no_expected(line: &Value) -> bool {
    line.get("lane").is_none()
        || line.get("reason").is_some()
        || number(line, "expected_cached") == 0
}

/// 底線分組鍵：CLI 依版本、API 依端點（主機:連接埠）分，缺欄歸「未知」那組。
fn floor_key(line: &Value) -> FloorKey {
    let transport = text(line, "transport");
    let origin = match transport.as_str() {
        "api" => text(line, "endpoint"),
        _ => text(line, "cli_version"),
    };
    (transport, text(line, "model"), origin)
}

/// 一行是不是對話快取的樣本：真實回報、非生圖、非舊保溫 ping、非事件行。
fn dialogue_sample(line: &Value) -> bool {
    line.get("model").is_some()
        && line.get("purpose").and_then(Value::as_str) != Some("image")
        && classify_recorded(line).0.as_deref() != Some("ping")
        && reported(line)
}

/// 全檔分組算底線：不受選桌影響，樣本才夠、選桌前後判定才一致。
///
/// claude 通道不收樣本＝一律未辨識：目前的統計分不出 claude 自帶段與我方固定前綴。2026-10-28 用 app
/// 同參數實測，claude CLI 自帶的整段只有 2–3k（opus／sonnet-4 系讀不到快取），帳本判出的 3,223～11,494
/// 是我方 system 穩定命中；照扣會把真命中抹掉。命中格、命中條、頭條與省額都因此一致不套底線。
fn floors(lines: &[Value]) -> Floors {
    let mut samples = FloorSamples::default();
    let candidates = lines
        .iter()
        .filter(|line| dialogue_sample(line) && text(line, "transport") != "claude");
    for line in candidates {
        samples.add(
            floor_key(line),
            number(line, "cached_tokens"),
            number(line, "prompt_tokens"),
        );
    }
    samples.finish()
}

fn classify_recorded(line: &Value) -> (Option<String>, String, Option<String>) {
    if let Some(cache) = line.get("cache").and_then(Value::as_str) {
        return (
            line.get("mode").and_then(Value::as_str).map(str::to_owned),
            cache.to_owned(),
            line.get("cache_reason")
                .and_then(Value::as_str)
                .map(str::to_owned),
        );
    }
    // 舊行只有 ping 的形狀是自明的，其餘一律不知道
    let mode = match text(line, "diag").as_str() {
        "ping" => Some("ping".to_owned()),
        _ => None,
    };
    // 先問看不看得見再問中沒中：加 `cache_reporting` 欄之前的 api 行寫過壓平的假 0
    // （見 .ai/plans/api-cache-visibility.md），把它當真 0 會與同頁「—」的命中率打架
    let cached = line.get("cached_tokens").and_then(Value::as_u64);
    let (Some(cached), true) = (cached, reported(line)) else {
        return (mode, "unknown".to_owned(), None);
    };
    // 沒有 lane 欄＝無狀態呼叫，本來就算不出理論可中量，只憑數字說話
    if line.get("lane").is_none() {
        let cache = match cached > 0 {
            true => "hit",
            false => "zero",
        };
        return (mode, cache.to_owned(), None);
    }
    let created = line.get("created_tokens").and_then(Value::as_u64);
    // 舊的續聊線行：reason 在＝那輪重開了線，我方內容本來就不該中
    let expected = match line.get("reason").is_some() {
        true => 0,
        false => number(line, "expected_cached"),
    };
    let short = match number(line, "age_secs") > crate::lanes::LEGACY_CACHE_TTL_SECS {
        true => "expired",
        false => "below-expected",
    };
    if cached > 0 {
        if expected == 0 || cached * 10 >= expected * 9 {
            return (mode, "hit".to_owned(), None);
        }
        return (mode, "partial".to_owned(), Some(short.to_owned()));
    }
    if expected == 0 {
        return (mode, "not-expected".to_owned(), None);
    }
    let reason = match created {
        Some(0) => "skipped",
        _ => short,
    };
    (mode, "zero".to_owned(), Some(reason.to_owned()))
}

fn number(line: &Value, key: &str) -> u64 {
    line.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn text(line: &Value, key: &str) -> String {
    line.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn hit_rate(row: &UsageRow) -> Option<f64> {
    if row.observed_rounds == 0 {
        return None; // 一輪都量不到＝命中率不存在，不是 0
    }
    if row.observed_prompt_tokens == 0 {
        return Some(0.0);
    }
    let rate = row.cached_tokens as f64 * 100.0 / row.observed_prompt_tokens as f64;
    Some((rate * 10.0).round() / 10.0)
}

/// 這一輪的快取數字看不看得見。舊行沒有 `cache_reporting` 欄位，靠兩條規則還原：
/// - api：當年只讀 OpenRouter 的 `prompt_tokens_details.cached_tokens`，其他 schema
///   一律被 `unwrap_or(0)` 捏成 0。**捏得出來的只有 0**——所以 >0 必是讀對欄位的真值
///   （那時真的連著 OpenRouter），採信；記 0 的分不出真假，保守當量不到。
/// - CLI 三路：欄位一直是對的，照舊採信。
fn reported(line: &Value) -> bool {
    if line.get("unreported").and_then(Value::as_bool) == Some(true) {
        return false; // 連 token 都不回報的來源（agy），快取自然更無從得知
    }
    match line.get("cache_reporting").and_then(Value::as_str) {
        Some(value) => value == "reported",
        None => text(line, "transport") != "api" || number(line, "cached_tokens") > 0,
    }
}

/// 快取計價係數（相對一般輸入價）：`(讀快取, 寫快取, 讀係數是不是估的)`。讀便宜、寫（Anthropic）
/// 反而貴，兩邊都算進去才不會把「省下多少」灌水。沒列到的來源不估——agy 完全不回報用量，
/// api 走哪個模型不定。claude 的寫入係數在 `claude_write_cost` 依 1h／5m 拆分另算，這裡只放 5m 價。
fn cache_price(transport: &str, model: &str) -> Option<(f64, f64, bool)> {
    match transport {
        "claude" => {
            let (read, estimated) = claude_read_price(model);
            Some((read, 1.25, estimated))
        }
        "codex" => Some((0.1, 1.0, false)), // OpenAI：讀一折、寫不加價
        "grok" => Some((0.5, 1.0, false)),  // xAI 折扣落在五到七五折，取最保守的那頭
        _ => None,
    }
}

/// Anthropic 官方讀快取係數：Opus 5.5 0.05、Fable 5.1 0.025，其餘 0.1。
/// 版本要比到邊界（本身或接 `-` 日期後綴），`claude-opus-5-50` 不算。帳本記的是送給 CLI 的字樣，
/// 別名 `opus`／`fable` 現在指向這兩代，但舊列看不出世代，所以按別名套優惠標成估計。
fn claude_read_price(model: &str) -> (f64, bool) {
    let is = |version: &str| {
        model == version
            || model
                .strip_prefix(version)
                .is_some_and(|rest| rest.starts_with('-'))
    };
    if is("claude-opus-5-5") {
        (0.05, false)
    } else if is("claude-fable-5-1") {
        (0.025, false)
    } else if model == "opus" {
        (0.05, true)
    } else if model == "fable" {
        (0.025, true)
    } else {
        (0.1, false)
    }
}

/// claude 寫快取的等值 token：有 1h 拆分就 1h 部分 2 倍、其餘 1.25 倍；
/// 沒有拆分（舊列或來源沒給）全按 1.25 倍，第二個值回 true＝估計。
fn claude_write_cost(line: &Value, created: u64) -> (f64, bool) {
    match line.get("created_1h_tokens").and_then(Value::as_u64) {
        Some(one_hour) => {
            let one_hour = one_hour.min(created);
            (
                2.0 * one_hour as f64 + 1.25 * (created - one_hour) as f64,
                false,
            )
        }
        None => (1.25 * created as f64, created > 0),
    }
}

/// 輸出價是輸入價的幾倍（Anthropic 與 xAI 全系列都是 5 倍）。
/// 用途：把 CLI 回報的整輪金額拆回「一個輸入 token 值多少錢」，才不必自建價目表。
const OUTPUT_MULTIPLE: f64 = 5.0;

/// 一輪對照底線的結果，供彙總用。
#[derive(Clone, Copy)]
struct FloorVerdict {
    floor: Floor,
    vendor_only: bool,
}

fn accumulate(row: &mut UsageRow, line: &Value, include_cache: bool, verdict: FloorVerdict) {
    row.rounds += 1;
    if include_cache {
        row.cache_rounds += 1;
    }
    if line.get("unreported").and_then(Value::as_bool) == Some(true) {
        row.unreported += 1;
        row.cost_partial = true;
        if include_cache {
            row.saved_partial = true;
        }
        return;
    }
    let prompt = number(line, "prompt_tokens");
    let cached = number(line, "cached_tokens");
    let created = number(line, "created_tokens");
    let output = number(line, "output_tokens");
    row.prompt_tokens += prompt;
    row.output_tokens += output;
    // 量不到的輪次照計輸入輸出與花費，但不進命中率的分子分母
    if include_cache && reported(line) {
        row.observed_rounds += 1;
        row.observed_prompt_tokens += prompt;
        row.cached_tokens += cached;
        if let Floor::Known(floor) = verdict.floor {
            row.vendor_cached_tokens += cached.min(floor);
        }
        if verdict.vendor_only {
            row.vendor_only_rounds += 1;
            if text(line, "transport") == "api" {
                row.vendor_only_api_rounds += 1;
            }
        } else if cached > 0 {
            row.own_hit_rounds += 1;
        }
    }
    let cost = add_cost(row, line);

    // 生圖不是劇情對話快取，只保留 token／實際費用，不估進對話省下量。
    if !include_cache {
        return;
    }
    // 省下多少一輪一算再累加：每輪的單價與命中結構都不同，先加總會算錯
    let transport = text(line, "transport");
    let Some((read_mult, write_mult, read_estimated)) =
        cache_price(&transport, &text(line, "model"))
    else {
        row.saved_partial = true;
        return;
    };
    let (write_cost, write_estimated) = match transport.as_str() {
        "claude" => claude_write_cost(line, created),
        _ => (write_mult * created as f64, false),
    };
    // 自帶段：認得出底線的組每輪先扣 min(讀到, 底線)，已省只算我方內容讀到的那部分（作者裁決 2026-10-28）。
    // 只中到底線的輪次因此自然扣到我方讀到 0，不另設特例；寫快取加價照算，省額可為負
    let vendor = match verdict.floor {
        Floor::Known(floor) => cached.min(floor),
        Floor::Unknown => 0,
    };
    if (read_estimated && cached > vendor) || write_estimated {
        row.estimated_rounds += 1;
    }
    // 輸入量整段留在分母（含自帶段）
    row.priced_tokens += prompt;
    let fresh = prompt.saturating_sub(cached + created) as f64;
    let paid = fresh + read_mult * cached as f64 + write_cost;
    // ＝原省額 − 自帶段×(1−讀係數)：自帶段當成照原價付，其餘照實付（這樣寫只中自帶的輪次精確得 0）
    let saved =
        prompt as f64 - (fresh + vendor as f64) - read_mult * (cached - vendor) as f64 - write_cost;
    row.saved_tokens += saved;
    let charged = paid + OUTPUT_MULTIPLE * output as f64;
    match cost {
        Some(value) if charged > 0.0 => {
            row.saved_usd = Some(row.saved_usd.unwrap_or(0.0) + value / charged * saved);
        }
        _ => row.saved_partial = true,
    }
}

fn add_cost(row: &mut UsageRow, line: &Value) -> Option<f64> {
    let cost = line.get("cost_usd").and_then(Value::as_f64);
    match cost {
        Some(value) => row.cost_usd = Some(row.cost_usd.unwrap_or(0.0) + value),
        None => row.cost_partial = true,
    }
    cost
}

/// `scope` 為 None＝所有桌總計；`Some(id)` 只算該桌（空字串＝未標桌）。
/// `names` 是桌 id→顯示名（找不到就用 id）；`in_use` 是目前設定在用的（來源, 模型）。
pub fn summarize(
    log: &str,
    scope: Option<&str>,
    names: &[(String, String)],
    in_use: &[(String, String)],
) -> UsageReport {
    // app 現有的桌先全部列進去（沒紀錄就顯示 0 輪），順序照桌列表
    let mut worlds: Vec<WorldOption> = names
        .iter()
        .map(|(id, name)| WorldOption {
            id: id.clone(),
            name: name.clone(),
            rounds: 0,
        })
        .collect();
    let mut rows: Vec<UsageRow> = Vec::new();
    let mut total = UsageRow::default();
    let mut caches: Vec<CacheCount> = Vec::new();
    let mut events = 0u64;
    let mut latest = None;

    // 壞行跳過：診斷設施本來就是盡力而為
    let lines: Vec<Value> = log
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect();
    let floors = floors(&lines);

    for line in lines {
        // 純診斷事件（CLI 提醒偵測）不是一輪也不是線事件：不算輪數、不佔「最近一輪」
        if line.get("event").and_then(Value::as_str)
            == Some(crate::usage::log::Event::CliReminderSeen.as_str())
        {
            continue;
        }
        let world = text(&line, "world");
        // 對不上現有桌＝開桌生成或已刪掉的桌，接在桌列表後面
        match worlds.iter_mut().find(|option| option.id == world) {
            Some(option) => option.rounds += 1,
            None => worlds.push(WorldOption {
                id: world.clone(),
                name: String::new(),
                rounds: 1,
            }),
        }
        if scope.is_some_and(|scope| scope != world) {
            continue;
        }

        // 事件行（丟線重來）沒有快取結果，不進快取統計的分母；但它是最近發生的事，
        // 該讓玩家在收合狀態就看到，所以照樣占用「最近一輪」那句話
        if line.get("model").is_none() {
            events += 1;
            latest = Some(LatestCall {
                ts: text(&line, "ts"),
                mode: None,
                cache: None,
                cache_reason: None,
                // 加 event 欄之前的舊行把它寫在 diag
                event: Some(match line.get("event").and_then(Value::as_str) {
                    Some(event) => event.to_owned(),
                    None => text(&line, "diag"),
                }),
                reason: line
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                reported: true,
            });
            continue;
        }
        let (mode, mut cache, cache_reason) = classify(&line);
        let floor = floors.get(&floor_key(&line));
        let verdict = FloorVerdict {
            floor,
            vendor_only: vendor_only(&line, &cache, floor),
        };
        if verdict.vendor_only {
            cache = match text(&line, "transport").as_str() {
                "api" => "vendor-only-api",
                _ => "vendor-only",
            }
            .to_owned();
        }
        // 舊帳本的保溫呼叫（已撤）：錢花了要算進合計與該模型那列，
        // 但它不推進劇情、一律命中，輪數、token、命中率、省額與快取分佈都不收
        let is_ping = mode.as_deref() == Some("ping");
        let is_image = line.get("purpose").and_then(Value::as_str) == Some("image");
        // 生圖可能命中供應商固定工具前綴，但那不是劇情對話快取。token／費用仍照算，
        // 只從對話命中統計與「最近一輪」燈號排除。
        if !is_image && !is_ping {
            let state = chip_state(&cache, cache_reason.as_deref());
            match caches.iter_mut().find(|count| count.cache == state) {
                Some(count) => count.rounds += 1,
                None => caches.push(CacheCount {
                    cache: state,
                    rounds: 1,
                }),
            }
        }
        if !is_ping && !is_image {
            latest = Some(LatestCall {
                ts: text(&line, "ts"),
                mode,
                cache: Some(cache.clone()),
                cache_reason,
                event: None,
                reason: line
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                reported: reported(&line),
            });
        }
        match is_ping {
            true => add_cost(&mut total, &line),
            false => {
                accumulate(&mut total, &line, !is_image, verdict);
                None
            }
        };
        let source = text(&line, "transport");
        let model = text(&line, "model");
        let row = match rows
            .iter_mut()
            .position(|row| row.source == source && row.model == model)
        {
            Some(index) => &mut rows[index],
            None => {
                rows.push(UsageRow {
                    in_use: in_use.iter().any(|(used_source, used_model)| {
                        *used_source == source && *used_model == model
                    }),
                    source,
                    model,
                    ..UsageRow::default()
                });
                rows.last_mut().expect("just pushed")
            }
        };
        match is_ping {
            true => {
                add_cost(row, &line);
            }
            false => accumulate(row, &line, !is_image, verdict),
        }
    }

    for row in rows.iter_mut().chain([&mut total]) {
        row.hit_rate = hit_rate(row);
    }
    // 花得最多的排前面（桌下拉不排序，照桌列表原順序）
    rows.sort_by_key(|row| std::cmp::Reverse(row.prompt_tokens));
    caches.sort_by_key(|count| std::cmp::Reverse(count.rounds));

    UsageReport {
        worlds,
        rows,
        total,
        caches,
        events,
        latest,
    }
}

#[cfg(test)]
mod tests;
