//! 各家 CLI 一輪 lane 呼叫的參數、stdin 正文、輸出協定與用量帳本設定（自 lanes/mod.rs 搬出）。

use super::*;

/// 一輪呼叫要交給 runner 的參數與 stdin；`files` 是提示詞暫存檔，活到 run_cli 收屍返回後才刪。
pub(super) struct ProviderCommand {
    pub args: Vec<String>,
    pub stdin: String,
    pub files: Vec<cli::PromptFile>,
}

/// system／正文一律不進命令列（Windows 整條命令列只有 32,767 個 UTF-16 單位）：
/// Claude 的 system 寫檔、正文走 stdin；Grok 開線的 system 寫成 agent profile、正文寫檔；
/// Agy 與 codex 整包走 stdin（開線帶 system、續聊只送本輪）。
pub(super) fn provider_command(
    call: &LaneCall,
    opening: bool,
    session_id: &str,
    system: &str,
    prompt: &str,
) -> Result<ProviderCommand, String> {
    let session = if opening {
        cli::CliSession::Open(session_id)
    } else {
        cli::CliSession::Resume(session_id)
    };
    let mut files = Vec::new();
    let (args, stdin) = match call.provider {
        LaneProvider::Claude => {
            let system_file = cli::PromptFile::create(&call.prompt_dir, "system.txt", system)?;
            let args = cli::claude_session_args(call.model_label(), system_file.path(), &session);
            files.push(system_file);
            (args, prompt.to_owned())
        }
        LaneProvider::Grok => {
            let profile = match (&session, system.is_empty()) {
                (cli::CliSession::Open(_), false) => Some(cli::PromptFile::create(
                    &call.prompt_dir,
                    "grok-system.md",
                    &cli::grok_payload(system, "", false).0.unwrap_or_default(),
                )?),
                _ => None,
            };
            let body = cli::PromptFile::create(&call.prompt_dir, "prompt.txt", prompt)?;
            let args = cli::grok_session_args(
                call.model.as_deref(),
                profile.as_ref().map(cli::PromptFile::path),
                body.path(),
                &session,
            );
            files.extend(profile);
            files.push(body);
            (args, String::new())
        }
        LaneProvider::Agy => {
            let conversation = (!opening).then_some(session_id);
            (
                cli::agy_session_args(call.model.as_deref(), conversation),
                cli::agy_session_body(system, prompt, conversation),
            )
        }
        LaneProvider::Codex => {
            let thread = (!opening).then_some(session_id);
            (
                cli::codex_session_args(
                    call.model.as_deref(),
                    call.effort.as_deref().unwrap_or("medium"),
                    thread,
                ),
                cli::agy_session_body(system, prompt, thread),
            )
        }
    };
    Ok(ProviderCommand { args, stdin, files })
}

pub(super) fn protocol(provider: LaneProvider) -> &'static cli::CliProtocol {
    match provider {
        LaneProvider::Claude => &cli::CLAUDE_PROTOCOL,
        LaneProvider::Agy => &cli::AGY_PROTOCOL,
        LaneProvider::Grok => &cli::GROK_PROTOCOL,
        LaneProvider::Codex => &cli::CODEX_PROTOCOL,
    }
}

/// 一次嘗試的觀測槽：降級重開不沿用失敗那次的用量、超額旗標與 id。
#[derive(Default)]
pub(super) struct AttemptSlots {
    pub usage: std::sync::Mutex<Option<transport::PromptCacheUsage>>,
    pub overage: std::sync::atomic::AtomicBool,
    pub identity: std::sync::Mutex<Option<String>>,
    pub conversation_id: std::sync::Mutex<Option<String>>,
    pub agy_usage: std::sync::Mutex<Option<cli::AgyUsageCounters>>,
    pub codex_thread: std::sync::Mutex<Option<String>>,
    pub codex_usage: std::sync::Mutex<Option<cli::CodexUsageCounters>>,
    pub unclean_exit: std::sync::atomic::AtomicBool,
}

/// 這輪的用量帳本設定（帳本關著就 None）。
#[allow(clippy::too_many_arguments)]
pub(super) fn usage_log<'a>(
    call: &'a LaneCall,
    world_id: &'a str,
    lane_log: usage_log::LaneContext,
    opening: bool,
    session_id: &'a str,
    prior_agy_usage: Option<cli::AgyUsageCounters>,
    prompt_tokens: &'a std::sync::atomic::AtomicU64,
    slots: &'a AttemptSlots,
) -> Option<cli::UsageLog<'a>> {
    let agy = call.provider == LaneProvider::Agy;
    call.usage_log.as_deref().map(|path| cli::UsageLog {
        usage_out: Some(&slots.usage),
        overage_out: (call.provider == LaneProvider::Claude).then_some(&slots.overage),
        path,
        world: Some(world_id),
        transport: call.provider.as_str(),
        model: call.model_label(),
        parse: match call.provider {
            LaneProvider::Claude => cli::parse_claude_usage,
            LaneProvider::Agy => cli::parse_agy_usage,
            LaneProvider::Grok => cli::parse_grok_usage,
            LaneProvider::Codex => cli::parse_codex_usage,
        },
        lane: Some(lane_log),
        shape: usage_log::PromptShape::Oneshot, // 續聊線的形狀由 LaneContext 說明，這欄不參與判定
        prompt_tokens_out: Some(prompt_tokens),
        conversation_id_out: agy.then_some(&slots.conversation_id),
        expected_conversation_id: (agy && !opening).then_some(session_id),
        agy_usage_base: (agy && !opening).then_some(prior_agy_usage).flatten(),
        agy_usage_out: agy.then_some(&slots.agy_usage),
        identity_out: Some(&slots.identity),
    })
}

/// codex 的 thread id 與累計用量觀測：不綁在帳本上，帳本關著也照樣擷取（下一輪 resume 要用）。
pub(super) fn session_watch<'a>(
    call: &LaneCall,
    opening: bool,
    session_id: &'a str,
    prior_codex_usage: Option<cli::CodexUsageCounters>,
    slots: &'a AttemptSlots,
) -> cli::SessionWatch<'a> {
    if call.provider != LaneProvider::Codex {
        return cli::SessionWatch::default();
    }
    cli::SessionWatch {
        id_out: Some(&slots.codex_thread),
        expected_id: (!opening).then_some(session_id),
        codex_usage_base: (!opening).then_some(prior_codex_usage).flatten(),
        resumed: !opening,
        codex_usage_out: Some(&slots.codex_usage),
        unclean_exit_out: Some(&slots.unclean_exit),
    }
}
