//! 傳輸層共用介面：上下文組裝→單發呼叫→串流回傳。
//! API 直連與（之後的）CLI 傳輸都必須經由 assemble_messages 取得上下文（KICKOFF §4）。
mod arrivals;
mod assemble;
mod client;
mod context;
pub(crate) mod dispatch;
mod messages;
mod response;
pub(crate) mod responses;
mod state_view;
#[cfg(test)]
mod test_support;
pub(crate) mod translate;
mod turns;

pub use arrivals::{
    card_arrival, card_private, detect_new_arrivals, detect_new_card_arrivals, person_arrival, Side,
};
pub use assemble::{assemble_gm_messages, assemble_shared_messages, PLAYER_SENTINEL};
#[cfg(feature = "test-harness")]
pub(crate) use client::DEFAULT_IMAGE_MODEL;
pub use client::{
    base_url, generate_image, gm_tier, refactor_expand_tier, resolve_model, stream_chat,
    stream_chat_models, tier_model, ui_language, PromptCacheUsage, SseParser, StreamChatResult,
    TierModel, DEFAULT_BASE_URL,
};
pub(crate) use client::{describe, http_error};
pub use context::{gm_prompt_full_entries, PromptEntry};
#[cfg(test)]
pub(crate) use messages::language_rule;
pub use messages::{history_header, resolve_display_macros, speaker_prefix, ChatMessage};
pub(crate) use messages::{player_fallback_name, replace_st_macros, scaffold_en};
pub use response::{
    card_format_instruction, card_format_turn, extract_next_speaker, extract_scene_title,
    extract_state_block, gm_closing, gm_turn_format, narrate_instruction, parse_indented_fields,
    pick_speaker, summary_closing, takeover_instruction, GmTurnFormat, StateBlock,
};
pub use state_view::{resolve_branch, snapshot_updates, state_scope, StateScope};
pub use turns::{
    chars_lane_system, chars_lane_turn, gm_lane_system, gm_lane_turn, lane_event_line,
    summary_messages,
};
