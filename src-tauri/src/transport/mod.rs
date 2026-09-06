//! 傳輸層共用介面：上下文組裝→單發呼叫→串流回傳。
//! API 直連與（之後的）CLI 傳輸都必須經由 assemble_messages 取得上下文（KICKOFF §4）。
mod arrivals;
mod assemble;
mod client;
mod context;
mod messages;
mod response;
mod state_view;
#[cfg(test)]
mod test_support;
mod turns;

pub use arrivals::{
    appeared_card_names, appeared_person_titles, card_arrival_text, detect_new_arrivals,
    detect_new_card_arrivals, person_arrival_text,
};
pub use assemble::{assemble_gm_messages, assemble_shared_messages, PLAYER_SENTINEL};
pub(crate) use client::describe;
pub use client::{
    base_url, generate_image, gm_tier, refactor_expand_tier, resolve_model, stream_chat,
    tier_model, ui_language, PromptCacheUsage, SseParser, TierModel, DEFAULT_BASE_URL,
};
pub(crate) use messages::{player_fallback_name, replace_st_macros};
pub use messages::{resolve_display_macros, ChatMessage};
pub use response::{
    card_format_instruction, extract_next_speaker, extract_scene_title, extract_state_block,
    narrate_instruction, parse_indented_fields, pick_speaker, StateBlock,
};
pub use state_view::{resolve_branch, snapshot_updates, state_scope, StateScope};
pub use turns::{
    chars_lane_system, chars_lane_turn, gm_lane_system, gm_lane_turn, lane_event_line,
    summary_messages,
};
