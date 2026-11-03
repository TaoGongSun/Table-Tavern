//! 傳輸層共用介面：上下文組裝→單發呼叫→串流回傳。
//! API 直連與（之後的）CLI 傳輸都必須經由 assemble_messages 取得上下文（KICKOFF §4）。
mod api_failure;
mod arrivals;
mod assemble;
mod client;
mod context;
pub(crate) mod context_overflow;
pub(crate) mod dispatch;
mod field_notes;
mod gm_opening;
mod list_protocol;
mod messages;
mod own_prefix;
mod prompt_regex;
mod reply_cleanup;
mod response;
pub(crate) mod responses;
mod runaway;
pub(crate) mod siliconflow;
mod stall;
mod state_view;
mod state_yaml;
pub use state_yaml::parse_initvar_tree;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod translate;
mod turns;
pub(crate) mod vn_background_prompts;
pub(crate) mod vn_bible_prompts;
mod vn_format;
pub(crate) mod vn_sprite_prompts;
mod worldbook;

#[cfg(test)]
pub use api_failure::RateLimit;
pub use api_failure::{ApiFailure, ErrorDetail, FailureStage};
pub use arrivals::{
    card_arrival, card_private, detect_new_arrivals, detect_new_card_arrivals, person_arrival,
    NeutralFill, Side,
};
pub(crate) use arrivals::{prompt_speaker, prompt_text};
#[cfg(test)]
pub use assemble::assemble_shared_messages;
pub use assemble::{assemble_gm_messages_with, assemble_shared_messages_with, PLAYER_SENTINEL};
pub(crate) use client::key_tier_for;
pub(crate) use client::openrouter_api_base;
#[cfg(feature = "test-harness")]
pub(crate) use client::set_openrouter_origin;
#[cfg(feature = "test-harness")]
pub(crate) use client::DEFAULT_IMAGE_MODEL;
pub use client::{
    base_url, generate_image, generate_image_with, gm_tier, refactor_expand_tier, resolve_model,
    stream_chat, stream_chat_models, tier_model, ui_language, PromptCacheUsage, SmartChatResult,
    SseParser, StreamChatResult, TierModel, DEFAULT_BASE_URL,
};
pub(crate) use client::{describe, http_error};
pub use context::{gm_prompt_full_entries, GmVoice, PromptEntry};
pub use gm_opening::GmOpening;
#[cfg(test)]
pub(crate) use messages::language_rule;
pub use messages::{history_header, speaker_prefix, ChatMessage};
pub(crate) use messages::{player_fallback_name, replace_st_macros, scaffold_en};
pub use own_prefix::OwnPrefixStream;
pub use prompt_regex::{depths as prompt_depths, PromptRegex};
pub use reply_cleanup::{
    cut_unclosed_tail, empty_reply_error, final_reply_text, finish_character_reply,
    strip_narrator_label, strip_self_closing_controls, strip_state_echo, CharacterReply,
};
#[cfg(test)]
pub use response::card_format_instruction;
pub use response::{
    card_format_turn, extract_next_speaker, extract_scene_title, extract_state_block, gm_closing,
    gm_turn_format, narrate_instruction, narration_closing_with_updates, narration_update_note,
    parse_indented_fields, party_instruction, pick_speaker, summary_closing, takeover_instruction,
    GmTurnFormat, StateBlock,
};
pub(crate) use runaway::{runaway_message, LINE_CAP_BYTES, RUNAWAY_CODE};
pub use runaway::{RunawayGuard, RunawayPolicy, RunawayReason};
pub(crate) use stall::STALLED_CODE;
pub use state_view::{resolve_branch, snapshot_updates, state_scope, StateScope};
#[cfg(test)]
pub use turns::chars_lane_turn;
pub use turns::{
    chars_lane_system, chars_lane_turn_with, gm_lane_system_with, gm_lane_turn, lane_event_line,
    lane_line, merge_summary_messages, segment_summary_messages, shorten_summary_messages,
    summary_lines, summary_messages, system_worldbook, Hoist, LaneTurn, SEGMENT_SUMMARY_CHARS,
};
pub use vn_format::{
    expression_note, vn_card_note, vn_character_rules, vn_closing, vn_format_rules,
    vn_narrate_instruction, vn_reroll_closing, vn_reroll_instruction, vn_variant_note, ChapterCue,
    VnDirection, VN_EXPRESSIONS,
};
