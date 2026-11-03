//! VN 桌匯出成 VN 卡（plans/vn-mode-pkg9.md §2.7）：從 vn.json、角色、世界書、開場反向組卡。角色 id 重編成
//! `c1…cN`，秘密持有者、世界書可見角色、主角與逐角色欄位一起換；美術欄位從型別寫回卡上原名。ST 標準欄位照 §2.1 填。
use super::remap::{remap_book, remap_holders, IdMap};
use super::schema::{FORMAT, VERSION};
use crate::data::vn::{self, CardOrigin, Pov};
use crate::data::vn_art::{ArtEntry, CastArt};
use crate::data::{self, CharacterCard, DataResult};
use crate::import::card::display_png;
use crate::import::card_io::blank_png;
use crate::import::export::{embed_chara_chunk, v2_entry};
use crate::ui_msg::UiMsg;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

/// 卡內角色上限（與匯入同）
const CAST_MAX: usize = 8;

/// `cover`：桌上沒有封面時玩家挑的 PNG（None＝不用，給透明底圖）。副檔名 .json 寫 JSON，其餘寫 PNG。
pub fn export_vn_card(
    root: &Path,
    world_id: &str,
    path: &Path,
    cover: Option<&Path>,
    lang: &str,
) -> DataResult<()> {
    data::refuse_if_updating()?;
    let card = vn_card_json(root, world_id, lang)?;
    let json = serde_json::to_vec_pretty(&card)?;
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        fs::write(path, json)?;
        return Ok(());
    }
    let base = match vn::cover_path(root, world_id)? {
        stored if stored.is_file() => fs::read(stored)?,
        _ => match cover {
            Some(picked) => {
                display_png(&fs::read(picked)?).ok_or_else(|| UiMsg::ImageNotPng.into_error())?
            }
            None => blank_png(),
        },
    };
    fs::write(path, embed_chara_chunk(&base, &json)?)?;
    Ok(())
}

/// `lang`：卡原件沒記語系（AI 生成的 VN 桌）時填的介面語系。
pub(super) fn vn_card_json(root: &Path, world_id: &str, lang: &str) -> DataResult<Value> {
    let state = data::read_state(root, world_id)?;
    let file = vn::read_vn(root, world_id)?;
    let mut origin = file.card.clone().unwrap_or_default();
    if origin.lang.is_empty() {
        origin.lang = lang.to_owned();
    }
    let protagonist = data::read_player_card(root, world_id)?.filter(|_| origin.protagonist_fixed);
    let mut cast: Vec<CharacterCard> = protagonist.clone().into_iter().collect();
    for meta in data::list_characters(root, world_id)? {
        if matches!(
            meta.cast_role(),
            data::CastRole::Speaker | data::CastRole::Hidden
        ) {
            cast.push(data::read_character(root, world_id, &meta.id)?);
        }
    }
    cast.truncate(CAST_MAX);
    let ids: IdMap = cast
        .iter()
        .enumerate()
        .map(|(index, card)| (card.id.clone(), format!("c{}", index + 1)))
        .collect();
    let mut ignored = Vec::new();
    let bible = file.bible.map(|mut bible| {
        remap_holders(&mut bible, &ids, &mut ignored);
        bible
    });
    let world = data::read_world_md(root, world_id)?;
    let opening = data::read_transcript(root, world_id, 0)?
        .into_iter()
        .find(|event| event.opening)
        .map(|event| event.text)
        .unwrap_or_default();
    let mut vn_value = json!({
        "format": FORMAT,
        "version": VERSION,
        "lang": origin.lang,
        "rating": origin.rating,
        "protagonist": match &protagonist {
            Some(card) => json!({ "mode": "fixed", "id": ids[&card.id] }),
            None => json!({ "mode": "player" }),
        },
        "pov": origin.pov.unwrap_or(Pov::Third),
        "world": world,
        "opening": opening,
        "cast": cast.iter().map(|card| cast_json(card, &ids, &origin)).collect::<Vec<_>>(),
        "bible": bible,
    });
    if let Some(object) = vn_value.as_object_mut() {
        if let Some(art) = &origin.art {
            object.insert("art".to_owned(), json!(art));
        }
        if !origin.locations.is_empty() {
            // 卡上叫 `slug`
            let locations = origin.locations.iter().map(|location| {
                let mut value = json!(location);
                if let Some(object) = value.as_object_mut() {
                    if let Some(key) = object.remove("key") {
                        object.insert("slug".to_owned(), key);
                    }
                }
                value
            });
            object.insert("locations".to_owned(), Value::Array(locations.collect()));
        }
    }
    let mut book =
        json!({ "name": state.name, "entries": book_entries(root, world_id)?, "extensions": {} });
    remap_book(&mut book, &ids, &mut ignored);
    let description = std::iter::once(world.trim().to_owned())
        .chain(
            cast.iter()
                .map(|card| format!("{}: {}", card.name, card.public_md.trim())),
        )
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    let data = json!({
        "name": state.name,
        "description": description,
        "personality": "",
        "scenario": "",
        "first_mes": prose_opening(&opening),
        "mes_example": "",
        "creator_notes": creator_notes(&origin),
        "system_prompt": "",
        "post_history_instructions": "",
        "alternate_greetings": [],
        "tags": [],
        "creator": "",
        "character_version": "",
        "character_book": book,
        "extensions": { "table_tavern": { "vn": vn_value } },
    });
    Ok(json!({
        "spec": "chara_card_v2",
        "spec_version": "2.0",
        "name": state.name,
        "data": data,
    }))
}

fn cast_json(card: &CharacterCard, ids: &IdMap, origin: &CardOrigin) -> Value {
    let mut member = Map::new();
    member.insert("id".to_owned(), json!(ids[&card.id]));
    member.insert("name".to_owned(), json!(card.name));
    member.insert("color".to_owned(), json!(card.color));
    member.insert("emoji".to_owned(), json!(card.avatar));
    member.insert("public".to_owned(), json!(card.public_md));
    member.insert("private".to_owned(), json!(card.private_md));
    if let Some(cast) = origin.cast.get(&card.id) {
        if let Some(role) = cast.role {
            member.insert("role".to_owned(), json!(role));
        }
        if !cast.appears.is_empty() {
            member.insert("appears".to_owned(), json!(cast.appears));
        }
        cast_art_json(&cast.art, &mut member);
    }
    Value::Object(member)
}

/// 逐角色美術欄位寫回卡上原名（`expressions` → `expressions_extra`）；沒有的不寫
fn cast_art_json(art: &CastArt, member: &mut Map<String, Value>) {
    let entries = |list: &[ArtEntry]| json!(list);
    if let Some(appearance) = &art.appearance {
        member.insert("appearance".to_owned(), json!(appearance));
    }
    if !art.expressions.is_empty() {
        member.insert("expressions_extra".to_owned(), entries(&art.expressions));
    }
    if !art.variants.is_empty() {
        member.insert("variants".to_owned(), entries(&art.variants));
    }
    if let Some(height) = art.height_cm {
        member.insert("height_cm".to_owned(), json!(height));
    }
}

/// 這桌世界書全部條目（GM 條目也帶：卡本來就是明文，匯回來才不遺失），轉回 V2 欄位並保留角色可見度
/// （之後由 `remap_book` 換成卡內 id）。
fn book_entries(root: &Path, world_id: &str) -> DataResult<Vec<Value>> {
    Ok(data::read_worldbook_raw(root, world_id)?
        .values()
        .map(|raw| {
            let mut entry = v2_entry(raw);
            if let Some(characters) = raw
                .pointer("/extensions/table_tavern/visibility")
                .filter(|visibility| visibility.get("characters").is_some())
            {
                if let Some(extensions) = entry.get_mut("extensions").and_then(Value::as_object_mut)
                {
                    extensions.insert(
                        "table_tavern".to_owned(),
                        json!({ "visibility": characters }),
                    );
                }
            }
            entry
        })
        .collect())
}

/// `first_mes`：開場去掉 `[bg]`、`[choices]` 與選項行的散文版。
fn prose_opening(opening: &str) -> String {
    let mut choices = false;
    opening
        .lines()
        .filter(|line| {
            let line = line.trim();
            if line.starts_with("[choices") {
                choices = true;
                return false;
            }
            !(line.starts_with('[') && line.ends_with(']')) && !(choices && line.starts_with('-'))
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

fn creator_notes(origin: &CardOrigin) -> String {
    let level = serde_json::to_value(origin.rating.level)
        .ok()
        .and_then(|level| level.as_str().map(str::to_owned))
        .unwrap_or_default();
    let warnings = match origin.rating.warnings.is_empty() {
        true => String::new(),
        false => format!(" ({})", origin.rating.warnings.join(", ")),
    };
    format!("Rating: {level}{warnings}. Play in Table Tavern's visual novel mode; this card contains the full script and secrets in plain text.")
}
