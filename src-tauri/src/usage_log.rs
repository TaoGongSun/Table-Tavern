//! 快取／花費紀錄（prompt-cache-optimization 包 4）：一次呼叫一行 JSONL，
//! 落在資料目錄的 `prompt-cache.jsonl`，供設定頁的額度分頁（包 6）直接讀。
//! 線的動作（重開／補丁／追平）與該次呼叫的用量寫在同一筆——分成兩種行時，
//! 「命中率為什麼掉」得靠時間戳自己接，接錯就誤診（三輪 0% 的誤判即出於此）。
//!
//! 診斷標籤是有限清單，純規則判定（不靠 AI）。每個標籤對應一句給玩家看的話，
//! 包 6 照這張表配 i18n 字典：
//! - `ok`：快取正常，這一句只付新內容的錢。
//! - `warmup`：這條線重新開始，整份設定要重新建快取（`reason` 說明為什麼重開）。
//! - `expired`：距上一句超過五分鐘，快取自然過期，這句重建。
//! - `prefix-broken`：照理該命中卻沒中；`expected_cached` 對 `cached_tokens` 看差多少，
//!   `cached_tokens` 接近 `system_tokens` 代表只有設定段中、對話段斷了。
//! - `no-cache`：這條路完全沒有快取（模型或 CLI 不支援）。
//! - `single`：單發模式（API／codex／grok），只記數字不做續聊診斷。
//! - `drop-lane`：回合後改寫失敗，這條線丟棄重來（`reason` 說明原因）。

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
    NoCache,
    Single,
    DropLane,
}

impl Diag {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warmup => "warmup",
            Self::Expired => "expired",
            Self::PrefixBroken => "prefix-broken",
            Self::NoCache => "no-cache",
            Self::Single => "single",
            Self::DropLane => "drop-lane",
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
}

/// 九成以上算正常：CLI 端 token 計數與分段邊界本來就會有零頭出入。
const HIT_TOLERANCE_NUM: u64 = 9;
const HIT_TOLERANCE_DEN: u64 = 10;

fn diagnose(lane: Option<&LaneContext>, usage: &PromptCacheUsage) -> Diag {
    let Some(lane) = lane else {
        return Diag::Single;
    };
    if lane.reopen.is_some() {
        return Diag::Warmup;
    }
    if usage.cached_tokens == 0 && usage.created_tokens == 0 {
        return Diag::NoCache;
    }
    if lane.age_secs > CACHE_TTL_SECS {
        return Diag::Expired;
    }
    if usage.cached_tokens * HIT_TOLERANCE_DEN >= lane.expected_cached * HIT_TOLERANCE_NUM {
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
pub fn append_call(
    path: &Path,
    transport: &str,
    model: &str,
    lane: Option<&LaneContext>,
    usage: PromptCacheUsage,
) {
    let mut fields = Map::new();
    fields.insert("transport".to_owned(), json!(transport));
    fields.insert("model".to_owned(), json!(model));
    fields.insert("diag".to_owned(), json!(diagnose(lane, &usage).as_str()));
    fields.insert("prompt_tokens".to_owned(), json!(usage.prompt_tokens));
    fields.insert("cached_tokens".to_owned(), json!(usage.cached_tokens));
    fields.insert("created_tokens".to_owned(), json!(usage.created_tokens));
    fields.insert("output_tokens".to_owned(), json!(usage.output_tokens));
    fields.insert(
        "hit_rate".to_owned(),
        json!((usage.hit_rate() * 10.0).round() / 10.0),
    );
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

/// 沒有用量數字的線事件（目前只有抹寫失敗丟線）。
pub fn append_event(path: &Path, lane: &str, diag: Diag, reason: &str) {
    let mut fields = Map::new();
    fields.insert("transport".to_owned(), json!("claude"));
    fields.insert("lane".to_owned(), json!(lane));
    fields.insert("diag".to_owned(), json!(diag.as_str()));
    fields.insert("reason".to_owned(), json!(reason));
    append(path, fields);
}

/// 時間戳固定排第一個 key（JSON 物件保序），肉眼掃 log 時一行讀得下去。
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
            cached_tokens: cached,
            created_tokens: created,
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
        // 完全沒有快取數字（模型或 CLI 不支援）優先於過期判定
        assert_eq!(
            diagnose(Some(&lane(600, 9_000)), &usage(9_300, 0, 0)),
            Diag::NoCache
        );
        // 重開線＝暖機，不論數字
        let mut reopened = lane(30, 0);
        reopened.reopen = Some("scene-changed");
        assert_eq!(
            diagnose(Some(&reopened), &usage(9_300, 0, 9_300)),
            Diag::Warmup
        );
        // 單發路徑不做續聊診斷
        assert_eq!(diagnose(None, &usage(100, 0, 0)), Diag::Single);
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
            "claude",
            "sonnet",
            Some(&patched),
            usage(9_300, 9_000, 300),
        );
        append_event(&path, "chars:sonnet", Diag::DropLane, "rewrite-failed");

        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);

        let call: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(call["ts"].as_str().unwrap().len(), 19); // 秒級：分不出五分鐘過期線就沒用
        assert_eq!(call["transport"], json!("claude"));
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
