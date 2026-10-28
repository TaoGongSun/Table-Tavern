use crate::transport::PromptCacheUsage;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct CliInfo {
    pub id: String,
    pub path: String,
    pub version: String,
}

/// 設定 UI 下拉用的模型選項。清單讀自各 CLI 自身（codex：app 的 codex-home，退回 ~/.codex 的 models_cache.json；
/// claude：執行檔內建的模型註冊表），非本程式寫死的正典；
/// 實際用哪個模型仍由 config 的覆寫決定。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
    /// 總 context（OpenRouter 目錄的 context_length 與 top_provider 取小）；CLI 目錄沒有
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u64>,
    /// 供應商的輸出上限（OpenRouter top_provider.max_completion_tokens）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
}

/// Agy 在 `--conversation` 續聊時回傳的是整段 conversation 的累積計數。
/// lane 保存上一輪快照，runner 才能把本輪增量換算成共用的 PromptCacheUsage 語意。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgyUsageCounters {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
}

/// codex `exec resume` 的 `turn.completed.usage` 是整條 thread 的累計值；lane 存上一輪成功那次，
/// runner 相減才是這一輪的用量。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexUsageCounters {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_input_tokens: u64,
    pub output_tokens: u64,
}

/// 續聊線的 session 觀測（目前只有 codex 用）：不綁在 usage_log 上，沒開用量帳本也照樣擷取。
#[derive(Default)]
pub struct SessionWatch<'a> {
    /// 回填這次看到的 thread id（開線時由 CLI 產生）
    pub id_out: Option<&'a std::sync::Mutex<Option<String>>>,
    /// 續聊必須留在這個 thread；CLI 回報別的 id 就在正文前中止
    pub expected_id: Option<&'a str>,
    /// 上一輪成功時的累計用量；續聊時 None＝基準不明，這輪用量記為未知
    pub codex_usage_base: Option<CodexUsageCounters>,
    /// 續聊（要相減）才設 true；開線那輪累計值就是本輪用量
    pub resumed: bool,
    /// 回填這輪 turn.completed 的累計用量，供 lane 存成下一輪的基準
    pub codex_usage_out: Option<&'a std::sync::Mutex<Option<CodexUsageCounters>>>,
    /// 收到完成事件、程序卻以非 0 結束：正文照交，但 session 是否落檔不可信，lane 據此丟線
    pub unclean_exit_out: Option<&'a std::sync::atomic::AtomicBool>,
}

/// 一家 CLI 的輸出協定：逐行解析器、認得的事件形狀與收尾規則。
pub struct CliProtocol {
    /// CLI id（claude／codex／agy／grok）：上次正常版本與去重記錄的鍵
    pub id: &'static str,
    /// 錯誤訊息裡的顯示名
    pub name: &'static str,
    pub parse: fn(&str) -> CliLine,
    /// 認不得的行回 Some(種類標籤)，認得回 None（`stream::known`）
    pub unrecognized: fn(&str) -> Option<String>,
    /// 沒收到完成事件（`CliLine::Done`）就結束一律算失敗，即使程序以 0 結束：
    /// 收尾事件改名或半途斷掉時，半截正文不能當成功（四家都是 true）。
    pub require_done: bool,
    /// 已報失敗的收尾不讓後到的事件蓋掉。只給 codex：grok 的頂層 error 之後 end_turn＝成功。
    pub sticky_failed_done: bool,
    /// codex 專屬的 session 落檔檢查：rollout 存檔失敗偵測、完成但非 0 退出就丟線。
    pub codex_session_checks: bool,
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
    Done {
        text: String,
        is_error: bool,
    },
    /// 非致命的錯誤通知（codex 頂層 error：重試中的 Reconnecting 也走這裡）：記下來，不結束回合；
    /// 回合最後失敗時拿最後一則當錯誤文字。
    SoftError(String),
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
    pub lane: Option<crate::usage::log::LaneContext>,
    /// 這通送出去的是什麼形狀（唯讀情報，只用來標帳本的 mode）。
    pub shape: crate::usage::log::PromptShape,
    /// 回填本輪總輸入，供 lane 記成下輪的理論可中量（跨 await 需 Sync，故用 atomic）。
    pub prompt_tokens_out: Option<&'a std::sync::atomic::AtomicU64>,
    /// Agy 開線由 CLI 產生 conversation ID；runner 在 init/result 看到後回填。
    pub conversation_id_out: Option<&'a std::sync::Mutex<Option<String>>>,
    /// Agy resume 必須留在指定對話；CLI 若靜默改開新線，在正文前就中止。
    pub expected_conversation_id: Option<&'a str>,
    /// 同一 Agy conversation 上輪的累積 usage；None＝開線或單發。
    pub agy_usage_base: Option<AgyUsageCounters>,
    /// 回填這輪 result 的累積 usage，供 lane 保存給下輪做差。
    pub agy_usage_out: Option<&'a std::sync::Mutex<Option<AgyUsageCounters>>>,
    /// claude：這一次回報裡辨識得出的實際模型 id（辨識不了就保持 None）。
    /// 換幕容量的估計校正只在身分確認時才記，避免把新倍率掛到舊模型上。
    pub identity_out: Option<&'a std::sync::Mutex<Option<String>>>,
    /// 回填這次嘗試解析出的用量（含 1h 拆分），供 lane 估計快取時效與超額觀測。
    pub usage_out: Option<&'a std::sync::Mutex<Option<PromptCacheUsage>>>,
    /// claude 這次嘗試內任一 rate_limit_event 標示正在用超額，就設 true（不撤銷）。
    pub overage_out: Option<&'a std::sync::atomic::AtomicBool>,
}
