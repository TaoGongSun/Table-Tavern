//! 單輪輸出失控偵測（runaway-output-cap）：模型持續吐字、`stall.rs` 的停滯逾時攔不下時用。
//! 規則逐字元滑動，同一段文字不論怎麼分塊判定都相同。只攔空白型退化與總長度，
//! 不宣稱能辨識一般的非空白亂碼。

use crate::usage::log::PromptShape;

/// 正文累計上限（字元，Unicode scalar）。
const LENGTH_CAP: usize = 30_000;
/// 連續空白達到這個長度就觸發。
const WHITESPACE_RUN: usize = 2_000;
/// 空白比例的滑動窗口長度；窗口滿了才判。
const RATIO_WINDOW: usize = 4_000;
/// 窗口內空白達到這個數（90%）就觸發。
const RATIO_WHITESPACE: usize = RATIO_WINDOW * 9 / 10;
/// CLI 單行上限（位元組）：記憶體上限，與政策無關。
pub(crate) const LINE_CAP_BYTES: usize = 1024 * 1024;

/// 一次呼叫要套哪些檢查。呼叫端一律明確傳入，不從帳本形狀反推。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunawayPolicy {
    /// 字數上限＋退化偵測：劇情輪（角色、GM 旁白、GM 建議）。
    Full,
    /// 只做退化偵測：單發（卡重構、開桌、換幕摘要、翻譯）。
    DegenerateOnly,
    /// 不檢查：生圖、測試通道等。
    Off,
}

impl RunawayPolicy {
    pub fn for_shape(shape: PromptShape) -> Self {
        match shape {
            PromptShape::Turn { .. } => Self::Full,
            PromptShape::Oneshot => Self::DegenerateOnly,
            PromptShape::Image => Self::Off,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunawayReason {
    Length,
    WhitespaceRun,
    WhitespaceRatio,
    /// CLI 單行超過 `LINE_CAP_BYTES`（只有 CLI 會有）。
    LineTooLong,
}

impl RunawayReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Length => "length",
            Self::WhitespaceRun => "whitespace_run",
            Self::WhitespaceRatio => "whitespace_ratio",
            Self::LineTooLong => "line_too_long",
        }
    }
}

/// 後端穩定碼（前端 `errOutputRunaway`、`dispatch::ai_call_failure` 白名單認開頭）。
pub(crate) const RUNAWAY_CODE: &str = "AI_OUTPUT_RUNAWAY:";

pub(crate) fn runaway_message(reason: RunawayReason, chars: usize) -> String {
    format!("{RUNAWAY_CODE} reason={} chars={chars}", reason.as_str())
}

fn non_empty(value: Option<&serde_json::Value>) -> Option<&str> {
    value
        .and_then(|value| value.as_str())
        .filter(|text| !text.is_empty())
}

/// Chat Completions 的思考增量，餵思考那支。兩欄是同一段文字的相容複製（有的供應商兩欄都給），
/// 只取一欄：先取非空的 `delta.reasoning`，沒有才取 `delta.reasoning_content`，否則會重複計數。
pub(crate) fn chat_reasoning(payload: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(payload).ok()?;
    let delta = value.pointer("/choices/0/delta")?;
    non_empty(delta.get("reasoning"))
        .or_else(|| non_empty(delta.get("reasoning_content")))
        .map(str::to_owned)
}

/// Responses 的思考增量（任何 `response.reasoning*.delta`）。
pub(crate) fn responses_reasoning(payload: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(payload).ok()?;
    let kind = value.get("type")?.as_str()?;
    (kind.starts_with("response.reasoning") && kind.ends_with(".delta"))
        .then(|| non_empty(value.get("delta")).map(str::to_owned))
        .flatten()
}

/// 一支檢查器只看一條增量流（正文或思考），兩條分開計。
pub struct RunawayGuard {
    length_cap: Option<usize>,
    degenerate: bool,
    chars: usize,
    run: usize,
    window: Vec<bool>,
    cursor: usize,
    filled: usize,
    whitespace_in_window: usize,
    tripped: Option<RunawayReason>,
}

impl RunawayGuard {
    fn new(length_cap: Option<usize>, degenerate: bool) -> Self {
        Self {
            length_cap,
            degenerate,
            chars: 0,
            run: 0,
            window: if degenerate {
                vec![false; RATIO_WINDOW]
            } else {
                Vec::new()
            },
            cursor: 0,
            filled: 0,
            whitespace_in_window: 0,
            tripped: None,
        }
    }

    /// 正文那支：Full 有字數上限＋退化，DegenerateOnly 只退化，Off 什麼都不查。
    pub fn text(policy: RunawayPolicy) -> Self {
        match policy {
            RunawayPolicy::Full => Self::new(Some(LENGTH_CAP), true),
            RunawayPolicy::DegenerateOnly => Self::new(None, true),
            RunawayPolicy::Off => Self::new(None, false),
        }
    }

    /// 思考那支：字數不設限，Off 以外只做退化偵測。
    pub fn thinking(policy: RunawayPolicy) -> Self {
        Self::new(None, policy != RunawayPolicy::Off)
    }

    /// 已吃進的字元數。
    pub fn chars(&self) -> usize {
        self.chars
    }

    /// 觸發時的錯誤字串：計數用這一支自己的（思考那支觸發時正文可能是 0）。
    pub fn message(&self, reason: RunawayReason) -> String {
        runaway_message(reason, self.chars)
    }

    /// 吃進一段增量；觸發後之後每次都回同一個理由。
    pub fn push(&mut self, delta: &str) -> Option<RunawayReason> {
        if self.tripped.is_some() || (self.length_cap.is_none() && !self.degenerate) {
            if self.tripped.is_none() {
                self.chars += delta.chars().count();
            }
            return self.tripped;
        }
        for ch in delta.chars() {
            self.chars += 1;
            if self.length_cap.is_some_and(|cap| self.chars > cap) {
                self.tripped = Some(RunawayReason::Length);
                break;
            }
            if !self.degenerate {
                continue;
            }
            let space = ch.is_whitespace();
            self.run = if space { self.run + 1 } else { 0 };
            if self.run >= WHITESPACE_RUN {
                self.tripped = Some(RunawayReason::WhitespaceRun);
                break;
            }
            if self.filled == RATIO_WINDOW {
                if self.window[self.cursor] {
                    self.whitespace_in_window -= 1;
                }
            } else {
                self.filled += 1;
            }
            self.window[self.cursor] = space;
            if space {
                self.whitespace_in_window += 1;
            }
            self.cursor = (self.cursor + 1) % RATIO_WINDOW;
            if self.filled == RATIO_WINDOW && self.whitespace_in_window >= RATIO_WHITESPACE {
                self.tripped = Some(RunawayReason::WhitespaceRatio);
                break;
            }
        }
        self.tripped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(policy: RunawayPolicy, text: &str) -> Option<RunawayReason> {
        RunawayGuard::text(policy).push(text)
    }

    #[test]
    fn length_cap_trips_only_past_the_limit() {
        let mut guard = RunawayGuard::text(RunawayPolicy::Full);
        assert_eq!(guard.push(&"字".repeat(LENGTH_CAP)), None);
        assert_eq!(guard.push("多"), Some(RunawayReason::Length));
        // 單一增量本身就超過（codex 一次給整則）
        assert_eq!(
            feed(RunawayPolicy::Full, &"a".repeat(LENGTH_CAP + 1)),
            Some(RunawayReason::Length)
        );
    }

    #[test]
    fn degenerate_only_and_off_never_trip_on_length() {
        let long = "正常的長篇輸出。".repeat(10_000);
        assert_eq!(feed(RunawayPolicy::DegenerateOnly, &long), None);
        assert_eq!(feed(RunawayPolicy::Off, &long), None);
        assert_eq!(feed(RunawayPolicy::Off, &" ".repeat(50_000)), None);
    }

    #[test]
    fn whitespace_run_trips_at_threshold() {
        let below = format!("開頭{}", " ".repeat(WHITESPACE_RUN - 1));
        assert_eq!(feed(RunawayPolicy::Full, &below), None);
        let at = format!("開頭{}", " ".repeat(WHITESPACE_RUN));
        assert_eq!(
            feed(RunawayPolicy::DegenerateOnly, &at),
            Some(RunawayReason::WhitespaceRun)
        );
    }

    #[test]
    fn full_width_space_and_newlines_count_as_whitespace() {
        let mixed: String = "\u{3000}\n\t ".repeat(WHITESPACE_RUN / 4);
        assert_eq!(
            feed(RunawayPolicy::Full, &mixed),
            Some(RunawayReason::WhitespaceRun)
        );
    }

    #[test]
    fn ratio_waits_for_a_full_window() {
        // 開頭就一大段換行（但不到連續上限）＝窗口未滿不判
        let head = "\n".repeat(WHITESPACE_RUN - 1);
        assert_eq!(feed(RunawayPolicy::Full, &head), None);
        // 九成空白、每段不到連續上限：滿窗後才觸發
        let chunk = format!("x{}", " ".repeat(9));
        let mut guard = RunawayGuard::text(RunawayPolicy::Full);
        let mut fed = 0;
        let mut tripped_at = None;
        while fed < RATIO_WINDOW * 2 {
            if let Some(reason) = guard.push(&chunk) {
                tripped_at = Some((fed + chunk.len(), reason));
                break;
            }
            fed += chunk.len();
        }
        let (at, reason) = tripped_at.expect("trips");
        assert_eq!(reason, RunawayReason::WhitespaceRatio);
        assert!(at >= RATIO_WINDOW, "tripped before window filled: {at}");
    }

    #[test]
    fn indented_prose_does_not_trip() {
        let para = "    她推開門，雨聲灌了進來。\n\n    「你終於來了。」\n\n";
        let text = para.repeat(400);
        assert_eq!(feed(RunawayPolicy::DegenerateOnly, &text), None);
    }

    #[test]
    fn run_resets_after_normal_text() {
        // 中間的正文夠長，窗口內空白比例壓在 90% 以下，只測連續計數有歸零
        let text = format!(
            "{}{}{}",
            " ".repeat(WHITESPACE_RUN - 1),
            "正常文字".repeat(200),
            " ".repeat(WHITESPACE_RUN - 1)
        );
        assert_eq!(feed(RunawayPolicy::Full, &text), None);
    }

    #[test]
    fn chunking_does_not_change_the_verdict() {
        let samples = [
            format!("a{}b{}", " ".repeat(1500), " ".repeat(2100)),
            format!("x{}", " ".repeat(9)).repeat(500),
            "正常".repeat(20_000),
        ];
        for sample in samples {
            let whole = {
                let mut guard = RunawayGuard::text(RunawayPolicy::Full);
                (guard.push(&sample), guard.chars())
            };
            let split = {
                let mut guard = RunawayGuard::text(RunawayPolicy::Full);
                let mut verdict = None;
                for ch in sample.chars() {
                    verdict = guard.push(&ch.to_string());
                    if verdict.is_some() {
                        break;
                    }
                }
                (verdict, guard.chars())
            };
            assert_eq!(whole, split);
        }
    }

    #[test]
    fn thinking_guard_follows_policy() {
        let blank = " ".repeat(WHITESPACE_RUN);
        assert_eq!(
            RunawayGuard::thinking(RunawayPolicy::Full).push(&blank),
            Some(RunawayReason::WhitespaceRun)
        );
        assert_eq!(
            RunawayGuard::thinking(RunawayPolicy::DegenerateOnly).push(&blank),
            Some(RunawayReason::WhitespaceRun)
        );
        assert_eq!(
            RunawayGuard::thinking(RunawayPolicy::Off).push(&blank),
            None
        );
        // 思考不設字數上限
        assert_eq!(
            RunawayGuard::thinking(RunawayPolicy::Full).push(&"想".repeat(LENGTH_CAP * 2)),
            None
        );
    }

    #[test]
    fn policy_maps_from_shape() {
        assert_eq!(
            RunawayPolicy::for_shape(PromptShape::Turn {
                roster: 1,
                solo: true
            }),
            RunawayPolicy::Full
        );
        assert_eq!(
            RunawayPolicy::for_shape(PromptShape::Oneshot),
            RunawayPolicy::DegenerateOnly
        );
        assert_eq!(
            RunawayPolicy::for_shape(PromptShape::Image),
            RunawayPolicy::Off
        );
    }

    #[test]
    fn chat_reasoning_takes_one_field_not_both() {
        // 只有一欄：哪一欄都取得到
        assert_eq!(
            chat_reasoning(r#"{"choices":[{"delta":{"reasoning":"想"}}]}"#).as_deref(),
            Some("想")
        );
        assert_eq!(
            chat_reasoning(r#"{"choices":[{"delta":{"reasoning_content":"想"}}]}"#).as_deref(),
            Some("想")
        );
        // reasoning 是空字串：退到 reasoning_content
        assert_eq!(
            chat_reasoning(r#"{"choices":[{"delta":{"reasoning":"","reasoning_content":"想"}}]}"#)
                .as_deref(),
            Some("想")
        );
        // 兩欄同一段（相容複製）：只算一次
        assert_eq!(
            chat_reasoning(
                r#"{"choices":[{"delta":{"reasoning":"想","reasoning_content":"想"}}]}"#
            )
            .as_deref(),
            Some("想")
        );
        // 兩欄不同：reasoning 優先
        assert_eq!(
            chat_reasoning(
                r#"{"choices":[{"delta":{"reasoning":"甲","reasoning_content":"乙"}}]}"#
            )
            .as_deref(),
            Some("甲")
        );
        assert_eq!(
            chat_reasoning(r#"{"choices":[{"delta":{"content":"字"}}]}"#),
            None
        );
    }

    #[test]
    fn responses_reasoning_picks_only_reasoning_deltas() {
        assert_eq!(
            responses_reasoning(r#"{"type":"response.reasoning_text.delta","delta":" "}"#)
                .as_deref(),
            Some(" ")
        );
        assert_eq!(
            responses_reasoning(r#"{"type":"response.output_text.delta","delta":"字"}"#),
            None
        );
    }

    #[test]
    fn message_uses_this_guards_own_count() {
        let mut guard = RunawayGuard::thinking(RunawayPolicy::Full);
        let reason = guard.push(&" ".repeat(WHITESPACE_RUN)).unwrap();
        assert_eq!(
            guard.message(reason),
            "AI_OUTPUT_RUNAWAY: reason=whitespace_run chars=2000"
        );
    }

    #[test]
    fn message_carries_code_and_reason() {
        assert_eq!(
            runaway_message(RunawayReason::WhitespaceRun, 2010),
            "AI_OUTPUT_RUNAWAY: reason=whitespace_run chars=2010"
        );
    }
}
