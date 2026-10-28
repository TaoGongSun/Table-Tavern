//! 「需要更新 CLI」的錯誤辨識（plans/cli-version-gate.md 包 2 第 2 點）：模型叫不到與 CLI 太舊，
//! 各自掛穩定碼，前端據此接更新鈕。claude、codex 認結構化欄位，agy、grok 認文字（四家都有夾具測試）。

use crate::ui_msg::UiMsg;

/// CLI 不認得 app 用的旗標時的 stderr 字樣（方案「前提」：claude `unknown option`；codex、grok
/// `unexpected argument`；agy `flags provided but not defined`）。
const UNKNOWN_FLAG: [&str; 3] = [
    "error: unknown option '",
    "error: unexpected argument '",
    "flags provided but not defined:",
];

/// claude 伺服器要求升級（官方錯誤文件）：`Claude Code <ver> does not support this model; version <req> or newer is required`。
fn claude_required(text: &str) -> Option<String> {
    let rest = text.split("does not support this model; version ").nth(1)?;
    let required = rest.split(" or newer is required").next()?.trim();
    (!required.is_empty() && required.len() <= 32).then(|| required.to_owned())
}

/// codex `turn.failed.error.message` 是內嵌 JSON：`error.code == "model_not_found"`。
fn codex_model_not_found(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/code")
                .and_then(|code| code.as_str())
                .map(|code| code == "model_not_found")
        })
        .unwrap_or(false)
}

/// 失敗收尾（或 SoftError）的原文、整段 stderr、claude 的結構化旗標 → 需要更新 CLI 就回對應訊息。
/// `version` 是 `--version` 原文（查不到給空字串）。
pub(in crate::cli) fn needs_update(
    cli: &str,
    text: &str,
    stderr: &str,
    claude_model_not_found: bool,
    model: Option<&str>,
    version: &str,
) -> Option<UiMsg> {
    if let Some(required) = (cli == "claude").then(|| claude_required(text)).flatten() {
        return Some(UiMsg::CliTooOldRequired {
            cli: cli.to_owned(),
            version: version.to_owned(),
            required,
        });
    }
    if let Some(msg) = too_old_flag(cli, stderr, version) {
        return Some(msg);
    }
    let unavailable = match cli {
        "claude" => claude_model_not_found,
        "codex" => codex_model_not_found(text),
        "agy" => {
            text.contains("is not recognized as a known model")
                || stderr.contains("is not recognized as a known model")
        }
        "grok" => text.contains("\"unknown model id\"") || stderr.contains("\"unknown model id\""),
        _ => false,
    };
    unavailable.then(|| UiMsg::CliModelUnavailable {
        cli: cli.to_owned(),
        model: model.unwrap_or_default().to_owned(),
    })
}

/// stderr 出現「不認得旗標」：CLI 比 app 用到的旗標舊。
pub(in crate::cli) fn too_old_flag(cli: &str, stderr: &str, version: &str) -> Option<UiMsg> {
    UNKNOWN_FLAG
        .iter()
        .any(|needle| stderr.contains(needle))
        .then(|| UiMsg::CliTooOld {
            cli: cli.to_owned(),
            version: version.to_owned(),
        })
}

/// 參數裡的模型 id（`--model <id>`／`-m <id>`）。
pub(in crate::cli) fn model_arg(args: &[String]) -> Option<&str> {
    args.windows(2)
        .find(|pair| pair[0] == "--model" || pair[0] == "-m")
        .map(|pair| pair[1].as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_server_upgrade_message_carries_the_required_version() {
        let text = "API Error: Claude Code 2.1.200 does not support this model; version 2.1.250 or newer is required";
        assert_eq!(
            needs_update(
                "claude",
                text,
                "",
                false,
                Some("opus"),
                "2.1.200 (Claude Code)"
            ),
            Some(UiMsg::CliTooOldRequired {
                cli: "claude".to_owned(),
                version: "2.1.200 (Claude Code)".to_owned(),
                required: "2.1.250".to_owned(),
            })
        );
    }

    #[test]
    fn unknown_flags_mean_the_cli_is_too_old() {
        for (cli, stderr) in [
            ("claude", "error: unknown option '--safe-mode'"),
            (
                "codex",
                "error: unexpected argument '--ignore-user-config' found",
            ),
            ("grok", "error: unexpected argument '--prompt-file' found"),
            ("agy", "flags provided but not defined: -agent"),
        ] {
            assert!(
                matches!(
                    needs_update(cli, "", stderr, false, None, ""),
                    Some(UiMsg::CliTooOld { .. })
                ),
                "{cli}"
            );
        }
    }

    #[test]
    fn unrelated_failures_are_not_classified() {
        for cli in ["claude", "codex", "agy", "grok"] {
            assert_eq!(
                needs_update(cli, "rate limit exceeded", "boom", false, None, ""),
                None
            );
        }
    }

    #[test]
    fn model_arg_reads_both_spellings() {
        let args: Vec<String> = ["-p", "--model", "haiku"].map(str::to_owned).to_vec();
        assert_eq!(model_arg(&args), Some("haiku"));
        let args: Vec<String> = ["-m", "grok-x", "-p"].map(str::to_owned).to_vec();
        assert_eq!(model_arg(&args), Some("grok-x"));
        assert_eq!(model_arg(&[]), None);
    }
}
