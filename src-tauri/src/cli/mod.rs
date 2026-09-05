//! CLI 傳輸層（訂閱模式，NewPlan §3.2／§4.2）。
//! 原則：只偵測不代辦；CLI 是無狀態傳輸——上下文一律由 transport::assemble_messages
//! 組裝、headless 單發、system prompt 覆寫，不依賴 CLI 自身 session（§8.1）。
//! 旗標依當場原始碼／--help 查證：claude 2.1.210、codex-cli 0.145.0、agy 1.1.17、grok 1.0.5。

mod catalog;
mod detect;
mod request;
mod runner;
mod stream;
mod types;

pub use catalog::cli_model_catalog;
pub use detect::detect_clis;
// 唯一 crate 內呼叫者 commands::cli_setup 的安裝路徑只在 Windows 編譯
#[cfg(target_os = "windows")]
pub(crate) use detect::find_binary;
pub use request::{
    agy_args, agy_supports_stream_json, claude_args, claude_model_for,
    claude_session_args, codex_args, codex_effort_for, flatten_messages, grok_args,
    grok_envs, grok_session_args, tier_override,
};
pub use runner::run_cli;
pub use stream::{
    parse_agy_line, parse_agy_usage, parse_claude_line, parse_claude_usage,
    parse_codex_line, parse_codex_usage, parse_grok_line, parse_grok_usage,
};
pub use types::{CliInfo, CliSession, ModelOption, UsageLog};
