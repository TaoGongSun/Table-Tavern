//! 傳輸層共用介面：上下文組裝→單發呼叫→串流回傳。
//! API 直連與（之後的）CLI 傳輸都必須經由 assemble_messages 取得上下文（KICKOFF §4）。
mod arrivals;
mod assemble;
mod client;
mod context;
mod messages;
mod response;
mod state_view;
mod turns;
#[cfg(test)]
mod test_support;

pub use arrivals::{PERSON_ARRIVAL_PREFIX, appeared_card_names, appeared_person_titles, card_arrival_text, detect_new_arrivals, detect_new_card_arrivals, person_arrival_text};
pub use assemble::{PLAYER_SENTINEL, assemble_gm_messages, assemble_shared_messages};
pub use client::{DEFAULT_BASE_URL, DEFAULT_IMAGE_MODEL, PromptCacheUsage, SseParser, StreamOutcome, TierModel, base_url, extract_delta, extract_usage, generate_image, gm_tier, refactor_expand_tier, resolve_model, stream_chat, tier_model, ui_language};
pub use context::{active_worldbook_entries};
pub use messages::{ChatMessage, resolve_display_macros};
pub use response::{StateBlock, card_format_instruction, extract_next_speaker, extract_scene_title, extract_state_block, narrate_instruction, parse_indented_fields, pick_speaker};
pub use state_view::{StateScope, character_state_block, resolve_branch, snapshot_updates, state_scope};
pub use turns::{LaneTurn, chars_lane_system, chars_lane_turn, gm_lane_system, gm_lane_turn, lane_event_line, summary_messages};
pub(crate) use client::{describe};
pub(crate) use messages::{player_fallback_name, replace_st_macros};
