use crate::transport::PromptCacheUsage;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct CliInfo {
    pub id: String,
    pub path: String,
    pub version: String,
}

/// 設定 UI 下拉用的模型選項。清單讀自各 CLI 自身（codex：~/.codex/models_cache.json；
/// claude：執行檔內建的模型註冊表），非本程式寫死的正典；
/// 實際用哪個模型仍由 config 的覆寫決定。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
}

/// claude lane 續聊（prompt-cache-optimization 包 2）的 session 指定方式。
pub enum CliSession<'a> {
    /// 開新線：session id 由本程式產生（UUID），之後靠它 resume 與定位 session 檔。
    Open(&'a str),
    /// 續聊既有線：實測 resume 沿用同一 id、續寫同一檔（不分叉）。
    Resume(&'a str),
}

#[derive(Debug, PartialEq)]
pub enum CliLine {
    Delta(String),
    /// 思考增量：只餵進度顯示（on_delta），不進正文——長思考段（如卡重構盤點）若無它，
    /// 進度小框會整段空白，玩家分不出「在想」與「掛了」。
    Thinking(String),
    Done { text: String, is_error: bool },
    Other,
}

/// CLI 路徑的用量落檔設定（prompt-cache-optimization：CLI 量測）。
/// 帶著就在收尾事件把這次呼叫追加成一行 JSONL，與 API 那條共用同一份檔案與格式。
pub struct UsageLog<'a> {
    pub path: &'a Path,
    /// 這次呼叫屬於哪一桌；None＝開桌生成等不屬於任何一桌的呼叫。
    pub world: Option<&'a str>,
    /// log 的 transport 欄位，例如 "claude"。
    pub transport: &'a str,
    pub model: &'a str,
    pub parse: fn(&str) -> Option<PromptCacheUsage>,
    /// 續聊線的脈絡；None＝無狀態路徑（快取結果照樣判，形狀由 `shape` 交代）。
    pub lane: Option<crate::usage_log::LaneContext>,
    /// 這通送出去的是什麼形狀（唯讀情報，只用來標帳本的 mode）。
    pub shape: crate::usage_log::PromptShape,
    /// 回填本輪總輸入，供 lane 記成下輪的理論可中量（跨 await 需 Sync，故用 atomic）。
    pub prompt_tokens_out: Option<&'a std::sync::atomic::AtomicU64>,
}

