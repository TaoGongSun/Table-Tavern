//! 聊天回合的業務流程（角色接話、GM 旁白、提交與登場紀錄）。`commands::chat` 只做 Tauri 邊界、轉呼叫這裡；
//! 送什麼由 `chat_assembly` 組裝。
mod arrivals;
mod character;
mod commit;
mod gm;
mod gm_reply;
mod party_batch;
pub(crate) mod relay;
mod solo_relay;
mod stream;
mod vn_ending;
mod vn_reroll;
mod vn_script;

pub(crate) use character::chat_with_character;
#[cfg(test)]
pub(crate) use commit::gm_commit_apply;
pub(crate) use gm::{gm_narrate, scene_panel, scene_status};
pub(crate) use party_batch::{
    append_party_batch, BatchOutcome, DepartureMode, PartyAction, PartyBatch,
};
pub(crate) use solo_relay::{
    active as solo_relay_active, chat_relay, interrupt as interrupt_relay,
    release as release_relay, RelayReport,
};
pub(crate) use vn_reroll::reroll_choices;
pub(crate) use vn_script::{
    beats, finish_vn_character_reply, parse_text, read_scene, VnCell, VnEventCells, VnRoster,
    VnSource, VnText,
};
