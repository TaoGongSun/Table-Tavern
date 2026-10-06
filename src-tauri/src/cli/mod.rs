//! CLI 傳輸層（訂閱模式，NewPlan §3.2／§4.2）。
//! 原則：只偵測不代辦；CLI 是無狀態傳輸——上下文一律由 transport::assemble_messages
//! 組裝、headless 單發、system prompt 覆寫，不依賴 CLI 自身 session（§8.1）。
//! 旗標依當場原始碼／--help 查證：claude 2.1.210、codex-cli 0.145.0、agy 1.1.17、grok 1.0.5；
//! 提示詞改走檔案／stdin 的旗標於 claude 2.1.287、agy 1.2.16、grok 1.0.46 實測（long-prompt-scene-hint）。

mod catalog;
mod detect;
#[allow(dead_code)]
pub(crate) mod install;
mod prompt_file;
mod proxy;
mod request;
mod runner;
mod stream;
mod types;

pub use catalog::cli_model_catalog;
pub use detect::detect_clis;
// 唯一 crate 內呼叫者 commands::cli_setup 的安裝路徑只在 Windows 編譯
#[cfg(target_os = "windows")]
pub(crate) use detect::find_binary;
pub use prompt_file::PromptFile;
pub use request::{
    agy_args, agy_body, agy_session_args, agy_session_body, agy_supports_stream_json, claude_args,
    claude_model_for, claude_session_args, codex_args, codex_effort_for, flatten_messages,
    grok_args, grok_envs, grok_payload, grok_session_args, tier_override,
};
pub use runner::{run_cli, run_cli_cancellable, CliFinish};
pub use stream::{
    parse_agy_line, parse_agy_usage, parse_claude_line, parse_claude_usage, parse_codex_line,
    parse_codex_usage, parse_grok_line, parse_grok_usage,
};
pub use types::{AgyUsageCounters, CliInfo, CliSession, ModelOption, UsageLog};
