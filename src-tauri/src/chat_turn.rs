//! 聊天回合的業務流程（角色接話、GM 旁白、提交與登場紀錄）。`commands::chat` 只做 Tauri 邊界、轉呼叫這裡；
//! 送什麼由 `chat_assembly` 組裝。
mod arrivals;
mod character;
mod commit;
mod gm;
mod gm_reply;
// 包 2（房間執行端）接上前只有測試在用
#[cfg_attr(not(test), allow(dead_code))]
mod party_batch;
mod stream;

pub(crate) use character::{chat_with_character, ChatReply};
#[cfg(test)]
pub(crate) use commit::gm_commit_apply;
pub(crate) use gm::{gm_narrate, GmNarration};
#[allow(unused_imports)]
pub(crate) use party_batch::{
    append_party_batch, BatchOutcome, DepartureMode, PartyAction, PartyBatch,
};
