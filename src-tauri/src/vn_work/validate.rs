//! 作品包的結構驗證（plans/vn-mode-pkg8.md §2.5 #9–#13、#11a）：格式版本、字串夾斷、幕樹、角色與資產參照、
//! 去重分岔展開後的事件數與文字量。建桌前在記憶體裡做完；匯出端也拿它自驗，保證自家包自家收得下。
use super::archive::broken;
use super::format::*;
use crate::data::vn::valid_id;
use crate::data::vn_art::valid_art_key;
use crate::ui_msg::UiMsg;
use crate::vn_sprites::store::valid_variant_key;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};

/// 匯入／匯出時給玩家看的一條提示
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notice {
    pub kind: NoticeKind,
    pub target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeKind {
    /// 文字超過上限，已截短（target＝幕號或欄位）
    Clipped,
    /// 匯出：這張圖驗不過，沒進包（target＝包內路徑）
    ImageSkipped,
    /// 附設定：卡原件的美術欄位一筆不合格，沒保存（target＝欄位路徑）
    Dropped,
}

pub(crate) fn header(manifest: &Manifest) -> Result<(), String> {
    if manifest.format != FORMAT {
        return Err(UiMsg::VnWorkNotPack.to_string());
    }
    if manifest.version != VERSION {
        return Err(UiMsg::VnWorkVersion {
            version: manifest.version,
        }
        .to_string());
    }
    Ok(())
}

/// 桌名：單行、夾斷，空的給預設名
pub(crate) fn clip_title(title: &mut String) {
    clip_line(title, TITLE_CHARS);
    if title.trim().is_empty() {
        *title = "VN".to_owned();
    }
}

/// 夾斷成單行、不超過 `cap` 字；有截回 true
fn clip_line(text: &mut String, cap: usize) -> bool {
    if text.contains(['\n', '\r']) {
        *text = text.replace(['\n', '\r'], " ");
    }
    clip(text, cap)
}

fn clip(text: &mut String, cap: usize) -> bool {
    match text.char_indices().nth(cap) {
        Some((cut, _)) => {
            text.truncate(cut);
            true
        }
        None => false,
    }
}

fn valid_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// 驗角色、劇本、資產並就地夾斷；回驗過的資產清單（照 manifest 順序、去重）。`has`＝包內有沒有這個條目
/// （匯出自驗時資產還沒寫，傳全收）。
pub(crate) fn check(
    manifest: &mut Manifest,
    script: &mut Script,
    cast: &mut Characters,
    limits: Limits,
    has: impl Fn(&str) -> bool,
    notices: &mut Vec<Notice>,
) -> Result<Vec<Asset>, String> {
    clip_title(&mut manifest.title);
    let ids = characters(cast, limits)?;
    let endings = story_tables(script)?;
    scenes(script, &ids, &endings, limits, notices)?;
    assets(manifest, &ids, limits, has)
}

fn characters(cast: &mut Characters, limits: Limits) -> Result<HashSet<String>, String> {
    if cast.characters.len() > limits.characters {
        return Err(UiMsg::VnWorkTooLarge.to_string());
    }
    let mut ids = HashSet::new();
    for member in &mut cast.characters {
        if crate::data::validate_id(&member.id).is_err() || !ids.insert(member.id.clone()) {
            return Err(broken(format!("character id {:?}", member.id)));
        }
        clip_line(&mut member.name, NAME_CHARS);
        if member.name.trim().is_empty() {
            member.name = "?".to_owned();
        }
        if !valid_color(&member.color) {
            member.color = "#888888".to_owned();
        }
        clip_line(&mut member.emoji, EMOJI_CHARS);
        member.height_cm = member.height_cm.filter(|cm| (30..=2000).contains(cm));
        let mut seen = HashSet::new();
        member
            .expressions_extra
            .retain(|key| valid_art_key(key) && seen.insert(key.clone()));
        member.expressions_extra.truncate(MAX_EXTRA_EXPRESSIONS);
        let mut seen = HashSet::new();
        member
            .variants
            .retain(|key| valid_variant_key(key) && seen.insert(key.clone()));
        member.variants.truncate(MAX_VARIANTS);
    }
    if cast
        .player
        .as_ref()
        .is_some_and(|player| !ids.contains(player))
    {
        return Err(broken("player not in characters".to_owned()));
    }
    Ok(ids)
}

/// 章名、結局名、地點；回結局 id 集合
fn story_tables(script: &mut Script) -> Result<HashSet<String>, String> {
    if script.chapters.len() > MAX_CHAPTER as usize
        || script.endings.len() > MAX_ENDINGS
        || script.locations.len() > MAX_LOCATIONS
    {
        return Err(UiMsg::VnWorkTooLarge.to_string());
    }
    for title in &mut script.chapters {
        clip_line(title, TITLE_CHARS);
    }
    let mut endings = HashSet::new();
    for ending in &mut script.endings {
        if !valid_id(&ending.id) || !endings.insert(ending.id.clone()) {
            return Err(broken(format!("ending id {:?}", ending.id)));
        }
        clip_line(&mut ending.title, TITLE_CHARS);
    }
    let mut keys = HashSet::new();
    for place in &mut script.locations {
        if !valid_art_key(&place.key) || !keys.insert(place.key.clone()) {
            return Err(broken(format!("location key {:?}", place.key)));
        }
        clip_line(&mut place.name, TITLE_CHARS);
        let mut seen = HashSet::new();
        place
            .times
            .retain(|time| valid_art_key(time) && seen.insert(time.clone()));
        place.times.truncate(8);
    }
    Ok(endings)
}

/// 幕樹、事件、展開後計限（#11、#11a）
fn scenes(
    script: &mut Script,
    ids: &HashSet<String>,
    endings: &HashSet<String>,
    limits: Limits,
    notices: &mut Vec<Notice>,
) -> Result<(), String> {
    let count = script.scenes.len();
    if count == 0 {
        return Err(broken("no scenes".to_owned()));
    }
    if count > limits.scenes {
        return Err(UiMsg::VnWorkTooLarge.to_string());
    }
    if script.current >= count as u64 {
        return Err(broken("current scene".to_owned()));
    }
    // 每幕展開後各則的累計落檔量（prefix[i][k]＝前 k 則），分岔前段從來源幕取
    let mut prefix: Vec<Vec<u64>> = Vec::with_capacity(count);
    let mut total_events = 0usize;
    let mut total_bytes = 0u64;
    let mut story_children: HashMap<u64, u64> = HashMap::new();
    for (index, scene) in script.scenes.iter_mut().enumerate() {
        let at = index as u64;
        if scene.scene != at {
            return Err(broken(format!("scene {} out of order", scene.scene)));
        }
        if let Some(title) = scene.title.as_mut() {
            clip_line(title, TITLE_CHARS);
        }
        let copied = label(scene, at, count, &prefix, &mut story_children)?;
        let mut sums = match copied {
            Some((source, keep)) => prefix[source as usize][..=keep].to_vec(),
            None => vec![0],
        };
        let mut clipped = false;
        for event in &mut scene.events {
            clipped |= self::event(event, ids, endings)?;
            let last = *sums.last().expect("至少有 0");
            sums.push(last + event.text.len() as u64 + STAGE_BYTES);
        }
        if clipped {
            notices.push(Notice {
                kind: NoticeKind::Clipped,
                target: format!("scene {}", at + 1),
            });
        }
        let full = sums.len() - 1;
        if full == 0 {
            return Err(broken(format!("scene {at} is empty")));
        }
        total_events += full;
        total_bytes += sums[full];
        if full > limits.scene_events
            || total_events > limits.total_events
            || total_bytes > limits.total_text_bytes
        {
            return Err(UiMsg::VnWorkTooLarge.to_string());
        }
        prefix.push(sums);
    }
    Ok(())
}

/// 驗一幕的標籤；回（來源幕, keep）＝要複製的前段
fn label(
    scene: &mut Scene,
    at: u64,
    count: usize,
    prefix: &[Vec<u64>],
    story_children: &mut HashMap<u64, u64>,
) -> Result<Option<(u64, usize)>, String> {
    let bad = |what: &str| Err(broken(format!("scene {at}: {what}")));
    let Some(label) = scene.label.as_mut() else {
        return match at {
            0 => Ok(None),
            _ => bad("missing label"),
        };
    };
    if at == 0 {
        return bad("first scene has a label");
    }
    label.base = label.base.min(count as u64);
    label.version = label.version.clamp(1, 99);
    if label.parent.is_some_and(|parent| parent >= at)
        || label.source.is_some_and(|source| source >= at)
        || label.forked != label.source.is_some()
        || (label.keep.is_some() && !label.forked)
    {
        return bad("label");
    }
    if !label.forked {
        if let Some(parent) = label.parent {
            if story_children.insert(parent, at).is_some() {
                return bad("second story child");
            }
        }
    }
    match (label.source, label.keep) {
        (Some(source), Some(keep)) => {
            let keep = keep as usize;
            if keep == 0 || keep >= prefix[source as usize].len() {
                return bad("keep");
            }
            Ok(Some((source, keep)))
        }
        _ => Ok(None),
    }
}

/// 驗一則並夾斷；回有沒有截字
fn event(
    event: &mut Event,
    ids: &HashSet<String>,
    endings: &HashSet<String>,
) -> Result<bool, String> {
    let mut clipped = false;
    match event.kind {
        Kind::System => {
            event.text.clear();
            event.speaker_name.clear();
            event.speaker = None;
        }
        Kind::Dialogue => {
            if event
                .speaker
                .as_ref()
                .is_some_and(|speaker| !ids.contains(speaker))
            {
                return Err(broken(format!("speaker {:?}", event.speaker)));
            }
        }
        Kind::Narration | Kind::Player => event.speaker = None,
    }
    clipped |= clip_line(&mut event.speaker_name, NAME_CHARS);
    clipped |= clip(&mut event.text, TEXT_CHARS);
    let stage = &mut event.stage;
    for text in [stage.place.as_mut(), stage.time.as_mut()]
        .into_iter()
        .flatten()
    {
        clipped |= clip_line(text, PLACE_CHARS);
    }
    if stage
        .chapter
        .is_some_and(|chapter| !(1..=MAX_CHAPTER).contains(&chapter))
    {
        return Err(broken(format!("chapter {:?}", stage.chapter)));
    }
    if stage.ended.as_ref().is_some_and(|id| !endings.contains(id)) {
        return Err(broken(format!("ending {:?}", stage.ended)));
    }
    Ok(clipped)
}

fn assets(
    manifest: &Manifest,
    ids: &HashSet<String>,
    limits: Limits,
    has: impl Fn(&str) -> bool,
) -> Result<Vec<Asset>, String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut images = 0usize;
    let mut sprites = BTreeSet::new();
    let mut bases = BTreeSet::new();
    for path in &manifest.assets {
        if !seen.insert(path.as_str()) {
            continue;
        }
        let asset = Asset::parse(path).ok_or_else(|| broken(format!("asset path {path:?}")))?;
        if asset.character().is_some_and(|id| !ids.contains(id)) {
            return Err(broken(format!("asset owner {path:?}")));
        }
        if !has(path) {
            return Err(broken(format!("missing {path}")));
        }
        match &asset {
            Asset::SpriteMeta { character } => {
                sprites.insert(character.clone());
            }
            Asset::SpriteBody { character, key } if key == "base" => {
                bases.insert(character.clone());
                images += 1;
            }
            _ => images += 1,
        }
        out.push(asset);
    }
    if images > limits.images {
        return Err(UiMsg::VnWorkTooLarge.to_string());
    }
    if let Some(character) = sprites.difference(&bases).next() {
        return Err(broken(format!("sprite {character} has no base")));
    }
    Ok(out)
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
