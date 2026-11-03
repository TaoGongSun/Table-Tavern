//! VN 卡匯入建桌（plans/vn-mode-pkg9.md §2.3）：拿到新 VN 桌的整桌獨占後一次建完——world.md、角色、世界書、
//! 開場、vn.json（卡原件）、聖經（走包 4 同一支套用）、主角、封面；任何一步失敗就刪掉整桌，不留半桌。
use super::remap::{remap_book, remap_holders, IdMap};
use super::schema::{self, Notice, VnCard};
use crate::data::vn::{self, CardOrigin, CastOrigin};
use crate::data::{self, CharacterCard, DataResult, Tier};
use crate::import::card::{display_png, parse_character};
use crate::import::card_io::PNG_MAGIC;
use serde_json::Value;
use std::path::Path;

#[derive(Debug, serde::Serialize)]
pub struct VnCardImported {
    pub world_id: String,
    pub notices: Vec<Notice>,
}

pub fn import_vn_card(root: &Path, bytes: &[u8], spoiler_lock: bool) -> DataResult<VnCardImported> {
    import_with(root, bytes, spoiler_lock, &mut |_| Ok(()))
}

/// 測試掛點：每做完一步呼叫一次（步驟名），回錯＝模擬之後的步驟失敗。
pub(super) fn import_with(
    root: &Path,
    bytes: &[u8],
    spoiler_lock: bool,
    hook: &mut dyn FnMut(&str) -> DataResult<()>,
) -> DataResult<VnCardImported> {
    let parsed = parse_character(bytes)?;
    let card_data = parsed
        .value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(&parsed.value);
    let vn = schema::vn_value(card_data).ok_or_else(|| data::invalid_data(schema::UNSUPPORTED))?;
    let mut notices = Vec::new();
    let card = schema::parse(vn, &mut notices)?;
    let title = card.title.clone().unwrap_or_else(|| parsed.name.clone());
    let (world_id, held) = data::create_world_exclusive(root, &title, data::TableKind::Vn)
        .map_err(|failed| failed.error)?;
    let built = build(
        root,
        &world_id,
        &card,
        card_data.get("character_book"),
        bytes,
        spoiler_lock,
        &mut notices,
        hook,
    );
    match built {
        Ok(()) => Ok(VnCardImported { world_id, notices }),
        Err(error) => {
            if let Err(cleanup) = data::discard_new_world(root, &world_id, &held) {
                log::warn!("VN 卡匯入失敗後收不掉新桌 {world_id}：{cleanup}");
            }
            Err(error)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build(
    root: &Path,
    world_id: &str,
    card: &VnCard,
    book: Option<&Value>,
    bytes: &[u8],
    spoiler_lock: bool,
    notices: &mut Vec<Notice>,
    hook: &mut dyn FnMut(&str) -> DataResult<()>,
) -> DataResult<()> {
    data::write_world_md(root, world_id, &card.world)?;
    let ids: IdMap = card
        .cast
        .iter()
        .map(|member| (member.id.clone(), data::new_id()))
        .collect();
    for member in &card.cast {
        data::write_character(
            root,
            world_id,
            &CharacterCard {
                id: ids[&member.id].clone(),
                name: member.name.clone(),
                color: member.color.clone(),
                avatar: member.emoji.clone(),
                tier: Tier::Balanced,
                show_image: true,
                archived: false,
                gen_prompt: String::new(),
                public_md: member.public.clone(),
                private_md: member.private.clone(),
                stage: false,
            },
        )?;
    }
    hook("characters")?;
    // 世界書照 GM 路收編（不建「作品名」那個角色）：可見角色換成新 id，對不到的剔除
    if let Some(mut book) = book.filter(|book| book.get("entries").is_some()).cloned() {
        remap_book(&mut book, &ids, notices);
        data::import_worldbook_as(
            root,
            world_id,
            &serde_json::to_string(&book)?,
            &data::BookOwner::Gm,
        )?;
    }
    hook("worldbook")?;
    if !card.opening.trim().is_empty() {
        data::append_transcript(
            root,
            world_id,
            0,
            &crate::genesis::opening_event(&card.opening),
        )?;
    }
    let cast: Vec<(String, String)> = card
        .cast
        .iter()
        .map(|member| (ids[&member.id].clone(), member.name.clone()))
        .collect();
    let bible = card.bible.clone().map(|mut bible| {
        remap_holders(&mut bible, &ids, notices);
        vn::normalize(bible, &cast)
    });
    let origin = CardOrigin {
        lang: card.lang.clone(),
        rating: card.rating.clone(),
        pov: card.pov,
        protagonist_fixed: card.protagonist.is_some(),
        original_bible: bible.clone(),
        cast: card
            .cast
            .iter()
            .map(|member| {
                let origin = CastOrigin {
                    role: member.role,
                    appears: member.appears.clone(),
                    art: member.art.clone(),
                };
                (ids[&member.id].clone(), origin)
            })
            .collect(),
        art: card.art.clone(),
        locations: card.locations.clone(),
    };
    data::state_commit::with_commit(root, world_id, |tx| {
        vn::edit_vn(tx, |file| {
            file.spoiler_lock = spoiler_lock;
            file.card = Some(origin);
        })
    })?;
    hook("vn")?;
    if let Some(bible) = bible {
        crate::vn_bible::install(root, world_id, &cast, None, |_| Ok(bible))
            .map_err(data::invalid_data)?;
    }
    hook("bible")?;
    // 主角借玩家卡槽位：放在世界書收編之後，條目的主角可見度才對得到本桌角色
    if let Some(id) = &card.protagonist {
        data::set_player_card(root, world_id, Some(ids[id].clone()))?;
    }
    if bytes.starts_with(PNG_MAGIC) {
        if let Some(cover) = display_png(bytes) {
            data::commit_world_write_atomic(&vn::cover_path(root, world_id)?, &cover)?;
        }
    }
    hook("cover")
}
