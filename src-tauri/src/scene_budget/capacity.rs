//! 模型容量與估計校正的落地檔（資料根 `model-capacity.json`，可重建的快取）。
//! 鍵＝「transport|送給 CLI 的模型字串」（claude 是別名，如 haiku）。
//! - claude 每次呼叫的 result 回報實際 model id、總 context（contextWindow）與 maxOutputTokens；
//!   實際 id 一變（CLI 更新換了別名對應）整筆校正作廢。
//! - 校正＝某一次實送的「完整 input usage」÷「同一次請求的保守估計」，依路徑種類（gm／chars／summary）
//!   分開存，不互相套用。
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Calibration {
    pub estimate: u64,
    pub actual: u64,
    /// 記錄當時的實際 model id；與目前不同就不採用
    #[serde(default)]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelCapacity {
    #[serde(default)]
    pub model_id: Option<String>,
    /// 總 context（輸入＋輸出＋思考共用）
    #[serde(default)]
    pub context_window: Option<u64>,
    #[serde(default)]
    pub max_output: Option<u64>,
    #[serde(default)]
    pub calibration: BTreeMap<String, Calibration>,
}

impl ModelCapacity {
    /// 這個路徑種類的校正倍率（實報 ÷ 估計）。下限 0.5 防壞資料；不設上限——符號多的文本比率本來就可能偏高。
    pub fn ratio(&self, kind: &str) -> Option<f64> {
        let entry = self.calibration.get(kind)?;
        if entry.estimate == 0 || entry.actual == 0 || entry.model_id != self.model_id {
            return None;
        }
        Some((entry.actual as f64 / entry.estimate as f64).max(0.5))
    }
}

pub type Store = BTreeMap<String, ModelCapacity>;

pub fn key(transport: &str, model: &str) -> String {
    format!("{transport}|{model}")
}

pub fn path(data_root: &Path) -> PathBuf {
    data_root.join("model-capacity.json")
}

/// 讀不到、壞掉都當空的（快取可重建，不擋任何呼叫）。
pub fn read(path: &Path) -> Store {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 讀改寫（全程持鎖、暫存檔＋rename）；任何一步失敗就放著不動，只是少一筆快取。
fn update(path: &Path, edit: impl FnOnce(&mut Store)) {
    let Ok(_guard) = WRITE_LOCK.lock() else {
        return;
    };
    let mut store = read(path);
    edit(&mut store);
    let Ok(text) = serde_json::to_string_pretty(&store) else {
        return;
    };
    let temp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    // world-write-exempt: 資料根的模型容量快取，不是桌目錄
    if std::fs::write(&temp, text).is_ok() && std::fs::rename(&temp, path).is_err() {
        let _ = std::fs::remove_file(&temp);
    }
}

/// claude result 回報的模型身分與容量。實際 id 變了就清掉舊校正。
pub fn record_model(
    path: &Path,
    transport: &str,
    model: &str,
    model_id: &str,
    context_window: Option<u64>,
    max_output: Option<u64>,
) {
    update(path, |store| {
        let entry = store.entry(key(transport, model)).or_default();
        if entry.model_id.as_deref() != Some(model_id) {
            entry.calibration.clear();
            entry.model_id = Some(model_id.to_owned());
        }
        if context_window.is_some() {
            entry.context_window = context_window;
        }
        if max_output.is_some() {
            entry.max_output = max_output;
        }
    });
}

/// 一次實送的校正（同一次請求的估計與實報）。
pub fn record_calibration(
    path: &Path,
    transport: &str,
    model: &str,
    kind: &str,
    estimate: u64,
    actual: u64,
    model_id: Option<&str>,
) {
    if estimate == 0 || actual == 0 {
        return;
    }
    update(path, |store| {
        let entry = store.entry(key(transport, model)).or_default();
        // 校正記的是這一次確認過的身分，不借快取裡的舊 id；與目前記錄的身分不同時 ratio 不會採用
        entry.calibration.insert(
            kind.to_owned(),
            Calibration {
                estimate,
                actual,
                model_id: model_id.map(str::to_owned),
            },
        );
    });
}

/// 這一次回報辨識不出實際模型：整筆作廢（容量與校正都不可信，回到預設值只提醒不鎖）。
pub fn forget_model(path: &Path, transport: &str, model: &str) {
    update(path, |store| {
        store.remove(&key(transport, model));
    });
}

/// claude result 行裡、對得上這次請求模型的 `modelUsage.<實際 id>`：(id, contextWindow, maxOutputTokens)。
/// 身分要能證明是主模型：`requested` 是完整 id 就要相同（或為其前綴），是別名（haiku／sonnet／opus…）
/// 就要 id 含該別名；對得上的不是恰好一個（CLI 內部另叫了同家族小模型、或都對不上）就不記——
/// 無法辨識不升格成可靠來源，容量判定只提醒不鎖。
pub fn claude_model_usage(
    line: &str,
    requested: &str,
) -> Option<(String, Option<u64>, Option<u64>)> {
    if !line.contains("\"modelUsage\"") {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("type").and_then(|t| t.as_str()) != Some("result") {
        return None;
    }
    let usage = value.get("modelUsage")?.as_object()?;
    let requested = requested.trim().to_lowercase();
    if requested.is_empty() {
        return None;
    }
    let matches = |id: &str| {
        let id = id.to_lowercase();
        match requested.starts_with("claude-") {
            true => id == requested || id.starts_with(&format!("{requested}-")),
            false => id.contains(&requested),
        }
    };
    let mut found = usage.iter().filter(|(id, _)| matches(id));
    let (id, entry) = found.next()?;
    if found.next().is_some() {
        return None;
    }
    let context = entry.get("contextWindow").and_then(|v| v.as_u64())?;
    Some((
        id.clone(),
        Some(context),
        entry.get("maxOutputTokens").and_then(|v| v.as_u64()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tt-capacity-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        path(&dir)
    }

    #[test]
    fn model_change_drops_calibration_and_kinds_stay_separate() {
        let file = temp("change");
        record_model(
            &file,
            "claude",
            "haiku",
            "claude-haiku-4-5",
            Some(200_000),
            Some(32_000),
        );
        record_calibration(
            &file,
            "claude",
            "haiku",
            "gm",
            1000,
            1100,
            Some("claude-haiku-4-5"),
        );
        let store = read(&file);
        let entry = &store[&key("claude", "haiku")];
        assert_eq!(entry.context_window, Some(200_000));
        assert_eq!(entry.ratio("gm"), Some(1.1));
        assert_eq!(entry.ratio("summary"), None);
        // 同一個 id 再報一次：校正保留
        record_model(
            &file,
            "claude",
            "haiku",
            "claude-haiku-4-5",
            Some(200_000),
            None,
        );
        assert_eq!(read(&file)[&key("claude", "haiku")].ratio("gm"), Some(1.1));
        // 別名換了實際模型：舊校正作廢
        record_model(
            &file,
            "claude",
            "haiku",
            "claude-haiku-5",
            Some(400_000),
            Some(64_000),
        );
        let entry = &read(&file)[&key("claude", "haiku")];
        assert_eq!(entry.ratio("gm"), None);
        assert_eq!(entry.context_window, Some(400_000));
        std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
    }

    /// 身分不明的回報：校正不借快取舊 id；整筆作廢後回到不可靠（Sol 探針：新倍率掛到舊模型、舊容量仍可鎖）
    #[test]
    fn unidentified_reports_never_calibrate_the_cached_model() {
        let file = temp("identity");
        record_model(
            &file,
            "claude",
            "haiku",
            "claude-haiku-4-5",
            Some(200_000),
            Some(32_000),
        );
        // 這次身分不明卻記了校正（model_id None）：與目前記錄的身分不符，不採用
        record_calibration(&file, "claude", "haiku", "summary", 1000, 1500, None);
        assert_eq!(read(&file)[&key("claude", "haiku")].ratio("summary"), None);
        // 記到別的 id 也不採用
        record_calibration(
            &file,
            "claude",
            "haiku",
            "summary",
            1000,
            1500,
            Some("claude-haiku-5"),
        );
        assert_eq!(read(&file)[&key("claude", "haiku")].ratio("summary"), None);
        // runner 辨識不了時整筆作廢：容量回到未知
        forget_model(&file, "claude", "haiku");
        assert!(read(&file).get(&key("claude", "haiku")).is_none());
        std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
    }

    #[test]
    fn ratio_has_a_floor_but_no_ceiling() {
        let mut entry = ModelCapacity::default();
        entry.calibration.insert(
            "summary".into(),
            Calibration {
                estimate: 100,
                actual: 10,
                model_id: None,
            },
        );
        assert_eq!(entry.ratio("summary"), Some(0.5));
        entry.calibration.insert(
            "summary".into(),
            Calibration {
                estimate: 100,
                actual: 300,
                model_id: None,
            },
        );
        assert_eq!(entry.ratio("summary"), Some(3.0));
    }

    #[test]
    fn corrupt_file_reads_as_empty() {
        let file = temp("corrupt");
        std::fs::write(&file, "{not json").unwrap();
        assert!(read(&file).is_empty());
        std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
    }

    // 2026-10-06 haiku 實測的 result 行（節錄 modelUsage）
    #[test]
    fn parses_claude_model_usage() {
        let line = r#"{"type":"result","is_error":false,"result":"OK","usage":{"input_tokens":9},"modelUsage":{"claude-haiku-4-5-20251001":{"inputTokens":9,"contextWindow":200000,"maxOutputTokens":32000}}}"#;
        let expected = Some((
            "claude-haiku-4-5-20251001".to_owned(),
            Some(200_000),
            Some(32_000),
        ));
        assert_eq!(claude_model_usage(line, "haiku"), expected);
        assert_eq!(claude_model_usage(line, "claude-haiku-4-5"), expected);
        assert_eq!(
            claude_model_usage(line, "claude-haiku-4-5-20251001"),
            expected
        );
        // 對不上請求的模型：不記
        assert_eq!(claude_model_usage(line, "opus"), None);
        assert_eq!(
            claude_model_usage(r#"{"type":"result","modelUsage":{}}"#, "haiku"),
            None
        );
        // 主模型＋CLI 內部另叫的模型：取對得上請求的那個，不是第一個
        let two = r#"{"type":"result","modelUsage":{"claude-haiku-4-5":{"contextWindow":200000},"claude-opus-5":{"contextWindow":1000000,"maxOutputTokens":64000}}}"#;
        assert_eq!(
            claude_model_usage(two, "opus"),
            Some(("claude-opus-5".to_owned(), Some(1_000_000), Some(64_000)))
        );
        // 兩個都對得上（同家族兩版）：無法辨識，不記
        let ambiguous = r#"{"type":"result","modelUsage":{"claude-haiku-4-5":{"contextWindow":200000},"claude-haiku-5":{"contextWindow":400000}}}"#;
        assert_eq!(claude_model_usage(ambiguous, "haiku"), None);
    }
}
