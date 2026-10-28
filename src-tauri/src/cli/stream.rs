use super::types::CliLine;
use crate::transport::PromptCacheUsage;
use crate::ui_msg::UiMsg;

/// claude --output-format stream-json 逐行解析：
/// text_delta 進正文；thinking_delta 只餵進度顯示（signature 仍略過）；result 事件收尾。
pub fn parse_claude_line(line: &str) -> CliLine {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return CliLine::Other;
    };
    match value.get("type").and_then(|t| t.as_str()) {
        Some("stream_event") => {
            let delta = value.pointer("/event/delta");
            let kind = delta.and_then(|d| d.get("type")).and_then(|t| t.as_str());
            match (
                kind,
                delta.and_then(|d| d.get("text")).and_then(|t| t.as_str()),
                delta
                    .and_then(|d| d.get("thinking"))
                    .and_then(|t| t.as_str()),
            ) {
                (Some("text_delta"), Some(text), _) => CliLine::Delta(text.to_owned()),
                // opus 4.7 世代 CLI 隱去思考本文（thinking 恆空、只剩 estimated_tokens），
                // 空增量轉一顆心跳點（約每 50 tok 一筆），字尾才看得出「在想」不是「掛了」。
                (Some("thinking_delta"), _, Some(text)) => CliLine::Thinking(if text.is_empty() {
                    "⋯".to_owned()
                } else {
                    text.to_owned()
                }),
                _ => CliLine::Other,
            }
        }
        Some("result") => claude_result(&value),
        _ => CliLine::Other,
    }
}

/// claude 收尾事件。兩種結構化欄位優先於字面：
/// `terminal_reason: "prompt_too_long"`＝輸入超過容量（字面改版也認得）；
/// `stop_reason: "model_context_window_exceeded"`＝生成途中撞到總容量而停，正文不完整，當失敗。
fn claude_result(value: &serde_json::Value) -> CliLine {
    let text = value
        .get("result")
        .and_then(|r| r.as_str())
        .unwrap_or_default()
        .to_owned();
    let is_error = value
        .get("is_error")
        .and_then(|b| b.as_bool())
        .unwrap_or(false);
    let field = |key: &str| value.get(key).and_then(|v| v.as_str());
    // 結構化欄位說太長就一律失敗，不看 is_error（缺省或 false 也不能當成功）
    if field("terminal_reason") == Some("prompt_too_long") {
        let text = match crate::transport::context_overflow::message_says_too_long(&text) {
            true => text,
            false => format!("Prompt is too long: {text}"),
        };
        return CliLine::Done {
            text,
            is_error: true,
        };
    }
    if field("stop_reason") == Some("model_context_window_exceeded") {
        return CliLine::Done {
            text: "AI_INCOMPLETE_RESPONSE: finish_reason=model_context_window_exceeded".to_owned(),
            is_error: true,
        };
    }
    CliLine::Done { text, is_error }
}

/// claude 的 rate_limit_event：這一發是否落在 Claude 訂閱的超額用量。
/// `isUsingOverage:true` 且超額沒被拒（allowed／allowed_warning 或缺欄）＝Some(true)；
/// 被拒（rejected）或沒在用超額＝Some(false)；不是這種事件＝None。
pub(crate) fn claude_overage(line: &str) -> Option<bool> {
    if !line.contains("\"rate_limit_event\"") {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("type").and_then(|t| t.as_str()) != Some("rate_limit_event") {
        return None;
    }
    let info = value.get("rate_limit_info")?;
    let using = info
        .get("isUsingOverage")
        .and_then(serde_json::Value::as_bool)
        == Some(true);
    let rejected = info.get("overageStatus").and_then(|s| s.as_str()) == Some("rejected");
    Some(using && !rejected)
}

/// 各家收尾事件裡的 token 用量。usage 只出現在收尾那一行，增量行不含 "usage" 字串——
/// 先做字串預檢，串流上千行也只有收尾那行真的解析 JSON。
fn usage_event(line: &str, kind: &str) -> Option<serde_json::Value> {
    if !line.contains("\"usage\"") {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("type").and_then(|t| t.as_str()) != Some(kind) {
        return None;
    }
    Some(value)
}

fn token_count(usage: &serde_json::Value, field: &str) -> u64 {
    usage.get(field).and_then(|v| v.as_u64()).unwrap_or(0)
}

/// claude result 事件的用量。input_tokens **不含**快取部分（實測：讀滿快取時 input_tokens=1、
/// cache_read=4771），總輸入要把建快取與讀快取加回來。
/// 四支 CLI 只有 claude 直接回報金額（total_cost_usd），額度分頁的花費以它為準。
pub fn parse_claude_usage(line: &str) -> Option<PromptCacheUsage> {
    let value = usage_event(line, "result")?;
    let usage = value.get("usage")?;
    let cached = token_count(usage, "cache_read_input_tokens");
    let created = token_count(usage, "cache_creation_input_tokens");
    // 1h 拆分缺欄、空物件、null 或非數字都算未知，不當成「全寫 5 分鐘」
    let created_1h = usage
        .get("cache_creation")
        .and_then(|split| split.get("ephemeral_1h_input_tokens"))
        .and_then(serde_json::Value::as_u64)
        .map(|one_hour| one_hour.min(created));
    Some(PromptCacheUsage {
        created_1h_tokens: created_1h,
        prompt_tokens: token_count(usage, "input_tokens") + created + cached,
        cached_tokens: Some(cached),
        created_tokens: Some(created),
        output_tokens: token_count(usage, "output_tokens"),
        cost_usd: value.get("total_cost_usd").and_then(|cost| cost.as_f64()),
    })
}

/// codex turn.completed 事件的用量。input_tokens **已含** cached_input_tokens
/// （codex 自己的顯示邏輯是「非快取輸入＝input − cached」），不再加總。
pub fn parse_codex_usage(line: &str) -> Option<PromptCacheUsage> {
    let value = usage_event(line, "turn.completed")?;
    let usage = value.get("usage")?;
    Some(PromptCacheUsage {
        created_1h_tokens: None,
        prompt_tokens: token_count(usage, "input_tokens"),
        cached_tokens: Some(token_count(usage, "cached_input_tokens")),
        created_tokens: Some(token_count(usage, "cache_write_input_tokens")),
        output_tokens: token_count(usage, "output_tokens"),
        cost_usd: None,
    })
}

/// grok end 事件的用量。input_tokens **不含** cache_read_input_tokens
/// （實測讀取數可遠大於輸入數：input=31509、cache_read=146304），總輸入要加總。
/// grok 不回報寫入快取的 token 數（實測 usage 只有 read），created 為 None（沒回報，不是 0）。
/// 金額在 end 事件頂層的 total_cost_usd（grok 0.2.111 實測），缺欄照慣例當 None。
pub fn parse_grok_usage(line: &str) -> Option<PromptCacheUsage> {
    let value = usage_event(line, "end")?;
    let usage = value.get("usage")?;
    let cached = token_count(usage, "cache_read_input_tokens");
    Some(PromptCacheUsage {
        created_1h_tokens: None,
        prompt_tokens: token_count(usage, "input_tokens") + cached,
        cached_tokens: Some(cached),
        created_tokens: None,
        output_tokens: token_count(usage, "output_tokens"),
        cost_usd: value.get("total_cost_usd").and_then(|cost| cost.as_f64()),
    })
}

/// agy result 事件的用量（agy 1.1.8 起回報，1.1.17 實測）。欄位在 `result.usage`，
/// 事件的判別鍵是 `event` 而非 `type`，所以不走 `usage_event`。
/// agy 不回報寫入快取的 token 數，created 為 None（沒回報，不是 0）；也不回報金額。
///
/// 官方 headless 契約的 `input_tokens` 是總輸入（含 `cache_read_tokens`），
/// `total_tokens = input_tokens + output_tokens`。舊版實測曾出現「input 不含 cache」的組合，
/// 所以對得上舊式時仍相容；但 `total_tokens` 若只是後端聚合誤差，不得把明確回報的
/// `cache_read_tokens` 整個丟掉。原始五個 usage 計數另寫入帳本，方便追查契約漂移。
pub fn parse_agy_usage(line: &str) -> Option<PromptCacheUsage> {
    if !line.contains("\"usage\"") {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("event").and_then(|event| event.as_str()) != Some("result") {
        return None;
    }
    let usage = value.pointer("/result/usage")?;
    let input = token_count(usage, "input_tokens");
    let output = token_count(usage, "output_tokens");
    let cached = token_count(usage, "cache_read_tokens");
    let total = token_count(usage, "total_tokens");
    let (prompt_tokens, cached_tokens) = if total == input + output && cached <= input {
        (input, Some(cached)) // 官方契約：input 已含 cache_read
    } else if total == input + cached + output {
        (input + cached, Some(cached)) // input 未含，要加回
    } else if cached <= input {
        (input, Some(cached)) // total 漂移，但 input/cache 本身仍是明確計數
    } else {
        (input, None) // 兩式都對不上：記數字、不產生命中率
    };
    Some(PromptCacheUsage {
        created_1h_tokens: None,
        prompt_tokens,
        cached_tokens,
        created_tokens: None,
        output_tokens: output,
        cost_usd: None,
    })
}

/// 擷取 Agy conversation 的累積計數。內容欄位不會離開 parser。
pub fn agy_usage_counters(line: &str) -> Option<super::types::AgyUsageCounters> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("event").and_then(|event| event.as_str()) != Some("result") {
        return None;
    }
    let usage = value.pointer("/result/usage")?;
    Some(super::types::AgyUsageCounters {
        input_tokens: token_count(usage, "input_tokens"),
        output_tokens: token_count(usage, "output_tokens"),
        cache_read_tokens: token_count(usage, "cache_read_tokens"),
    })
}

/// Agy resume 的 result 會把 conversation 至今的 input/output/cache 都累加。
/// 目前 CLI 的 input 不含 cache read，因此本輪總輸入是兩個增量的和。
/// 任一計數倒退代表契約或 conversation 已變，交回 None 讓呼叫端保留原始解析而不亂猜。
pub fn parse_agy_usage_delta(
    current: super::types::AgyUsageCounters,
    base: super::types::AgyUsageCounters,
) -> Option<PromptCacheUsage> {
    let input = current.input_tokens.checked_sub(base.input_tokens)?;
    let output = current.output_tokens.checked_sub(base.output_tokens)?;
    let cached = current
        .cache_read_tokens
        .checked_sub(base.cache_read_tokens)?;
    Some(PromptCacheUsage {
        created_1h_tokens: None,
        prompt_tokens: input.saturating_add(cached),
        cached_tokens: Some(cached),
        created_tokens: None,
        output_tokens: output,
        cost_usd: None,
    })
}

/// 從 Agy result 只擷取非內容型的原始用量證據；不保存 prompt/response。
pub(super) fn agy_usage_evidence(
    line: &str,
    init_conversation_id: Option<&str>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("event").and_then(|event| event.as_str()) != Some("result") {
        return None;
    }
    let result = value.get("result")?;
    let usage = result.get("usage")?;
    let mut fields = serde_json::Map::new();
    let conversation_id = result
        .get("conversation_id")
        .and_then(|value| value.as_str())
        .or(init_conversation_id);
    if let Some(id) = conversation_id.filter(|id| !id.is_empty()) {
        fields.insert("agy_conversation_id".to_owned(), serde_json::json!(id));
    }
    for key in [
        "input_tokens",
        "output_tokens",
        "thinking_tokens",
        "cache_read_tokens",
        "total_tokens",
    ] {
        if let Some(count) = usage.get(key).and_then(|value| value.as_u64()) {
            fields.insert(format!("agy_{key}"), serde_json::json!(count));
        }
    }
    if let Some(turns) = result.get("num_turns").and_then(|value| value.as_u64()) {
        fields.insert("agy_num_turns".to_owned(), serde_json::json!(turns));
    }
    if let Some(seconds) = result
        .get("duration_seconds")
        .and_then(|value| value.as_f64())
    {
        fields.insert(
            "agy_duration_seconds".to_owned(),
            serde_json::json!(seconds),
        );
    }
    Some(fields)
}

pub(super) fn agy_conversation_id(line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    value
        .get("conversation_id")
        .or_else(|| value.pointer("/result/conversation_id"))
        .and_then(|value| value.as_str())
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

/// codex exec --json 逐行解析：agent_message 為增量（通常一則），turn.completed 收尾。
/// item.type=="error" 可能只是非致命警告（例如 hooks 提示），不當失敗。
pub fn parse_codex_line(line: &str) -> CliLine {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return CliLine::Other;
    };
    match value.get("type").and_then(|t| t.as_str()) {
        Some("item.completed") => {
            let item = value.get("item");
            let kind = item.and_then(|i| i.get("type")).and_then(|t| t.as_str());
            match (
                kind,
                item.and_then(|i| i.get("text")).and_then(|t| t.as_str()),
            ) {
                // 一回合可能有多則 agent_message（前導說明＋結論），補換行才不會黏成一句
                (Some("agent_message"), Some(text)) => CliLine::Delta(format!("{text}\n")),
                _ => CliLine::Other,
            }
        }
        Some("turn.completed") => CliLine::Done {
            text: String::new(),
            is_error: false,
        },
        Some("turn.failed") | Some("error") => CliLine::Done {
            text: value
                .pointer("/error/message")
                .or_else(|| value.get("message"))
                .and_then(|m| m.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| turn_failed("Codex")),
            is_error: true,
        },
        _ => CliLine::Other,
    }
}

/// 收尾事件報失敗卻沒附原因時的說明；cli 是顯示名。
fn turn_failed(cli: &str) -> String {
    UiMsg::CliTurnFailed {
        cli: cli.to_owned(),
    }
    .to_string()
}

/// agy --output-format stream-json 逐行解析（1.1.17 實測）：正文增量在
/// `step_update.text_delta`（`step_type=="agent_response"`，ACTIVE 與 DONE 兩種狀態都可能帶），
/// `result` 事件收尾。`result.response` 是全文重述，**不可**當增量送出，否則正文會出現兩次。
pub fn parse_agy_line(line: &str) -> CliLine {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return CliLine::Other;
    };
    match value.get("event").and_then(|event| event.as_str()) {
        Some("step_update") => {
            let step = value.get("step_update");
            let is_response = step
                .and_then(|step| step.get("step_type"))
                .and_then(|kind| kind.as_str())
                == Some("agent_response");
            match (is_response, step.and_then(|step| step.get("text_delta"))) {
                (true, Some(text)) => text
                    .as_str()
                    .map(|text| CliLine::Delta(text.to_owned()))
                    .unwrap_or(CliLine::Other),
                _ => CliLine::Other,
            }
        }
        Some("result") => {
            let status = value
                .pointer("/result/status")
                .and_then(|status| status.as_str())
                .unwrap_or("");
            match status {
                // response 是全文重述：run_cli 只在完全沒收到增量時才用它（見 full_text
                // is_empty 那段），所以放進來是零增量的保險，不會讓正文重播兩次
                "SUCCESS" => CliLine::Done {
                    text: value
                        .pointer("/result/response")
                        .and_then(|text| text.as_str())
                        .unwrap_or_default()
                        .to_owned(),
                    is_error: false,
                },
                other => CliLine::Done {
                    text: match other.is_empty() {
                        true => turn_failed("Gemini"),
                        false => UiMsg::CliTurnFailedStatus {
                            cli: "Gemini".to_owned(),
                            status: other.to_owned(),
                        }
                        .to_string(),
                    },
                    is_error: true,
                },
            }
        }
        _ => CliLine::Other,
    }
}

/// grok --output-format streaming-json 逐行解析：thought 不進對話，text 為增量，end 正常收尾。
pub fn parse_grok_line(line: &str) -> CliLine {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return CliLine::Other;
    };
    match value.get("type").and_then(|t| t.as_str()) {
        Some("text") => value
            .get("data")
            .and_then(|data| data.as_str())
            .map(|text| CliLine::Delta(text.to_owned()))
            .unwrap_or(CliLine::Other),
        // 思考：聊天不進畫面（thinking_to_delta=false），但要過失控的退化偵測
        Some("thought") => value
            .get("data")
            .and_then(|data| data.as_str())
            .map(|text| CliLine::Thinking(text.to_owned()))
            .unwrap_or(CliLine::Other),
        Some("end") => grok_end(&value),
        Some("error") => CliLine::Done {
            text: value
                .get("data")
                .or_else(|| value.get("message"))
                .and_then(|message| message.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| turn_failed("Grok")),
            is_error: true,
        },
        _ => CliLine::Other,
    }
}

/// grok 的 `end` 收尾，照官方 headless 文件（streaming-json）的 stopReason 逐一分類：
/// end_turn＝完成；max_tokens／max_turn_requests＝正文不完整，比照 claude 掛 `AI_INCOMPLETE_RESPONSE:`；
/// refusal、cancelled、缺欄或文件沒列的值＝這輪沒完成。大小寫與底線不拘
/// （1.0.70 寫 `end_turn`，較舊版寫 `EndTurn`）。
fn grok_end(value: &serde_json::Value) -> CliLine {
    let reason = value
        .get("stopReason")
        .and_then(|reason| reason.as_str())
        .unwrap_or_default();
    match reason.replace('_', "").to_ascii_lowercase().as_str() {
        "endturn" => CliLine::Done {
            text: String::new(),
            is_error: false,
        },
        "maxtokens" => incomplete("max_tokens"),
        "maxturnrequests" => incomplete("max_turn_requests"),
        _ => CliLine::Done {
            text: UiMsg::CliTurnFailedStatus {
                cli: "Grok".to_owned(),
                status: match reason.is_empty() {
                    true => "(no stopReason)".to_owned(),
                    false => reason.to_owned(),
                },
            }
            .to_string(),
            is_error: true,
        },
    }
}

fn incomplete(reason: &str) -> CliLine {
    CliLine::Done {
        text: format!("AI_INCOMPLETE_RESPONSE: finish_reason={reason}"),
        is_error: true,
    }
}

#[cfg(test)]
mod tests;
