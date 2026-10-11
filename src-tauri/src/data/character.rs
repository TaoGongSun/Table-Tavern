use super::paths::{character_path, gallery_dir, validate_id, validate_single_line, world_dir};
use super::state::read_state;
use super::{invalid_data, DataResult, Tier};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// 匯入卡附原 PNG 時的顯示開關（NewPlan §5.2）；舊卡與手建卡缺此欄一律視為 true
fn default_show_image() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterMeta {
    pub id: String,
    pub name: String,
    pub color: String,
    pub avatar: String,
    pub tier: Tier,
    #[serde(default = "default_show_image")]
    pub show_image: bool,
    #[serde(default)]
    pub archived: bool,
    /// 自動隱藏（AI 卡重構包 4b）：換幕結算時系統判斷「這幕沒出現」才打開，劇情拉回來就
    /// 自動關掉；跟 `archived`（玩家手動封存，系統永不自動改動）是獨立的兩軸，見
    /// `settle_card_visibility` 與 `set_character_auto_hidden`。
    #[serde(default)]
    pub auto_hidden: bool,
    /// 側欄卡片的顯示順序；只在後端流通（前端拿到的已是排好的清單）
    #[serde(skip)]
    pub display_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterCard {
    pub id: String,
    pub name: String,
    pub color: String,
    pub avatar: String,
    pub tier: Tier,
    #[serde(default = "default_show_image")]
    pub show_image: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub gen_prompt: String,
    pub public_md: String,
    pub private_md: String,
}

/// 完整讀取用：角色卡解得開才算這棵目錄能當現行程式的桌。
pub(crate) fn character_file_parses(contents: &str) -> DataResult<()> {
    parse_frontmatter(contents).map(|_| ())
}

fn parse_frontmatter(contents: &str) -> DataResult<(CharacterMeta, String, &str)> {
    let rest = contents
        .strip_prefix("---\n")
        .ok_or_else(|| invalid_data("character card must start with frontmatter"))?;
    let end = rest
        .find("\n---\n")
        .ok_or_else(|| invalid_data("character card frontmatter is not closed"))?;
    let frontmatter = &rest[..end];
    let body = &rest[end + "\n---\n".len()..];

    let mut id = None;
    let mut name = None;
    let mut color = None;
    let mut avatar = None;
    let mut tier = None;
    let mut show_image = true;
    let mut archived = false;
    let mut auto_hidden = false;
    let mut display_index = None;
    let mut gen_prompt = String::new();
    for line in frontmatter.lines() {
        let Some((key, value)) = line.split_once(':') else {
            if line.trim().is_empty() {
                continue;
            }
            return Err(invalid_data(format!("invalid frontmatter line: {line}")));
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "id" => id = Some(value.to_owned()),
            "name" => name = Some(value.to_owned()),
            "color" => color = Some(value.to_owned()),
            "avatar" => avatar = Some(value.to_owned()),
            "tier" => tier = Some(Tier::parse(value)?),
            "show_image" => show_image = value != "false",
            "archived" => archived = value == "true",
            "auto_hidden" => auto_hidden = value == "true",
            "display_index" => display_index = value.parse().ok(),
            "gen_prompt" => gen_prompt = value.to_owned(),
            _ => {}
        }
    }

    // 新格式一律要有 id；缺 id 視為解析失敗（舊資料不遷移、不偵測，交給呼叫端略過）
    let id = id.ok_or_else(|| invalid_data("frontmatter is missing id"))?;
    let name = name.ok_or_else(|| invalid_data("frontmatter is missing name"))?;
    Ok((
        CharacterMeta {
            id,
            name,
            color: color.ok_or_else(|| invalid_data("frontmatter is missing color"))?,
            avatar: avatar.ok_or_else(|| invalid_data("frontmatter is missing avatar"))?,
            tier: tier.ok_or_else(|| invalid_data("frontmatter is missing tier"))?,
            show_image,
            archived,
            auto_hidden,
            display_index,
        },
        gen_prompt,
        body,
    ))
}

/// 卡檔本文的公開／私有分隔線：獨占一行、語系無關。內文裡長得像分隔線的行寫入時跳脫（見 `escape_body`）。
const PUBLIC_MARK: &str = "<!-- tt:public -->";
const PRIVATE_MARK: &str = "<!-- tt:private -->";
const MARK_PREFIX: &str = "<!-- tt:";

/// 逐行跳脫：前導反斜線數 k、其後以 `<!-- tt:` 開頭的行，寫成 k+1 個反斜線。
/// 讀回時 k≥1 的這種行去掉一個；k=0 且整行等於分隔線才是分隔線，因此規則無歧義。
fn escape_body(text: &str) -> String {
    map_lines(text, |line| {
        let rest = line.trim_start_matches('\\');
        rest.starts_with(MARK_PREFIX).then(|| format!("\\{line}"))
    })
}

fn unescape_body(text: &str) -> String {
    map_lines(text, |line| {
        let rest = line.trim_start_matches('\\');
        (rest.len() < line.len() && rest.starts_with(MARK_PREFIX)).then(|| line[1..].to_owned())
    })
}

/// 逐行改寫，保留原本的 `\n`／`\r\n` 行尾；`rewrite` 回 None＝這行原樣。
fn map_lines(text: &str, rewrite: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len());
    for segment in text.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line = line.strip_suffix('\r').unwrap_or(line);
        match rewrite(line) {
            Some(new_line) => {
                out.push_str(&new_line);
                out.push_str(&segment[line.len()..]);
            }
            None => out.push_str(segment),
        }
    }
    out
}

/// 嚴格切段：恰好一行公開分隔線、之後恰好一行私有分隔線，且公開分隔線在本文開頭；
/// 重複、缺失、錯序都回 None（解析失敗，不默默填空）。
fn parse_sections(body: &str) -> Option<(String, String)> {
    let mut marks = Vec::new();
    let mut offset = 0;
    for segment in body.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line == PUBLIC_MARK || line == PRIVATE_MARK {
            marks.push((line == PUBLIC_MARK, offset, offset + segment.len()));
        }
        offset += segment.len();
    }
    let [(true, 0, public_start), (false, private_heading, private_start)] = marks[..] else {
        return None;
    };
    let public = &body[public_start..private_heading];
    let public = public.strip_suffix('\n').unwrap_or(public);
    let public = public.strip_suffix('\r').unwrap_or(public);
    Some((unescape_body(public), unescape_body(&body[private_start..])))
}

/// `auto_hidden` 不是 `CharacterCard` 的欄位（那樣每個手動建卡的呼叫端都要補這個跟編輯
/// 無關的欄位）：呼叫端自己決定要延續舊值（`write_character`）還是寫新值
/// （`set_character_auto_hidden`），見兩者呼叫這支的方式。
fn serialize_character(card: &CharacterCard, display_index: u32, auto_hidden: bool) -> String {
    // frontmatter 逐行解析，生成提示詞中的換行須在寫入前攤平。
    let gen_prompt = card.gen_prompt.replace(['\n', '\r'], " ");
    format!(
        "---\nid: {}\nname: {}\ncolor: {}\navatar: {}\ntier: {}\nshow_image: {}\narchived: {}\nauto_hidden: {}\ndisplay_index: {}\ngen_prompt: {}\n---\n{PUBLIC_MARK}\n{}\n{PRIVATE_MARK}\n{}",
        card.id,
        card.name,
        card.color,
        card.avatar,
        card.tier.as_str(),
        card.show_image,
        card.archived,
        auto_hidden,
        display_index,
        gen_prompt,
        escape_body(&card.public_md),
        escape_body(&card.private_md)
    )
}

/// 舊卡沒有 display_index：整批依目前顯示順序補齊，免得只有被存到的那張拿到索引而跳到最前
fn ensure_display_indices(root: &Path, world_id: &str) -> DataResult<()> {
    let existing = list_characters(root, world_id)?;
    if existing.iter().all(|meta| meta.display_index.is_some()) {
        return Ok(());
    }
    let ids: Vec<String> = existing.into_iter().map(|meta| meta.id).collect();
    reorder_characters(root, world_id, &ids)
}

/// 已存在的卡保留原位，新卡排到最後
fn display_index_for(root: &Path, world_id: &str, path: &Path) -> DataResult<u32> {
    if path.exists() {
        let contents = fs::read_to_string(path)?;
        if let Some(index) = parse_frontmatter(&contents)?.0.display_index {
            return Ok(index);
        }
    }
    Ok(list_characters(root, world_id)?
        .iter()
        .filter_map(|meta| meta.display_index)
        .max()
        .map_or(0, |max| max.saturating_add(1)))
}

/// 解析失敗（含缺 id 的舊卡）一律略過該檔，不中斷整份清單（舊資料不遷移、不偵測）。
pub fn list_characters(root: &Path, world_id: &str) -> DataResult<Vec<CharacterMeta>> {
    let directory = world_dir(root, world_id)?.join("characters");
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let player_card_id = read_state(root, world_id)
        .ok()
        .and_then(|state| state.player_card_id);
    let mut characters = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.path().extension().and_then(|value| value.to_str()) == Some("md")
        {
            let contents = fs::read_to_string(entry.path())?;
            // 段落也要切得開才算這桌有這張卡（舊格式卡檔照略過，檔案留著不動）
            match parse_frontmatter(&contents).and_then(|(meta, _, body)| {
                parse_sections(body)
                    .map(|_| meta)
                    .ok_or_else(|| invalid_data(SECTIONS_MALFORMED))
            }) {
                Ok(meta) if player_card_id.as_deref() != Some(&meta.id) => characters.push(meta),
                Ok(_) => {}
                Err(error) => {
                    eprintln!("略過無法解析的角色卡 {}: {error}", entry.path().display())
                }
            }
        }
    }
    // 沒有 display_index 的舊卡排在有索引的之後，彼此依名字排
    characters.sort_by(|left, right| {
        left.display_index
            .unwrap_or(u32::MAX)
            .cmp(&right.display_index.unwrap_or(u32::MAX))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(characters)
}

/// 側欄拖曳排序：ids 就是新的顯示順序，沒送到的（如封存角色）依原順序接在後面
pub fn reorder_characters(root: &Path, world_id: &str, ids: &[String]) -> DataResult<()> {
    let existing = list_characters(root, world_id)?;
    let mut ordered: Vec<&str> = Vec::with_capacity(existing.len());
    for id in ids {
        if existing.iter().any(|meta| &meta.id == id) && !ordered.contains(&id.as_str()) {
            ordered.push(id);
        }
    }
    for meta in &existing {
        if !ordered.contains(&meta.id.as_str()) {
            ordered.push(&meta.id);
        }
    }

    for (index, id) in ordered.iter().enumerate() {
        let index =
            u32::try_from(index).map_err(|_| invalid_data("character display_index overflow"))?;
        let card = read_character(root, world_id, id)?;
        let path = character_path(root, world_id, id)?;
        // 拖曳排序只改 display_index，跟 write_character 一樣延續磁碟上原有的 auto_hidden。
        let auto_hidden = existing_auto_hidden(&path);
        super::world_file::commit_world_write(
            &path,
            serialize_character(&card, index, auto_hidden).as_bytes(),
        )?;
    }
    Ok(())
}

pub fn read_character(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<CharacterCard> {
    let contents = fs::read_to_string(character_path(root, world_id, character_id)?)?;
    card_from_contents(&contents)?.ok_or_else(|| invalid_data(SECTIONS_MALFORMED))
}

const SECTIONS_MALFORMED: &str = "character card sections are malformed";

/// frontmatter 壞掉回錯；段落切不開回 `Ok(None)`，讓玩家上下文能把它當成沒有這張卡。
fn card_from_contents(contents: &str) -> DataResult<Option<CharacterCard>> {
    let (meta, gen_prompt, body) = parse_frontmatter(contents)?;
    let Some((public_md, private_md)) = parse_sections(body) else {
        return Ok(None);
    };
    Ok(Some(CharacterCard {
        id: meta.id,
        name: meta.name,
        color: meta.color,
        avatar: meta.avatar,
        tier: meta.tier,
        show_image: meta.show_image,
        archived: meta.archived,
        gen_prompt,
        public_md,
        private_md,
    }))
}

pub fn read_player_card(root: &Path, world_id: &str) -> DataResult<Option<CharacterCard>> {
    let Some(character_id) = read_state(root, world_id)
        .ok()
        .and_then(|state| state.player_card_id)
    else {
        return Ok(None);
    };
    let Ok(path) = character_path(root, world_id, &character_id) else {
        return Ok(None);
    };
    if !path.is_file() {
        return Ok(None);
    }
    // 玩家上下文：段落切不開（舊格式卡檔）視為沒有玩家卡；讀檔與 frontmatter 錯誤照傳
    card_from_contents(&fs::read_to_string(path)?)
}

/// 這張卡目前落地的 auto_hidden 值；檔案不存在或解析失敗（新卡）一律當 false。
fn existing_auto_hidden(path: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(path) else {
        return false;
    };
    parse_frontmatter(&contents)
        .map(|(meta, _, _)| meta.auto_hidden)
        .unwrap_or(false)
}

/// id 由呼叫端先跟 new_id 要好（草稿期生圖需要落在正確的圖庫路徑）；空 id 直接回錯。
/// `CharacterCard` 不帶 auto_hidden（AI 卡重構包 4b：那是換幕結算的持久欄位，不是編輯表單
/// 的一部分），這裡改寫其他欄位時，延續磁碟上原有的 auto_hidden，不會被前端編輯捎帶清掉。
pub fn write_character(root: &Path, world_id: &str, card: &CharacterCard) -> DataResult<()> {
    validate_id(&card.id)?;
    validate_single_line("name", &card.name)?;
    validate_single_line("color", &card.color)?;
    validate_single_line("avatar", &card.avatar)?;
    let path = character_path(root, world_id, &card.id)?;
    let auto_hidden = existing_auto_hidden(&path);
    ensure_display_indices(root, world_id)?;
    let display_index = display_index_for(root, world_id, &path)?;
    super::world_file::commit_world_write(
        &path,
        serialize_character(card, display_index, auto_hidden).as_bytes(),
    )?;
    Ok(())
}

pub fn set_character_archived(
    root: &Path,
    world_id: &str,
    character_id: &str,
    archived: bool,
) -> DataResult<()> {
    let mut card = read_character(root, world_id, character_id)?;
    card.archived = archived;
    write_character(root, world_id, &card)
}

/// 換幕結算（`settle_card_visibility`）專用：直接寫入新的 auto_hidden 值，其餘欄位原樣保留。
/// 不走 `write_character`（那支會延續磁碟舊值，寫不進新值）。
pub fn set_character_auto_hidden(
    root: &Path,
    world_id: &str,
    character_id: &str,
    auto_hidden: bool,
) -> DataResult<()> {
    let card = read_character(root, world_id, character_id)?;
    let path = character_path(root, world_id, character_id)?;
    let display_index = display_index_for(root, world_id, &path)?;
    super::world_file::commit_world_write(
        &path,
        serialize_character(&card, display_index, auto_hidden).as_bytes(),
    )?;
    Ok(())
}

/// 卡檔（`characters/<id>.md`）確定已不在：刪角色後清世界書名單的判準。查不到（權限、I/O 錯誤）
/// 回 Err，不當成已刪。
pub fn character_card_gone(root: &Path, world_id: &str, character_id: &str) -> DataResult<bool> {
    Ok(!character_path(root, world_id, character_id)?.try_exists()?)
}

pub fn delete_character(root: &Path, world_id: &str, character_id: &str) -> DataResult<()> {
    let path = character_path(root, world_id, character_id)?;
    super::world_file::commit_world_remove(&path)?;
    let gallery = gallery_dir(root, world_id, character_id)?;
    if gallery.exists() {
        super::world_file::commit_world_remove(&gallery)?;
    }
    let image_path = path.with_extension("png");
    if image_path.exists() {
        super::world_file::commit_world_remove(&image_path)?;
    }
    let avatar_path = path.with_extension("avatar.png");
    if avatar_path.exists() {
        super::world_file::commit_world_remove(&avatar_path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
