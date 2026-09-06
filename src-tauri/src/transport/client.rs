use crate::data::{AppConfig, DataResult, Tier};

use futures_util::StreamExt;

use serde::Serialize;

use super::messages::ChatMessage;

pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";

pub const DEFAULT_IMAGE_MODEL: &str = "google/gemini-3.1-flash-image";

/// 使用者語系：preferences.language，預設 zh-TW；決定 system prompt 注入哪份語言規範
pub fn ui_language(config: &AppConfig) -> String {
    config
        .preferences
        .get("language")
        .and_then(|value| value.as_str())
        .unwrap_or("zh-TW")
        .to_owned()
}

/// GM 檔位：preferences.gm_tier，預設 best（GM 需掌握整體資訊，NewPlan §6.3）
pub fn gm_tier(config: &AppConfig) -> Tier {
    config
        .preferences
        .get("gm_tier")
        .and_then(|value| value.as_str())
        .and_then(|value| Tier::parse(value).ok())
        .unwrap_or(Tier::Best)
}

/// 重構展開檔位：展開／重寫照盤點規格產出，下放 balanced 省費（survey 留 GM 檔）；
/// API 模式未設 balanced 模型時退 GM 檔讓按鈕照常能用（同 translate_opening 慣例），
/// CLI 檔位一律有內建對應不用退。
pub fn refactor_expand_tier(config: &AppConfig, transport_kind: &str) -> Tier {
    if transport_kind == "api" && resolve_model(Tier::Balanced, config).is_err() {
        gm_tier(config)
    } else {
        Tier::Balanced
    }
}

/// 檔位→模型解析。模型 id 一律來自設定檔（config.tier_models），程式不內建。
pub fn resolve_model(tier: Tier, config: &AppConfig) -> Result<String, String> {
    let key = tier.as_str();
    config
        .tier_models
        .get(key)
        .cloned()
        .filter(|model| !model.is_empty())
        .ok_or_else(|| format!("尚未設定「{key}」檔位對應的模型，請先到設定填寫"))
}

/// 開場白翻譯的檔位挑選器要顯示的「這一檔實際會叫哪個模型」。解析與 `stream_via_transport`
/// 同源：tier_models 有覆寫就是覆寫值，沒有才是 CLI 內建對應——前端自己拼會拼錯（同樣是
/// 「低」檔，設了 claude:fast 的機器跑 claude-haiku-4-5，沒設的跑別名 haiku）。
/// 文案留給前端組：model=None 代表走 CLI 預設模型，後端不吐中文。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TierModel {
    /// 被問的檔位
    pub tier: String,
    /// 實際生效的檔位：API 模式該檔位沒設模型時會退 GM 檔（translate_opening 既有慣例）
    pub effective_tier: String,
    /// 實際送出的模型 id；None＝用 CLI 預設模型（codex／agy／grok 未覆寫時）
    pub model: Option<String>,
    /// codex 專用：檔位映射到的 reasoning effort，其他傳輸層為 None
    pub effort: Option<String>,
}

pub fn tier_model(config: &AppConfig, transport_kind: &str, tier: Tier) -> TierModel {
    if transport_kind == "api" {
        let (effective, model) = match resolve_model(tier, config) {
            Ok(model) => (tier, Some(model)),
            // 該檔沒設就退 GM 檔；GM 檔也沒設時 model 留 None，前端顯示「未設定」
            Err(_) => {
                let fallback = gm_tier(config);
                (fallback, resolve_model(fallback, config).ok())
            }
        };
        return TierModel {
            tier: tier.as_str().to_owned(),
            effective_tier: effective.as_str().to_owned(),
            model,
            effort: None,
        };
    }
    let override_model =
        crate::cli::tier_override(&config.tier_models, transport_kind, tier).map(str::to_owned);
    let model = match transport_kind {
        // claude 未覆寫時有內建別名，永遠有值
        "claude" => {
            Some(override_model.unwrap_or_else(|| crate::cli::claude_model_for(tier).to_owned()))
        }
        _ => override_model,
    };
    TierModel {
        tier: tier.as_str().to_owned(),
        effective_tier: tier.as_str().to_owned(),
        model,
        effort: (transport_kind == "codex").then(|| crate::cli::codex_effort_for(tier).to_owned()),
    }
}

pub fn base_url(config: &AppConfig) -> String {
    config
        .preferences
        .get("base_url")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim_end_matches('/').to_owned())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned())
}

/// SSE 逐塊解析器。以位元組緩衝避免 UTF-8 字元被 chunk 邊界切斷。
#[derive(Default)]
pub struct SseParser {
    buffer: Vec<u8>,
}

impl SseParser {
    /// 餵入一塊原始位元組，回傳所有完整行的 `data:` 承載內容。
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(chunk);
        let mut payloads = Vec::new();
        while let Some(index) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=index).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\n', '\r']);
            if let Some(payload) = line.strip_prefix("data:") {
                payloads.push(payload.trim_start().to_owned());
            }
            // 其餘：空行（事件分隔）與 ": comment"（OpenRouter 的處理中心跳）一律忽略
        }
        payloads
    }
}

/// 一次呼叫的用量（prompt-cache-optimization C）。API 走 OpenRouter usage accounting，
/// CLI 走各家收尾事件（見 cli::parse_*_usage）。
/// prompt_tokens 一律是「總輸入」（含快取部分），各家語意差異在抽取時就換算掉。
/// cached_tokens／created_tokens 是 Option，`None` 與 `Some(0)` 是**兩件事**：
/// None＝這條路沒回報快取欄位（量不到，不能宣稱沒命中）；Some(0)＝量到了，這輪沒中。
/// 兩者曾被 `unwrap_or(0)` 壓成同一個 0，額度分頁因此對 API 路顯示假的 0.0%
/// （2026-08-21 取證，見 .ai/plans/api-cache-visibility.md）。
/// created_tokens（寫入快取）是診斷關鍵：命中 0 時，它 >0 代表「有建但沒讀到」
/// （前綴變了或過期），=0 代表「根本沒建快取」；回報寫入數的來源才有值。
/// output_tokens 與 cost_usd 供額度分頁算花費；只有 claude 直接回報金額，其餘為 None。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PromptCacheUsage {
    pub prompt_tokens: u64,
    pub cached_tokens: Option<u64>,
    pub created_tokens: Option<u64>,
    pub output_tokens: u64,
    pub cost_usd: Option<f64>,
}

impl PromptCacheUsage {
    /// 讀自快取的輸入佔總輸入的百分比。沒回報快取欄位＝None（前端顯示「—」），
    /// 不是 0——「量不到」與「沒中」混講會讓玩家去修一個不存在的問題。
    pub fn hit_rate(&self) -> Option<f64> {
        match self.cached_tokens {
            Some(cached) if self.prompt_tokens > 0 => {
                Some(cached as f64 * 100.0 / self.prompt_tokens as f64)
            }
            Some(_) => Some(0.0),
            None => None,
        }
    }

    /// 這條路回不回報快取欄位。log 與報表據此把「量不到」與「沒中」分開。
    pub fn reported(&self) -> bool {
        self.cached_tokens.is_some()
    }
}

/// 診斷輸出用：沒回報就印「—」，不印 0。
pub(crate) fn describe(tokens: Option<u64>) -> String {
    tokens.map_or_else(|| "—".to_owned(), |value| value.to_string())
}

/// 快取欄位在各家 OpenAI-compatible 端點叫不同名字，有哪組抓哪組（回傳 `(讀, 寫)`）。
/// 一組都沒有＝這條路不回報，回 `(None, None)`——不可退成 0（那正是本案根因）。
/// 只認實際會走到這條路的三組：中轉站照抄上游 schema，光認 OpenRouter 那組不夠。
fn cache_tokens(usage: &serde_json::Value) -> (Option<u64>, Option<u64>) {
    let field = |value: &serde_json::Value, key: &str| value.get(key).and_then(|v| v.as_u64());
    let details = usage.get("prompt_tokens_details");
    let nested = |key: &str| details.and_then(|details| field(details, key));
    // 讀與寫各自挑第一個有值的來源：整組提前返回會讓「details 只有寫入數」的回應
    // 遮蔽掉同一包裡的 prompt_cache_hit_tokens（Sol 驗收 2026-08-21）。
    // 順序＝OpenRouter（usage accounting）→ DeepSeek 原生（中轉站照抄這組）→ Anthropic 原生。
    let read = nested("cached_tokens")
        .or_else(|| field(usage, "prompt_cache_hit_tokens"))
        .or_else(|| field(usage, "cache_read_input_tokens"));
    let write =
        nested("cache_write_tokens").or_else(|| field(usage, "cache_creation_input_tokens")); // DeepSeek 不回寫入數
    (read, write)
}

/// 從一則 SSE payload 取出 usage 統計；增量塊的 `"usage": null` 與缺欄位一律回 None。
pub fn extract_usage(payload: &str) -> Option<PromptCacheUsage> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    let usage = value.get("usage")?;
    let prompt_tokens = usage.get("prompt_tokens")?.as_u64()?;
    let (cached_tokens, created_tokens) = cache_tokens(usage);
    Some(PromptCacheUsage {
        prompt_tokens,
        cached_tokens,
        created_tokens,
        output_tokens: usage
            .get("completion_tokens")
            .and_then(|tokens| tokens.as_u64())
            .unwrap_or(0),
        cost_usd: None,
    })
}

/// Anthropic 系模型走顯式快取（prompt-cache-optimization B）：未標 cache_control＝完全不快取。
/// content 轉 multipart 陣列（斷點只能掛在 content 分段上），在穩定前綴尾標
/// `cache_control: {"type": "ephemeral"}`——具體是 system（角色卡／world.md／constant 條目，
/// 換卡前不變）與最後一則 assistant（其後只剩會變動的東西：可能被 push_merged 續寫的
/// 最後一則 user、每輪翻動的動態塊、導演指示）。transcript 逐輪增長，斷點位置跟著前移；
/// Anthropic 查快取時會回看斷點前約 20 個 content block，前一輪寫下的快取點仍在回看範圍內，
/// 逐輪增量命中。斷點上限 4 個，這裡用 2 個。
fn anthropic_messages(messages: &[ChatMessage]) -> serde_json::Value {
    let last_assistant = messages
        .iter()
        .rposition(|message| message.role == "assistant");
    let entries = messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            let mut part = serde_json::json!({ "type": "text", "text": message.content });
            if message.role == "system" || Some(index) == last_assistant {
                part["cache_control"] = serde_json::json!({ "type": "ephemeral" });
            }
            serde_json::json!({ "role": message.role, "content": [part] })
        })
        .collect();
    serde_json::Value::Array(entries)
}

/// chat/completions 請求本體。曾對 OpenRouter 端點附掛 `usage:{include:true}`，
/// 官方已將該參數（與 `stream_options:{include_usage:true}`）標為 deprecated 且無作用——
/// 完整 usage 一律自動回在尾塊，帶著只會讓嚴格的端點（OpenAI 官方）拒絕請求。
/// anthropic/ 系模型另走顯式快取斷點（見 anthropic_messages）；不適用時請求形狀維持素樸。
fn chat_request_body(model: &str, messages: &[ChatMessage]) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });
    if model.starts_with("anthropic/") {
        body["messages"] = anthropic_messages(messages);
    }
    body
}

/// 從一則 SSE payload 取出增量文字；非增量塊（usage、空 choices）回 None。
pub fn extract_delta(payload: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    let text = value
        .get("choices")?
        .get(0)?
        .get("delta")?
        .get("content")?
        .as_str()?;
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

/// 串流全程累積的收工訊號。判「這次呼叫算不算成功」靠的是 content 以外的欄位：
/// 供應商中途塞的 error 塊、finish_reason、有沒有見到 [DONE]。
/// 這些現在全被丟掉，於是「思考完但零內容」會冒充成功（見 .ai/plans/stream-failure-visible.md）。
#[derive(Default)]
pub struct StreamOutcome {
    /// 供應商中途送的錯誤原話（頂層 error，與 finish_reason="error" 同時出現）
    pub error: Option<String>,
    /// choices[0].finish_reason，取最後一則有值的
    pub finish_reason: Option<String>,
    /// usage.completion_tokens_details.reasoning_tokens，只進錯誤診斷小字
    pub reasoning_tokens: Option<u64>,
    /// 有沒有收到 [DONE]：沒有就 EOF＝串流被截斷
    pub saw_done: bool,
}

impl StreamOutcome {
    /// 吸收一則 payload 的訊號。error 與 finish_reason 都取最後一則有值的
    /// （增量塊的 finish_reason 是 null，真正的收尾原因在最後一塊）。
    pub fn absorb(&mut self, payload: &str) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
            return;
        };
        if let Some(error) = value.get("error").filter(|error| !error.is_null()) {
            // message 缺了或不是字串就序列化整包——供應商的錯誤不能靜默吞掉
            self.error = Some(
                error
                    .get("message")
                    .and_then(|message| message.as_str())
                    .filter(|message| !message.trim().is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| error.to_string()),
            );
        }
        if let Some(reason) = value
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(|reason| reason.as_str())
        {
            self.finish_reason = Some(reason.to_owned());
        }
        if let Some(tokens) = value
            .get("usage")
            .and_then(|usage| usage.get("completion_tokens_details"))
            .and_then(|details| details.get("reasoning_tokens"))
            .and_then(|tokens| tokens.as_u64())
        {
            self.reasoning_tokens = Some(tokens);
        }
    }

    /// 收工判定。優先序固定：供應商原話 → 內容過濾 → 不完整 → 正文空 → 成功。
    /// 順序決定歸類：length 又零正文歸 INCOMPLETE（原因是被截斷），不歸 EMPTY。
    /// 供應商原話不加碼原樣拋——免費層 429 的原話能被 ai-error.ts 既有的額度正則接住，
    /// 玩家看到「額度用完」比看到一句籠統的串流錯誤有用。
    /// 回傳 Some(錯誤字串)＝失敗，None＝成功。
    pub fn failure(&self, text: &str, model: &str) -> Option<String> {
        if let Some(error) = &self.error {
            return Some(error.clone());
        }
        let reason = self.finish_reason.as_deref();
        let diagnosis = format!(
            "model={model} finish_reason={}{}",
            reason.unwrap_or("(無)"),
            self.reasoning_tokens
                .map(|tokens| format!(" reasoning_tokens={tokens}"))
                .unwrap_or_default(),
        );
        match reason {
            Some("content_filter") => Some(format!("AI_CONTENT_FILTERED: {diagnosis}")),
            // stop 以外的收尾原因（length／tool_calls／沒見過的）這個 app 都接不下去
            Some(reason) if reason != "stop" => {
                Some(format!("AI_INCOMPLETE_RESPONSE: {diagnosis}"))
            }
            // 沒收尾原因又沒見到 [DONE]＝串流被中途截斷
            None if !self.saw_done => Some(format!("AI_INCOMPLETE_RESPONSE: {diagnosis}")),
            _ if text.trim().is_empty() => Some(format!("AI_EMPTY_RESPONSE: {diagnosis}")),
            _ => None,
        }
    }
}

/// 非 2xx 的錯誤字串：開頭掛穩定碼給前端分流（比照 AI_EMPTY_RESPONSE 慣例），
/// 後面照舊附人看得懂的狀態與原文。前端只認開頭那個碼、不解析 body——
/// 聚合 router 常把上游錯誤整包塞進 body，body 裡的數字（如轉包的 429）
/// 不該蓋掉真正的 HTTP 狀態。
///
/// 原文留到 2000 字：request id、欄位細節、說明網址常在後段，玩家要拿這串去問供應商。
/// 真的超長才截，並且明講截了——看似完整其實殘缺的 JSON 比明說截斷更難查。
fn http_error(status: reqwest::StatusCode, body: &str) -> String {
    const LIMIT: usize = 2000;
    let kept: String = body.chars().take(LIMIT).collect();
    let cut = if body.chars().nth(LIMIT).is_some() {
        "…（原始回應已截斷）"
    } else {
        ""
    };
    format!(
        "AI_HTTP_STATUS_{}: API 回應 {status}：{kept}{cut}",
        status.as_u16(),
    )
}

/// 單發呼叫 OpenAI-compatible chat/completions（SSE 串流），
/// 每個增量經 on_delta 回傳，結束後回傳完整文字。
/// usage_log 給路徑就把這次呼叫的用量追加成一行 JSONL（見 crate::usage_log）；
/// `shape` 是隨行的唯讀情報，只用來標帳本的 mode。
#[allow(clippy::too_many_arguments)]
pub async fn stream_chat(
    config: &AppConfig,
    model: &str,
    messages: &[ChatMessage],
    usage_log: Option<&std::path::Path>,
    world: Option<&str>,
    shape: crate::usage_log::PromptShape,
    mut on_delta: impl FnMut(&str),
) -> DataResult<String> {
    let base = base_url(config);
    let api_key = config
        .api_keys
        .get("openrouter")
        .filter(|key| !key.is_empty());
    if api_key.is_none() && base == DEFAULT_BASE_URL {
        return Err("尚未設定 OpenRouter API key，請先到設定貼上".into());
    }

    let mut request = reqwest::Client::new()
        .post(format!("{base}/chat/completions"))
        .json(&chat_request_body(model, messages));
    if let Some(key) = api_key {
        request = request.bearer_auth(key);
    }

    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(http_error(status, &body).into());
    }

    let mut stream = response.bytes_stream();
    let mut parser = SseParser::default();
    let mut full_text = String::new();
    let mut usage = None;
    let mut outcome = StreamOutcome::default();
    'outer: while let Some(chunk) = stream.next().await {
        for payload in parser.push(&chunk?) {
            if payload == "[DONE]" {
                outcome.saw_done = true;
                break 'outer;
            }
            outcome.absorb(&payload);
            if let Some(parsed) = extract_usage(&payload) {
                usage = Some(parsed);
            }
            if let Some(delta) = extract_delta(&payload) {
                on_delta(&delta);
                full_text.push_str(&delta);
            }
        }
    }
    if let Some(usage) = usage {
        // stderr 一行（終端機啟動時直接看）＋落檔一行（事後隨時查）
        eprintln!(
            "[prompt-cache] transport=api model={model} prompt_tokens={} cached_tokens={} created_tokens={} hit_rate={}",
            usage.prompt_tokens,
            describe(usage.cached_tokens),
            describe(usage.created_tokens),
            usage
                .hit_rate()
                .map_or_else(|| "—（這條路不回報快取）".to_owned(), |rate| format!("{rate:.0}%")),
        );
        if let Some(path) = usage_log {
            crate::usage_log::append_call(path, world, "api", model, None, shape, usage);
        }
    }
    // 用量照記再判成敗：失敗的呼叫一樣燒了 token，額度分頁不能少算這一筆
    if let Some(failure) = outcome.failure(&full_text, model) {
        return Err(failure.into());
    }
    Ok(full_text)
}

/// OpenRouter 專用 Images API（POST {base}/images）；回傳 data URL 或遠端圖片網址。
pub async fn generate_image(config: &AppConfig, prompt: &str) -> Result<String, String> {
    let api_key = config
        .api_keys
        .get("openrouter")
        .map(String::as_str)
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| "尚未設定 OpenRouter API key".to_owned())?;
    let model = config
        .preferences
        .get("image_model")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(DEFAULT_IMAGE_MODEL);
    let response = reqwest::Client::new()
        .post(format!("{}/images", base_url(config)))
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "aspect_ratio": "2:3",
            "resolution": "1K",
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(http_error(status, &body));
    }
    let value: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
    let image = value.get("data").and_then(|data| data.get(0));
    if let Some(b64) = image
        .and_then(|entry| entry.get("b64_json"))
        .and_then(|value| value.as_str())
    {
        return Ok(format!("data:image/png;base64,{b64}"));
    }
    if let Some(url) = image
        .and_then(|entry| entry.get("url"))
        .and_then(|value| value.as_str())
    {
        if url.starts_with("http") {
            return Ok(url.to_owned());
        }
    }
    Err("模型沒有回傳圖片".to_owned())
}

#[cfg(test)]
mod tests;
