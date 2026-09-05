use super::types::CliLine;
use crate::transport::PromptCacheUsage;

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
                delta.and_then(|d| d.get("thinking")).and_then(|t| t.as_str()),
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
        Some("result") => CliLine::Done {
            text: value
                .get("result")
                .and_then(|r| r.as_str())
                .unwrap_or_default()
                .to_owned(),
            is_error: value
                .get("is_error")
                .and_then(|b| b.as_bool())
                .unwrap_or(false),
        },
        _ => CliLine::Other,
    }
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
    Some(PromptCacheUsage {
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
/// `input_tokens` 含不含 `cache_read_tokens` 沒有文件可查，實測那筆快取剛好是 0、驗不出來。
/// 這裡不猜，改用 `total_tokens` 當契約判別：對得上哪一式就照哪一式算，兩式都對不上
/// （或讀取數大於輸入數）就回 `cached_tokens: None`——額度分頁顯示「—」代表量不到，
/// 好過用猜測算出一個 >100% 的命中率、讓玩家去修一個不存在的問題。
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
        (input, Some(cached)) // input 已含 cache_read
    } else if total == input + cached + output {
        (input + cached, Some(cached)) // input 未含，要加回
    } else {
        (input, None) // 兩式都對不上：記數字、不產生命中率
    };
    Some(PromptCacheUsage {
        prompt_tokens,
        cached_tokens,
        created_tokens: None,
        output_tokens: output,
        cost_usd: None,
    })
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
                .unwrap_or("CLI 回合失敗")
                .to_owned(),
            is_error: true,
        },
        _ => CliLine::Other,
    }
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
                        true => "Gemini CLI 回合失敗".to_owned(),
                        false => format!("Gemini CLI 回合失敗（{other}）"),
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
        Some("end") => CliLine::Done {
            text: String::new(),
            is_error: false,
        },
        Some("error") => CliLine::Done {
            text: value
                .get("data")
                .or_else(|| value.get("message"))
                .and_then(|message| message.as_str())
                .unwrap_or("Grok CLI 回合失敗")
                .to_owned(),
            is_error: true,
        },
        _ => CliLine::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 樣本取自 2026-07-19 真實 CLI 冒煙輸出（scratchpad claude-smoke.jsonl／codex-smoke.jsonl）
    #[test]
    fn parses_real_claude_stream_json_lines() {
        assert_eq!(
            parse_claude_line(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"測"}}}"#
            ),
            CliLine::Delta("測".to_owned())
        );
        assert_eq!(
            parse_claude_line(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"内心"}}}"#
            ),
            CliLine::Thinking("内心".to_owned())
        );
        // 2026-08-12 實測樣本：4.7 世代 CLI 思考本文隱去，空增量轉心跳點
        assert_eq!(
            parse_claude_line(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"","estimated_tokens":50}}}"#
            ),
            CliLine::Thinking("⋯".to_owned())
        );
        assert_eq!(
            parse_claude_line(
                r#"{"type":"result","subtype":"success","is_error":false,"result":"測試"}"#
            ),
            CliLine::Done {
                text: "測試".to_owned(),
                is_error: false
            }
        );
        assert_eq!(
            parse_claude_line(
                r#"{"type":"result","is_error":true,"result":"Failed to authenticate. API Error: 401 Invalid bearer token"}"#
            ),
            CliLine::Done {
                text: "Failed to authenticate. API Error: 401 Invalid bearer token".to_owned(),
                is_error: true
            }
        );
        assert_eq!(parse_claude_line("not json"), CliLine::Other);
    }

    /// 樣本取自 2026-08-03 真實冒煙輸出（同一段 system prompt 連跑兩次：第一次建快取、
    /// 第二次全命中）。各家 input_tokens 語意不同，這裡鎖住換算後的「總輸入／讀快取」。
    #[test]
    fn parses_claude_usage_adding_cache_tokens_to_input() {
        // 第一次：建快取 4771，讀 0 → 總輸入 4772、命中 0%；金額直接取 CLI 回報值
        assert_eq!(
            parse_claude_usage(
                r#"{"type":"result","total_cost_usd":0.0179,"usage":{"input_tokens":1,"cache_creation_input_tokens":4771,"cache_read_input_tokens":0,"output_tokens":3}}"#
            ),
            Some(PromptCacheUsage {
                prompt_tokens: 4772,
                cached_tokens: Some(0),
                created_tokens: Some(4771),
                output_tokens: 3,
                cost_usd: Some(0.0179),
            })
        );
        // 第二次：讀滿 4771 → 總輸入 4772、命中 100%
        let hit = parse_claude_usage(
            r#"{"type":"result","usage":{"input_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":4771,"output_tokens":3}}"#
        )
        .expect("result 事件有 usage");
        assert_eq!(hit.prompt_tokens, 4772);
        assert_eq!(hit.cached_tokens, Some(4771));
        assert!((hit.hit_rate().expect("claude 有回報") - 99.98).abs() < 0.01);
        // 增量行與非收尾事件不出數字
        assert_eq!(
            parse_claude_usage(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"好"}}}"#
            ),
            None
        );
        assert_eq!(parse_claude_usage("not json"), None);
    }

    /// codex 的 input_tokens 已含 cached_input_tokens，不可再加總（會虛報分母、低估命中率）。
    #[test]
    fn parses_codex_usage_without_double_counting_cached_input() {
        let usage = parse_codex_usage(
            r#"{"type":"turn.completed","usage":{"input_tokens":23144,"cached_input_tokens":11008,"cache_write_input_tokens":0,"output_tokens":5}}"#
        )
        .expect("turn.completed 有 usage");
        assert_eq!(usage.prompt_tokens, 23144);
        assert_eq!(usage.cached_tokens, Some(11008));
        assert!((usage.hit_rate().expect("codex 有回報") - 47.56).abs() < 0.01);
        assert_eq!(parse_codex_usage(r#"{"type":"turn.started"}"#), None);
    }

    /// grok 的 cache_read 可遠大於 input_tokens，證明兩者不重疊，總輸入要加總。
    /// 金額在頂層 total_cost_usd（包 6 額度分頁要顯示），缺欄時仍能解析、只是沒金額。
    #[test]
    fn parses_grok_usage_adding_cache_read_to_input() {
        let usage = parse_grok_usage(
            r#"{"type":"end","stopReason":"EndTurn","total_cost_usd":0.0421,"usage":{"input_tokens":31509,"cache_read_input_tokens":146304,"output_tokens":1416,"total_tokens":179229}}"#
        )
        .expect("end 事件有 usage");
        assert_eq!(usage.prompt_tokens, 177813);
        assert_eq!(usage.cached_tokens, Some(146304));
        assert_eq!(usage.created_tokens, None); // grok 不回報寫入數：None 不是 0
        assert_eq!(usage.cost_usd, Some(0.0421));
        assert!((usage.hit_rate().expect("grok 有回報") - 82.28).abs() < 0.01);
        assert_eq!(
            parse_grok_usage(r#"{"type":"end","usage":{"input_tokens":10}}"#)
                .expect("缺金額欄仍解析")
                .cost_usd,
            None
        );
        assert_eq!(parse_grok_usage(r#"{"type":"text","data":"好"}"#), None);
    }

    /// 缺欄位當 0，不可 panic 也不可整筆丟掉（CLI 版本變動時仍留下可讀的一行）。
    #[test]
    fn missing_usage_fields_count_as_zero() {
        assert_eq!(
            parse_claude_usage(r#"{"type":"result","usage":{"input_tokens":12}}"#),
            Some(PromptCacheUsage {
                prompt_tokens: 12,
                cached_tokens: Some(0),
                created_tokens: Some(0),
                output_tokens: 0,
                cost_usd: None, // 沒回報金額就不記，額度分頁靠有值的那些行加總
            })
        );
        assert_eq!(
            PromptCacheUsage {
                prompt_tokens: 0,
                cached_tokens: Some(0),
                created_tokens: Some(0),
                output_tokens: 0,
                cost_usd: None,
            }
            .hit_rate(),
            Some(0.0)
        );
    }

    #[test]
    fn parses_real_codex_json_lines_and_ignores_warning_items() {
        assert_eq!(
            parse_codex_line(
                r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"測試"}}"#
            ),
            CliLine::Delta("測試\n".to_owned())
        );
        // 非致命警告（真實輸出：hooks 提示）不可視為失敗
        assert_eq!(
            parse_codex_line(
                r#"{"type":"item.completed","item":{"id":"item_0","type":"error","message":"skipping async hook"}}"#
            ),
            CliLine::Other
        );
        assert_eq!(
            parse_codex_line(r#"{"type":"turn.completed","usage":{"input_tokens":15208}}"#),
            CliLine::Done {
                text: String::new(),
                is_error: false
            }
        );
        assert_eq!(
            parse_codex_line(r#"{"type":"turn.failed","error":{"message":"quota exceeded"}}"#),
            CliLine::Done {
                text: "quota exceeded".to_owned(),
                is_error: true
            }
        );
    }

    #[test]
    /// 樣本取自 agy 1.1.17 實跑（`-p ... --output-format stream-json`）。
    fn parses_agy_stream_json_events() {
        // 正文增量：ACTIVE 與 DONE 兩種狀態都可能帶 text_delta，都要進正文
        assert_eq!(
            parse_agy_line(
                r#"{"event":"step_update","step_update":{"step_index":2,"state":"ACTIVE","step_type":"agent_response","text_delta":"好的"}}"#
            ),
            CliLine::Delta("好的".to_owned())
        );
        assert_eq!(
            parse_agy_line(
                r#"{"event":"step_update","step_update":{"step_index":2,"state":"DONE","step_type":"agent_response","text_delta":"\n"}}"#
            ),
            CliLine::Delta("\n".to_owned())
        );
        // 非 agent_response 的步驟不進正文
        assert_eq!(
            parse_agy_line(
                r#"{"event":"step_update","step_update":{"step_index":1,"state":"DONE","step_type":"checkpoint"}}"#
            ),
            CliLine::Other
        );
        assert_eq!(parse_agy_line(r#"{"event":"init","init":{"cwd":"/x"}}"#), CliLine::Other);
        // result 收尾：response 是全文重述，不可當增量，否則正文出現兩次
        // response 進 Done.text 當零增量 fallback：run_cli 只在完全沒收到增量時才用它
        assert_eq!(
            parse_agy_line(r#"{"event":"result","result":{"status":"SUCCESS","response":"好的\n"}}"#),
            CliLine::Done {
                text: "好的\n".to_owned(),
                is_error: false,
            }
        );
        // 非 SUCCESS 仍要失敗，不能靜默當成功
        assert!(matches!(
            parse_agy_line(r#"{"event":"result","result":{"status":"ERROR"}}"#),
            CliLine::Done { is_error: true, .. }
        ));
        assert!(matches!(
            parse_agy_line(r#"{"event":"result","result":{}}"#),
            CliLine::Done { is_error: true, .. }
        ));
        assert_eq!(parse_agy_line("不是 JSON"), CliLine::Other);

        // 整段實跑逐行餵進去：增量串起來要跟 result.response 一字不差
        // （少一段＝正文被吞，多一段＝result.response 被當增量重播）
        let captured = [
            r#"{"event":"init","conversation_id":"c1","init":{"cwd":"/x"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":0,"state":"DONE","step_type":"user_input"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":1,"state":"DONE","step_type":"checkpoint"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"ACTIVE","step_type":"agent_response","text_delta":"好的"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"DONE","step_type":"agent_response","text_delta":"\n","usage":{"input_tokens":14827}}}"#,
            r#"{"event":"result","result":{"status":"SUCCESS","response":"好的\n","usage":{"input_tokens":14827,"output_tokens":282,"cache_read_tokens":0}}}"#,
        ];
        let body: String = captured
            .iter()
            .filter_map(|line| match parse_agy_line(line) {
                CliLine::Delta(text) => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(body, "好的\n");
    }

    #[test]
    /// agy 1.1.8 起才有 usage；欄位在 result.usage，判別鍵是 event 不是 type。
    fn parses_agy_usage_from_result_event() {
        let usage = parse_agy_usage(
            r#"{"event":"result","result":{"status":"SUCCESS","usage":{"input_tokens":14827,"output_tokens":282,"thinking_tokens":281,"cache_read_tokens":1024,"total_tokens":15109}}}"#,
        )
        .expect("result 事件要解得出用量");
        assert_eq!(usage.prompt_tokens, 14_827); // input 已含 cache_read，不加總
        assert_eq!(usage.cached_tokens, Some(1_024));
        assert_eq!(usage.created_tokens, None); // agy 不回報寫入數，沒回報不是 0
        assert_eq!(usage.output_tokens, 282);
        assert_eq!(usage.cost_usd, None); // agy 不回報金額
        // step_update 也帶 usage，但只有 result 是全回合總計
        assert!(parse_agy_usage(
            r#"{"event":"step_update","step_update":{"step_type":"agent_response","usage":{"input_tokens":1}}}"#
        )
        .is_none());
        assert!(parse_agy_usage(r#"{"event":"result","result":{"status":"SUCCESS"}}"#).is_none());
    }

    /// `input_tokens` 含不含 `cache_read_tokens` 沒有文件可查，靠 `total_tokens` 判別。
    /// 對不上就不產生命中率——寧可顯示「—」，也不要猜出一個 >100% 的數字。
    #[test]
    fn agy_usage_picks_contract_by_total_and_bails_out_when_neither_fits() {
        let build = |input: u64, cached: u64, output: u64, total: u64| {
            parse_agy_usage(&format!(
                r#"{{"event":"result","result":{{"usage":{{"input_tokens":{input},"cache_read_tokens":{cached},"output_tokens":{output},"total_tokens":{total}}}}}}}"#
            ))
            .expect("有 usage 就要解得出來")
        };
        // total = input + output → input 已含 cache_read
        let included = build(1_000, 400, 100, 1_100);
        assert_eq!(included.prompt_tokens, 1_000);
        assert_eq!(included.cached_tokens, Some(400));
        // total = input + cache_read + output → input 未含，要加回
        let excluded = build(1_000, 400, 100, 1_500);
        assert_eq!(excluded.prompt_tokens, 1_400);
        assert_eq!(excluded.cached_tokens, Some(400));
        // 兩式都對不上：記數字、不產生命中率
        let unknown = build(1_000, 400, 100, 9_999);
        assert_eq!(unknown.prompt_tokens, 1_000);
        assert_eq!(unknown.cached_tokens, None);
        // 讀取數大於輸入數又不符第二式：一樣不猜
        assert_eq!(build(100, 900, 50, 150).cached_tokens, None);
    }

    #[test]
    fn parses_grok_streaming_json_lines() {
        assert_eq!(
            parse_grok_line(r#"{"type":"text","data":"測試"}"#),
            CliLine::Delta("測試".to_owned())
        );
        assert_eq!(
            parse_grok_line(r#"{"type":"thought","data":"推理"}"#),
            CliLine::Other
        );
        assert_eq!(parse_grok_line(r#"{"type":"unknown"}"#), CliLine::Other);
        assert_eq!(parse_grok_line("not json"), CliLine::Other);
        assert_eq!(
            parse_grok_line(r#"{"type":"end","stopReason":"EndTurn"}"#),
            CliLine::Done {
                text: String::new(),
                is_error: false
            }
        );
        assert_eq!(
            parse_grok_line(r#"{"type":"error","data":"quota exceeded"}"#),
            CliLine::Done {
                text: "quota exceeded".to_owned(),
                is_error: true
            }
        );
        assert_eq!(
            parse_grok_line(r#"{"type":"error"}"#),
            CliLine::Done {
                text: "Grok CLI 回合失敗".to_owned(),
                is_error: true
            }
        );
    }

}
