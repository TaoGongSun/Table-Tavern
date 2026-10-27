mod export;
mod glyph;
mod import;
mod lifecycle;
mod marker;
mod presence;
mod transcript;
mod visibility;

pub use export::{export_scene_markdown, export_transcript_markdown};
pub(crate) use glyph::{fold_glyphs, glyph_equal};
pub use import::{write_imported_scene, ImportedEvent, ImportedScene};
pub use lifecycle::{
    begin_next_scene, fork_scene, replace_scene_summary, revert_scene, scene_label,
};
pub use marker::{
    appeared_card_names, appeared_person_titles, event_full_text, marker_heading, prompt_lang,
    EventMarker,
};
pub(crate) use marker::{lang_key, player_fallback_name};
pub(crate) use presence::{match_present, match_present_with, name_matches, split_present_names};
pub use transcript::{
    append_character_reply, append_event, append_opening, append_opening_keyed, append_transcript,
    append_within_turn, discard_unanswered_player, opening_checkpoint, pop_transcript,
    read_transcript, remove_transcript_event, set_last_transcript_state, settle_pending_turn,
    sync_scene_state_tree, TranscriptEvent, TranscriptKind,
};
pub(crate) use transcript::{edit_line, find_event_rev, find_rev, transcript_path, LineHead};
