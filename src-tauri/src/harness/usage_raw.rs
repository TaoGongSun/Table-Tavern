//! 原始 usage 留證（usage-cache-audit）：供應商收尾事件裡的 usage 原樣寫進 AI log（`usage-raw`），
//! 與 app 解析後落帳的數字並排，才分得出 `cached_tokens: 0` 是真零還是缺欄被補成 0。
//! 只擷取用量與模型／provider 欄位，不帶 prompt 與回覆內容。

use serde_json::{json, Value};

use crate::transport::PromptCacheUsage;

const UNREPORTED: &str = "未回報";

/// 一行收尾事件（CLI stdout 一行或 SSE 一個 payload）→ 原始用量摘要；沒有 usage 物件就是 None。
/// usage 所在：頂層（claude／codex／grok／chat completions）、`result`（agy）、`response`（Responses API）。
pub(crate) fn raw_usage(payload: &str) -> Option<Value> {
    if !payload.contains("\"usage\"") {
        return None;
    }
    let top: Value = serde_json::from_str(payload).ok()?;
    let container = [Some(&top), top.get("result"), top.get("response")]
        .into_iter()
        .flatten()
        .find(|value| value.get("usage").is_some_and(|usage| usage.is_object()))?;
    let model = container
        .get("model")
        .or_else(|| top.get("model"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            // claude 的 result 沒有 model 欄，實際模型只出現在 modelUsage 的鍵
            top.get("modelUsage")
                .and_then(Value::as_object)
                .filter(|models| !models.is_empty())
                .map(|models| models.keys().cloned().collect::<Vec<_>>().join(","))
        })
        .unwrap_or_else(|| UNREPORTED.to_owned());
    let provider = container
        .get("provider")
        .or_else(|| top.get("provider"))
        .and_then(Value::as_str)
        .unwrap_or(UNREPORTED);
    Some(json!({
        "event": top.get("type").or_else(|| top.get("event")),
        "usage": container.get("usage"),
        "modelUsage": top.get("modelUsage"),
        "total_cost_usd": top.get("total_cost_usd"),
        "responder_model": model,
        "provider": provider,
    }))
}

/// app 解析後要落帳的數字（Option 保留 None，看得出「沒回報」與「0」的差別）。
pub(crate) fn parsed_usage(usage: &PromptCacheUsage) -> Value {
    json!({
        "prompt_tokens": usage.prompt_tokens,
        "cached_tokens": usage.cached_tokens,
        "created_tokens": usage.created_tokens,
        "output_tokens": usage.output_tokens,
        "cost_usd": usage.cost_usd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_missing_cache_fields_missing_and_marks_unreported_model() {
        let raw = raw_usage(r#"{"type":"result","result":"台詞","usage":{"input_tokens":12},"modelUsage":{"claude-haiku-4-5":{}}}"#)
            .unwrap();
        assert_eq!(raw["usage"], json!({ "input_tokens": 12 })); // 不補 cache 欄
        assert_eq!(raw["responder_model"], "claude-haiku-4-5");
        assert_eq!(raw["provider"], UNREPORTED);
        assert!(raw.to_string().find("台詞").is_none(), "不帶回覆內容");

        let codex = raw_usage(
            r#"{"type":"turn.completed","usage":{"input_tokens":5,"cached_input_tokens":0}}"#,
        )
        .unwrap();
        assert_eq!(codex["responder_model"], UNREPORTED);
    }

    #[test]
    fn finds_usage_under_result_or_response_and_reads_provider() {
        let agy = raw_usage(r#"{"event":"result","result":{"response":"x","usage":{"input_tokens":9,"cache_read_tokens":4}}}"#)
            .unwrap();
        assert_eq!(agy["usage"]["cache_read_tokens"], 4);
        assert_eq!(agy["event"], "result");
        let api =
            raw_usage(r#"{"model":"z/free","provider":"Chutes","usage":{"prompt_tokens":3}}"#)
                .unwrap();
        assert_eq!(api["responder_model"], "z/free");
        assert_eq!(api["provider"], "Chutes");
        let responses = raw_usage(
            r#"{"type":"response.completed","response":{"model":"m","usage":{"input_tokens":1}}}"#,
        )
        .unwrap();
        assert_eq!(responses["responder_model"], "m");
        // 串流中途 usage:null 的塊不算
        assert!(raw_usage(r#"{"choices":[],"usage":null}"#).is_none());
    }
}
