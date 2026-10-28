use super::paths::{character_path, gallery_dir, validate_id, validate_single_line, world_dir};
use super::state::read_state;
use super::{invalid_data, DataResult, Tier};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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
    /// 自動關掉；退回／分岔回到某一幕時照那幕離開時的快照寫回（scene/visibility.rs）。
    /// 跟 `archived`（玩家手動封存，系統永不自動改動）是獨立的兩軸，見
    /// `settle_card_visibility` 與 `set_character_auto_hidden`。
    #[serde(default)]
    pub auto_hidden: bool,
    /// 舞台卡（stage-card-in-speaker-list）：提供場景／世界設定、不參與角色發言。設定照送提示詞，
    /// 但不進點名名單、發言對象與換幕結算；只由 `set_character_stage` 改寫，卡片編輯延續磁碟舊值。
    #[serde(default)]
    pub stage: bool,
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
    /// 舞台卡旗標的唯讀投影：讀卡時從 frontmatter 帶出，給提示詞組裝分段用；不進前端卡片 JSON，
    /// 寫卡時一律延續磁碟上的值（改寫只走 `set_character_stage`）。
    #[serde(skip)]
    pub stage: bool,
}

/// 這張卡在桌上的角色（stage-card-in-speaker-list）：所有「誰算發言者、誰算舞台、誰不在」的判斷都從這裡來。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastRole {
    /// 在主區、可被點名與選為發言對象的角色
    Speaker,
    /// 舞台卡：設定照送，不發言、不參與換幕結算（自動隱藏旗標對它無效）
    Stage,
    /// 自動隱藏中的角色（回歸候選）
    Hidden,
    /// 玩家手動封存
    Archived,
}

impl CharacterMeta {
    pub fn cast_role(&self) -> CastRole {
        if self.archived {
            CastRole::Archived
        } else if self.stage {
            CastRole::Stage
        } else if self.auto_hidden {
            CastRole::Hidden
        } else {
            CastRole::Speaker
        }
    }
}

/// 卡檔 frontmatter 裡不屬於編輯表單、由系統另外改寫的旗標；改寫其他欄位時延續磁碟上的值。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SystemFlags {
    auto_hidden: bool,
    stage: bool,
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
    let mut stage = false;
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
            "stage" => stage = value == "true",
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
            stage,
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

/// `auto_hidden`、`stage` 不跟著編輯表單走：呼叫端自己決定要延續舊值（`write_character`）還是寫新值
/// （`set_character_auto_hidden`、`set_character_stage`），見各自呼叫這支的方式。
fn serialize_character(card: &CharacterCard, display_index: u32, flags: SystemFlags) -> String {
    // frontmatter 逐行解析，生成提示詞中的換行須在寫入前攤平。
    let gen_prompt = card.gen_prompt.replace(['\n', '\r'], " ");
    format!(
        "---\nid: {}\nname: {}\ncolor: {}\navatar: {}\ntier: {}\nshow_image: {}\narchived: {}\nauto_hidden: {}\n{}display_index: {}\ngen_prompt: {}\n---\n{PUBLIC_MARK}\n{}\n{PRIVATE_MARK}\n{}",
        card.id,
        card.name,
        card.color,
        card.avatar,
        card.tier.as_str(),
        card.show_image,
        card.archived,
        flags.auto_hidden,
        // 舞台旗標只在開啟時寫出：一般卡的 frontmatter 與加這欄之前逐字相同
        if flags.stage { "stage: true\n" } else { "" },
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
        // 拖曳排序只改 display_index，跟 write_character 一樣延續磁碟上原有的系統旗標。
        let flags = existing_flags(&path);
        super::world_file::commit_world_write(
            &path,
            serialize_character(&card, index, flags).as_bytes(),
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
        stage: meta.stage,
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

/// 這張卡目前落地的系統旗標；檔案不存在或解析失敗（新卡）一律當 false。
fn existing_flags(path: &Path) -> SystemFlags {
    let Ok(contents) = fs::read_to_string(path) else {
        return SystemFlags::default();
    };
    parse_frontmatter(&contents)
        .map(|(meta, _, _)| SystemFlags {
            auto_hidden: meta.auto_hidden,
            stage: meta.stage,
        })
        .unwrap_or_default()
}

/// id 由呼叫端先跟 new_id 要好（草稿期生圖需要落在正確的圖庫路徑）；空 id 直接回錯。
/// `CharacterCard` 不帶 auto_hidden（AI 卡重構包 4b：那是換幕結算與退回／分岔復原的持久欄位，不是編輯表單
/// 的一部分），舞台旗標同理；這裡改寫其他欄位時延續磁碟上原有的值，不會被前端編輯捎帶清掉。
pub fn write_character(root: &Path, world_id: &str, card: &CharacterCard) -> DataResult<()> {
    validate_id(&card.id)?;
    validate_single_line("name", &card.name)?;
    validate_single_line("color", &card.color)?;
    validate_single_line("avatar", &card.avatar)?;
    let path = character_path(root, world_id, &card.id)?;
    let flags = existing_flags(&path);
    ensure_display_indices(root, world_id)?;
    let display_index = display_index_for(root, world_id, &path)?;
    super::world_file::commit_world_write(
        &path,
        serialize_character(card, display_index, flags).as_bytes(),
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

/// 換幕結算（`settle_card_visibility`）、退回／分岔復原（scene/visibility.rs）與隱藏區手動拉回共用：
/// 直接寫入新的 auto_hidden 值，其餘欄位原樣保留，整檔原子寫（中途失敗卡檔不截壞）。
/// 不走 `write_character`（那支會延續磁碟舊值，寫不進新值）。
pub fn set_character_auto_hidden(
    root: &Path,
    world_id: &str,
    character_id: &str,
    auto_hidden: bool,
) -> DataResult<()> {
    let card = read_character(root, world_id, character_id)?;
    let flags = SystemFlags {
        auto_hidden,
        stage: card.stage,
    };
    write_flags(root, world_id, &card, flags)
}

/// 舞台旗標：開啟時一併清掉 auto_hidden（舊桌換幕時可能已把它收進隱藏區，清掉設定才恢復送出）；
/// 關閉時不動 auto_hidden，卡留在原處、下次換幕照一般規則結算。整檔原子寫。
pub fn set_character_stage(
    root: &Path,
    world_id: &str,
    character_id: &str,
    stage: bool,
) -> DataResult<()> {
    let card = read_character(root, world_id, character_id)?;
    let path = character_path(root, world_id, character_id)?;
    let flags = SystemFlags {
        auto_hidden: !stage && existing_flags(&path).auto_hidden,
        stage,
    };
    write_flags(root, world_id, &card, flags)
}

fn write_flags(
    root: &Path,
    world_id: &str,
    card: &CharacterCard,
    flags: SystemFlags,
) -> DataResult<()> {
    let path = character_path(root, world_id, &card.id)?;
    let display_index = display_index_for(root, world_id, &path)?;
    super::world_file::commit_world_write_atomic(
        &path,
        serialize_character(card, display_index, flags).as_bytes(),
    )?;
    Ok(())
}

/// 卡檔（`characters/<id>.md`）確定已不在：刪角色後清世界書名單的判準。查不到（權限、I/O 錯誤）
/// 回 Err，不當成已刪。
pub fn character_card_gone(root: &Path, world_id: &str, character_id: &str) -> DataResult<bool> {
    Ok(!character_path(root, world_id, character_id)?.try_exists()?)
}

/// 角色卡檔之外的附檔：圖庫、顯示圖、頭像、原卡（`.card.png`／`.import.json`）與匯入暫存檔。
fn character_attachments(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<[PathBuf; 8]> {
    let path = character_path(root, world_id, character_id)?;
    Ok([
        gallery_dir(root, world_id, character_id)?,
        path.with_extension("png"),
        path.with_extension("avatar.png"),
        path.with_extension("card.png"),
        path.with_extension("import.json"),
        // 匯入原子寫失敗時可能留下的暫存檔
        path.with_extension("card.png.tmp"),
        path.with_extension("import.json.tmp"),
        path.with_extension("png.tmp"),
    ])
}

/// 給玩家看的殘檔位置：相對這桌資料夾（`characters/<檔名>`、圖庫 `gen-gallery/<id>/`），不列絕對路徑。
fn leftover_name(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parent = path
        .parent()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if path.is_dir() {
        format!("{parent}/{name}/")
    } else {
        format!("{parent}/{name}")
    }
}

/// 刪角色：先刪卡檔（md），刪不掉就停；再逐一清附檔，單檔失敗不擋其他檔，最後以
/// `CharacterFilesLeft` 彙總沒清掉的檔名。全部可重做（不在的檔算已清）。
pub fn delete_character(root: &Path, world_id: &str, character_id: &str) -> DataResult<()> {
    super::world_file::commit_world_remove(&character_path(root, world_id, character_id)?)?;
    let failed: Vec<String> = character_attachments(root, world_id, character_id)?
        .iter()
        .filter(|leftover| super::world_file::commit_world_remove(leftover).is_err())
        .map(|leftover| leftover_name(leftover))
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(crate::ui_msg::UiMsg::CharacterFilesLeft {
            count: failed.len(),
            files: failed.join(", "),
        }
        .into_error())
    }
}

/// 這個角色還留在磁碟上的檔（卡檔與附檔）檔名；查不到的也列上（保守當成還在）。
pub fn character_files_present(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<Vec<String>> {
    let mut paths = vec![character_path(root, world_id, character_id)?];
    paths.extend(character_attachments(root, world_id, character_id)?);
    Ok(paths
        .iter()
        .filter(|path| path.try_exists().unwrap_or(true))
        .map(|path| leftover_name(path))
        .collect())
}

#[cfg(test)]
mod tests;
