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
//! 診斷標籤是有限清單，純規則判定（不靠 AI）。每個標籤對應一句給玩家看的話，
//! 包 6 照這張表配 i18n 字典：
//! - `ok`：快取正常，這一句只付新內容的錢。
//! - `warmup`：這條線重新開始，整份設定要重新建快取（`reason` 說明為什麼重開）。
//! - `expired`：距上一句超過五分鐘，快取自然過期，這句重建。
//! - `prefix-broken`：照理該命中卻沒中；`expected_cached` 對 `cached_tokens` 看差多少，
//!   `cached_tokens` 接近 `system_tokens` 代表只有設定段中、對話段斷了。
//! - `cache-skipped`：CLI 這一句沒帶快取標記（claude CLI resume 已知毛病，2026-08 實證：
//!   同一線隔輪出現，讀寫皆 0、整句付全額；上輪有送內容才判此標，與整條路不支援區分）。
//! - `no-cache`：這條路完全沒有快取（模型或 CLI 不支援）。
//! - `single`：單發模式（API／codex／grok），只記數字不做續聊診斷。**不代表沒有快取**——
//!   單發也可能命中供應商的自動前綴快取，能不能看見由 `cache_reporting` 獨立表達。
//! - `drop-lane`：回合後改寫失敗，這條線丟棄重來（`reason` 說明原因）。
//! - `ping`：保溫呼叫（包 7），不推進劇情、只為刷新快取壽命；花費與劇情輪分開統計。

use crate::lanes::CACHE_TTL_SECS;
use crate::transport::PromptCacheUsage;
use serde_json::{json, Map, Value};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diag {
    Ok,
    Warmup,
    Expired,
    PrefixBroken,
    CacheSkipped,
    NoCache,
    Single,
    DropLane,
    Ping,
}

impl Diag {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warmup => "warmup",
            Self::Expired => "expired",
            Self::PrefixBroken => "prefix-broken",
            Self::CacheSkipped => "cache-skipped",
            Self::NoCache => "no-cache",
            Self::Single => "single",
            Self::DropLane => "drop-lane",
            Self::Ping => "ping",
        }
    }
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

fn diagnose(lane: Option<&LaneContext>, usage: &PromptCacheUsage) -> Diag {
    let Some(lane) = lane else {
        return Diag::Single;
    };
    if lane.ping {
        return Diag::Ping;
    }
    if lane.reopen.is_some() {
        return Diag::Warmup;
    }
    // lane 路徑只有 claude CLI，它必定回報快取欄位；None 退 0 只是型別收尾
    let cached = usage.cached_tokens.unwrap_or(0);
    let created = usage.created_tokens.unwrap_or(0);
    if cached == 0 && created == 0 {
        // 上輪送過內容＝這條路支援快取，讀寫皆 0 是 CLI 這句沒帶標記
        if lane.expected_cached > 0 {
            return Diag::CacheSkipped;
        }
        return Diag::NoCache;
    }
    if lane.age_secs > CACHE_TTL_SECS {
        return Diag::Expired;
    }
    if cached * HIT_TOLERANCE_DEN >= lane.expected_cached * HIT_TOLERANCE_NUM {
        return Diag::Ok;
    }
    Diag::PrefixBroken
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

/// 一次呼叫一行。lane 為 None＝單發路徑（API／codex／grok），只記數字。
/// world 為 None＝不屬於任何一桌（開桌生成）；加欄前的舊行同樣沒有，額度分頁歸「未標桌」。
pub fn append_call(
    path: &Path,
    world: Option<&str>,
    transport: &str,
    model: &str,
    lane: Option<&LaneContext>,
    usage: PromptCacheUsage,
) {
    let mut fields = Map::new();
    fields.insert("transport".to_owned(), json!(transport));
    if let Some(world) = world {
        fields.insert("world".to_owned(), json!(world));
    }
    fields.insert("model".to_owned(), json!(model));
    fields.insert("diag".to_owned(), json!(diagnose(lane, &usage).as_str()));
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

/// 沒有用量數字的線事件（目前只有抹寫失敗丟線）。
pub fn append_event(path: &Path, world: Option<&str>, lane: &str, diag: Diag, reason: &str) {
    let mut fields = Map::new();
    fields.insert("transport".to_owned(), json!("claude"));
    if let Some(world) = world {
        fields.insert("world".to_owned(), json!(world));
    }
    fields.insert("lane".to_owned(), json!(lane));
    fields.insert("diag".to_owned(), json!(diag.as_str()));
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

    /// 標籤是有限清單，判定只看 app 自己的決策＋數字，不猜。
    #[test]
    fn diagnosis_covers_each_label_by_rule() {
        // 續聊、幾乎全中
        assert_eq!(
            diagnose(Some(&lane(30, 9_000)), &usage(9_300, 9_000, 300)),
            Diag::Ok
        );
        // 上輪送 9000，這輪只中 200＝前綴斷了
        assert_eq!(
            diagnose(Some(&lane(30, 9_000)), &usage(9_300, 200, 9_100)),
            Diag::PrefixBroken
        );
        // 隔太久＝自然過期，不算故障
        assert_eq!(
            diagnose(Some(&lane(600, 9_000)), &usage(9_300, 0, 9_300)),
            Diag::Expired
        );
        // 讀寫皆 0 但上輪送過內容＝CLI 這句沒帶標記，優先於過期判定
        assert_eq!(
            diagnose(Some(&lane(600, 9_000)), &usage(9_300, 0, 0)),
            Diag::CacheSkipped
        );
        // 讀寫皆 0 且上輪也沒有可中量＝這條路整個不支援快取
        assert_eq!(
            diagnose(Some(&lane(30, 0)), &usage(9_300, 0, 0)),
            Diag::NoCache
        );
        // 重開線＝暖機，不論數字
        let mut reopened = lane(30, 0);
        reopened.reopen = Some("scene-changed");
        assert_eq!(
            diagnose(Some(&reopened), &usage(9_300, 0, 9_300)),
            Diag::Warmup
        );
        // 保溫呼叫自成一類：花費要和劇情輪分開統計，命中與否不代表劇情線有問題
        let mut ping = lane(200, 9_000);
        ping.ping = true;
        assert_eq!(diagnose(Some(&ping), &usage(9_000, 9_000, 0)), Diag::Ping);
        // 單發路徑不做續聊診斷
        assert_eq!(diagnose(None, &usage(100, 0, 0)), Diag::Single);
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
            usage(9_300, 9_000, 300),
        );
        append_event(
            &path,
            Some("w1"),
            "chars:sonnet",
            Diag::DropLane,
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
        assert_eq!(call["diag"], json!("ok"));
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
        assert_eq!(event["diag"], json!("drop-lane"));
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

        append_call(&path, None, "claude", "opus", None, usage(500, 0, 0)); // 開桌大綱
        append_call(&path, None, "claude", "opus", None, usage(900, 0, 0)); // 開桌展開
        append_call(&path, Some("w9"), "claude", "sonnet", None, usage(100, 0, 0)); // 別桌，不該被動到
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
        assert_eq!(first["diag"], json!("single"));
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
