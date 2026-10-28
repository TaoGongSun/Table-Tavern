//! 讀迴圈結束後的收尾判定（plans/cli-version-gate.md 包 1 第 2 點）。取消、失控、斷流在這之前就返回。
//! 優先序：收尾事件報的錯 > SoftError > CliCrashed（非 0 退出）> CliOutputUnrecognized（只在退出 0）
//! > CliNoReply。純函式，優先序可以直接單元測試。

use super::super::types::CliProtocol;

/// 判定要看的事實。
pub(super) struct Facts<'a> {
    pub protocol: &'a CliProtocol,
    /// 最後的收尾事件：(文字, 是否報錯)
    pub done: Option<&'a (String, bool)>,
    /// codex 頂層 error 的最後一則
    pub soft_error: Option<&'a str>,
    pub exit_ok: bool,
    /// 串流正文是否為空
    pub text_empty: bool,
    /// 這輪有沒有認不得的 stdout 行
    pub unrecognized: bool,
}

#[derive(Debug, PartialEq)]
pub(super) enum Verdict {
    /// 收尾事件報的錯（原文；容量、不完整碼由呼叫端分流）
    DoneError(String),
    /// 沒收尾、CLI 送過頂層 error：拿它當原因
    SoftError(String),
    /// 沒收尾、非 0 退出
    Crashed,
    /// 格式認不得（只在退出 0）
    Unrecognized,
    /// 沒收尾、退出 0、有正文：回合失敗
    TurnFailed,
    NoReply,
    /// 完成；`use_done_text`＝串流沒抓到增量，改用收尾文字
    Completed {
        use_done_text: bool,
    },
}

pub(super) fn decide(facts: &Facts<'_>) -> Verdict {
    match facts.done {
        Some((text, true)) => Verdict::DoneError(text.clone()),
        None if facts.protocol.require_done || !facts.exit_ok => {
            if let Some(error) = facts.soft_error {
                return Verdict::SoftError(error.to_owned());
            }
            if !facts.exit_ok {
                return Verdict::Crashed;
            }
            if facts.unrecognized {
                return Verdict::Unrecognized;
            }
            match facts.text_empty {
                true => Verdict::NoReply,
                false => Verdict::TurnFailed,
            }
        }
        done => {
            let done_text = done.map(|(text, _)| text.as_str()).unwrap_or_default();
            if !facts.text_empty {
                return Verdict::Completed {
                    use_done_text: false,
                };
            }
            if !done_text.is_empty() {
                return Verdict::Completed {
                    use_done_text: true,
                };
            }
            // 收尾到了、正文與收尾文字都空：正文事件可能改名了
            match facts.unrecognized && facts.exit_ok {
                true => Verdict::Unrecognized,
                false => Verdict::NoReply,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::stream::{CLAUDE_PROTOCOL, CODEX_PROTOCOL};
    use super::*;

    fn facts<'a>(done: Option<&'a (String, bool)>) -> Facts<'a> {
        Facts {
            protocol: &CODEX_PROTOCOL,
            done,
            soft_error: None,
            exit_ok: true,
            text_empty: false,
            unrecognized: false,
        }
    }

    #[test]
    fn finish_error_beats_everything() {
        let done = ("boom".to_owned(), true);
        let mut f = facts(Some(&done));
        f.soft_error = Some("retry");
        f.exit_ok = false;
        f.unrecognized = true;
        assert_eq!(decide(&f), Verdict::DoneError("boom".to_owned()));
    }

    #[test]
    fn soft_error_beats_crash_and_unrecognized_when_finish_is_missing() {
        let mut f = facts(None);
        f.soft_error = Some("context too long");
        f.exit_ok = false;
        f.unrecognized = true;
        assert_eq!(
            decide(&f),
            Verdict::SoftError("context too long".to_owned())
        );
        f.exit_ok = true;
        assert_eq!(
            decide(&f),
            Verdict::SoftError("context too long".to_owned())
        );
    }

    #[test]
    fn crash_beats_unrecognized() {
        let mut f = facts(None);
        f.exit_ok = false;
        f.unrecognized = true;
        assert_eq!(decide(&f), Verdict::Crashed);
    }

    #[test]
    fn missing_finish_with_clean_exit() {
        let mut f = facts(None);
        f.unrecognized = true;
        assert_eq!(decide(&f), Verdict::Unrecognized);
        f.unrecognized = false;
        assert_eq!(decide(&f), Verdict::TurnFailed);
        f.text_empty = true;
        assert_eq!(decide(&f), Verdict::NoReply);
    }

    #[test]
    fn retry_notice_then_finish_still_completes() {
        let done = (String::new(), false);
        let mut f = facts(Some(&done));
        f.soft_error = Some("Reconnecting 2/5");
        assert_eq!(
            decide(&f),
            Verdict::Completed {
                use_done_text: false
            }
        );
    }

    #[test]
    fn empty_finish_is_unrecognized_only_on_clean_exit() {
        let done = (String::new(), false);
        let mut f = facts(Some(&done));
        f.text_empty = true;
        f.unrecognized = true;
        assert_eq!(decide(&f), Verdict::Unrecognized);
        f.exit_ok = false;
        assert_eq!(decide(&f), Verdict::NoReply);
        let done = ("全文".to_owned(), false);
        let mut f = facts(Some(&done));
        f.text_empty = true;
        f.protocol = &CLAUDE_PROTOCOL;
        assert_eq!(
            decide(&f),
            Verdict::Completed {
                use_done_text: true
            }
        );
    }
}
