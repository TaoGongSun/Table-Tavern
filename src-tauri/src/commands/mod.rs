pub(crate) mod character;
pub(crate) mod chat;
pub(crate) mod cli_setup;
pub(crate) mod genesis;
pub(crate) mod image;
pub(crate) mod refactor;
pub(crate) mod scene;
pub(crate) mod settings;
pub(crate) mod state;
pub(crate) mod world;

// 三個以上子模組的測試共用這兩樣，放在模組根才不必各檔複製一份。
#[cfg(test)]
use crate::data;
#[cfg(test)]
use std::sync::atomic::AtomicU64;

#[cfg(test)]
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
fn character_card(id: &str, name: &str) -> data::CharacterCard {
    data::CharacterCard {
        id: id.to_owned(),
        name: name.to_owned(),
        color: "#336699".to_owned(),
        avatar: "🦊".to_owned(),
        tier: data::Tier::Balanced,
        show_image: true,
        archived: false,
        gen_prompt: String::new(),
        public_md: String::new(),
        private_md: String::new(),
    }
}
