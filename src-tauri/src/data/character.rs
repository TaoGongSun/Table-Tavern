use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use super::{DataResult, Tier, invalid_data};
use super::paths::{character_path, gallery_dir, validate_id, validate_single_line, world_dir};
use super::state::read_state;

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

fn parse_sections(body: &str) -> (String, String) {
    #[derive(Clone, Copy)]
    enum Section {
        Public,
        Private,
    }

    let mut markers = Vec::new();
    let mut offset = 0;
    for segment in body.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let section = match line {
            "## 公開" => Some(Section::Public),
            "## 私有" => Some(Section::Private),
            _ => None,
        };
        if let Some(section) = section {
            markers.push((offset, offset + segment.len(), section));
        }
        offset += segment.len();
    }

    let mut public_md = String::new();
    let mut private_md = String::new();
    for (index, (_, content_start, section)) in markers.iter().copied().enumerate() {
        let content_end = markers
            .get(index + 1)
            .map(|(heading_start, _, _)| *heading_start)
            .unwrap_or(body.len());
        let mut content = &body[content_start..content_end];
        if index + 1 < markers.len() {
            content = content.strip_suffix('\n').unwrap_or(content);
        }
        match section {
            Section::Public => public_md = content.to_owned(),
            Section::Private => private_md = content.to_owned(),
        }
    }
    (public_md, private_md)
}

/// `auto_hidden` 不是 `CharacterCard` 的欄位（那樣每個手動建卡的呼叫端都要補這個跟編輯
/// 無關的欄位）：呼叫端自己決定要延續舊值（`write_character`）還是寫新值
/// （`set_character_auto_hidden`），見兩者呼叫這支的方式。
fn serialize_character(card: &CharacterCard, display_index: u32, auto_hidden: bool) -> String {
    // frontmatter 逐行解析，生成提示詞中的換行須在寫入前攤平。
    let gen_prompt = card.gen_prompt.replace(['\n', '\r'], " ");
    format!(
        "---\nid: {}\nname: {}\ncolor: {}\navatar: {}\ntier: {}\nshow_image: {}\narchived: {}\nauto_hidden: {}\ndisplay_index: {}\ngen_prompt: {}\n---\n## 公開\n{}\n## 私有\n{}",
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
        card.public_md,
        card.private_md
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
            match parse_frontmatter(&contents) {
                Ok((meta, _, _)) if player_card_id.as_deref() != Some(&meta.id) => {
                    characters.push(meta)
                }
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
        fs::write(path, serialize_character(&card, index, auto_hidden))?;
    }
    Ok(())
}

pub fn read_character(
    root: &Path,
    world_id: &str,
    character_id: &str,
) -> DataResult<CharacterCard> {
    let contents = fs::read_to_string(character_path(root, world_id, character_id)?)?;
    let (meta, gen_prompt, body) = parse_frontmatter(&contents)?;
    let (public_md, private_md) = parse_sections(body);
    Ok(CharacterCard {
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
    })
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
    read_character(root, world_id, &character_id).map(Some)
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
    fs::write(path, serialize_character(card, display_index, auto_hidden))?;
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
    fs::write(path, serialize_character(&card, display_index, auto_hidden))?;
    Ok(())
}

pub fn delete_character(root: &Path, world_id: &str, character_id: &str) -> DataResult<()> {
    let path = character_path(root, world_id, character_id)?;
    fs::remove_file(&path)?;
    let gallery = gallery_dir(root, world_id, character_id)?;
    if gallery.exists() {
        fs::remove_dir_all(gallery)?;
    }
    let image_path = path.with_extension("png");
    if image_path.exists() {
        fs::remove_file(image_path)?;
    }
    let avatar_path = path.with_extension("avatar.png");
    if avatar_path.exists() {
        fs::remove_file(avatar_path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::*;
    use crate::data::test_support::*;

    #[test]
    fn list_characters_excludes_player_card() {
        let root = TestRoot::new("player-card");
        let world_id = create_world(root.path(), "玩家卡桌").unwrap();
        let player = character_card(&new_id(), "阿濤");
        let npc = character_card(&new_id(), "狐狸");
        write_character(root.path(), &world_id, &player).unwrap();
        write_character(root.path(), &world_id, &npc).unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.player_card_id = Some(player.id.clone());
        write_state(root.path(), &world_id, &state).unwrap();

        assert_eq!(
            list_characters(root.path(), &world_id).unwrap(),
            vec![CharacterMeta {
                id: npc.id,
                name: npc.name,
                color: npc.color,
                avatar: npc.avatar,
                tier: npc.tier,
                show_image: npc.show_image,
                archived: npc.archived,
                auto_hidden: false,
                display_index: Some(1),
            }]
        );
        assert_eq!(
            read_player_card(root.path(), &world_id).unwrap(),
            Some(player)
        );
    }

    #[test]
    fn rejects_multiline_frontmatter_values() {
        let root = TestRoot::new("scalars");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut card = character_card(&new_id(), "角色");
        card.color = "#123456\ntier: best".to_owned();
        assert!(write_character(root.path(), &world_id, &card).is_err());
    }

    #[test]
    fn character_round_trip_preserves_fields_and_sections() {
        let root = TestRoot::new("character");
        let world_id = create_world(root.path(), "港灣").unwrap();
        let character_id = new_id();
        let card = CharacterCard {
            id: character_id.clone(),
            name: "阿藍".to_owned(),
            color: "#3366ff".to_owned(),
            avatar: "avatars/blue.png".to_owned(),
            tier: Tier::Best,
            show_image: true,
            archived: true,
            gen_prompt: "暖色調 水彩風".to_owned(),
            public_md: "第一段\n\n- 公開條目\n".to_owned(),
            private_md: "秘密第一行\n\n秘密第二行".to_owned(),
        };

        write_character(root.path(), &world_id, &card).unwrap();
        assert_eq!(
            read_character(root.path(), &world_id, &character_id).unwrap(),
            card
        );
        assert_eq!(
            list_characters(root.path(), &world_id).unwrap(),
            vec![CharacterMeta {
                id: character_id.clone(),
                name: "阿藍".to_owned(),
                color: "#3366ff".to_owned(),
                avatar: "avatars/blue.png".to_owned(),
                tier: Tier::Best,
                show_image: true,
                archived: true,
                auto_hidden: false,
                display_index: Some(0),
            }]
        );

        let raw = fs::read_to_string(
            root.path()
                .join(format!("worlds/{world_id}/characters/{character_id}.md")),
        )
        .unwrap();
        let frontmatter = raw
            .strip_prefix("---\n")
            .unwrap()
            .split_once("\n---\n")
            .unwrap()
            .0;
        let keys: Vec<_> = frontmatter
            .lines()
            .map(|line| line.split_once(':').unwrap().0)
            .collect();
        assert_eq!(
            keys,
            [
                "id",
                "name",
                "color",
                "avatar",
                "tier",
                "show_image",
                "archived",
                "auto_hidden",
                "display_index",
                "gen_prompt"
            ]
        );
        assert!(raw.contains("\n## 公開\n"));
        assert!(raw.contains("\n## 私有\n"));

        set_character_archived(root.path(), &world_id, &character_id, false).unwrap();
        assert!(
            !read_character(root.path(), &world_id, &character_id)
                .unwrap()
                .archived
        );
    }

    #[test]
    fn show_image_false_round_trips() {
        let root = TestRoot::new("show-image");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut card = character_card(&new_id(), "藏圖");
        card.show_image = false;
        write_character(root.path(), &world_id, &card).unwrap();
        assert!(
            !read_character(root.path(), &world_id, &card.id)
                .unwrap()
                .show_image
        );
    }

    /// 測試清單 #7：舊格式資料（缺 id）被略過，不會炸掉整份清單
    #[test]
    fn legacy_cards_and_worlds_without_id_are_skipped() {
        let root = TestRoot::new("legacy-skip");
        let world_id = create_world(root.path(), "世界").unwrap();

        // 舊卡沒有 id：list_characters 略過該檔，直接讀取也是錯
        fs::write(
            root.path()
                .join(format!("worlds/{world_id}/characters/舊卡.md")),
            "---\nname: 舊卡\ncolor: #111111\navatar: 🎭\ntier: default\n---\n## 公開\n\n## 私有\n",
        )
        .unwrap();
        // 有 id 的正常卡應該仍被列出
        let good_card = character_card(&new_id(), "正常卡");
        write_character(root.path(), &world_id, &good_card).unwrap();

        let characters = list_characters(root.path(), &world_id).unwrap();
        assert_eq!(characters.len(), 1);
        assert_eq!(characters[0].name, "正常卡");

        // 舊桌沒有 id/name：list_worlds 略過該桌
        let legacy_world_dir = root.path().join("worlds").join(new_id());
        fs::create_dir_all(legacy_world_dir.join("characters")).unwrap();
        fs::create_dir_all(legacy_world_dir.join("transcript")).unwrap();
        fs::write(
            legacy_world_dir.join("state.json"),
            serde_json::json!({ "current_scene": 0 }).to_string(),
        )
        .unwrap();

        let worlds = list_worlds(root.path()).unwrap();
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].id, world_id);
    }

    #[test]
    fn delete_character_removes_card_images_and_gallery() {
        let root = TestRoot::new("delete-character");
        let world_id = create_world(root.path(), "世界").unwrap();
        let card = character_card(&new_id(), "退場角色");
        write_character(root.path(), &world_id, &card).unwrap();
        let md_path = character_path(root.path(), &world_id, &card.id).unwrap();
        let png_path = md_path.with_extension("png");
        let avatar_path = md_path.with_extension("avatar.png");
        fs::write(&png_path, b"png").unwrap();
        fs::write(&avatar_path, b"avatar").unwrap();
        let gallery = gallery_dir(root.path(), &world_id, &card.id).unwrap();
        fs::create_dir_all(&gallery).unwrap();
        fs::write(gallery.join("1.png"), b"gen").unwrap();
        // 生成圖庫收在世界目錄內，不是放錯層的舊路徑
        assert!(gallery.starts_with(root.path().join("worlds").join(&world_id)));

        delete_character(root.path(), &world_id, &card.id).unwrap();

        assert!(list_characters(root.path(), &world_id).unwrap().is_empty());
        assert!(!md_path.exists());
        assert!(!png_path.exists());
        assert!(!avatar_path.exists());
        assert!(!gallery.exists());
    }

    /// 測試清單 #4：兩個同名角色可並存，各自讀寫互不干擾
    #[test]
    fn two_characters_with_same_name_coexist_independently() {
        let root = TestRoot::new("same-name-characters");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut first = character_card(&new_id(), "重名");
        first.public_md = "第一位".to_owned();
        let mut second = character_card(&new_id(), "重名");
        second.public_md = "第二位".to_owned();
        write_character(root.path(), &world_id, &first).unwrap();
        write_character(root.path(), &world_id, &second).unwrap();

        assert_eq!(
            read_character(root.path(), &world_id, &first.id)
                .unwrap()
                .public_md,
            "第一位"
        );
        assert_eq!(
            read_character(root.path(), &world_id, &second.id)
                .unwrap()
                .public_md,
            "第二位"
        );
        assert_eq!(list_characters(root.path(), &world_id).unwrap().len(), 2);
    }

    /// 測試清單 #5：改名（＝重存卡片）後路徑全部不變，transcript 舊事件保留舊名快照，
    /// model_bindings 不需改動仍指向同一角色
    #[test]
    fn rename_keeps_paths_and_preserves_transcript_snapshot() {
        let root = TestRoot::new("rename-character");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut card = character_card(&new_id(), "舊名");
        card.tier = Tier::Best;
        card.public_md = "舊名是個吟遊詩人".to_owned();
        write_character(root.path(), &world_id, &card).unwrap();
        let md_path = character_path(root.path(), &world_id, &card.id).unwrap();
        fs::write(md_path.with_extension("png"), b"png").unwrap();
        fs::write(md_path.with_extension("avatar.png"), b"avatar").unwrap();
        let gallery = gallery_dir(root.path(), &world_id, &card.id).unwrap();
        fs::create_dir_all(&gallery).unwrap();
        fs::write(gallery.join("1.png"), b"gen").unwrap();
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "2026-07-27 12:00".to_owned(),
                speaker_id: card.id.clone(),
                speaker_name: "舊名".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "舊名說了一句話".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();
        let mut entry = worldbook_entry(0, "條目");
        entry.visibility = Visibility::Characters(vec![card.id.clone(), "別的代碼".to_owned()]);
        upsert_worldbook_entry(root.path(), &world_id, entry).unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state
            .model_bindings
            .insert(card.id.clone(), "model-x".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();

        // 改名＝存一次卡片（前端就是這條路徑），id 不變所以什麼都不用搬
        let mut renamed_card = read_character(root.path(), &world_id, &card.id).unwrap();
        renamed_card.name = "新名".to_owned();
        write_character(root.path(), &world_id, &renamed_card).unwrap();

        let renamed = read_character(root.path(), &world_id, &card.id).unwrap();
        assert_eq!(renamed.name, "新名");
        assert_eq!(renamed.tier, Tier::Best);
        // 自然語言內文不動（拍板：機械取代會誤傷）
        assert_eq!(renamed.public_md, "舊名是個吟遊詩人");
        // 路徑全部不變——id 沒變，改名不搬檔
        assert!(md_path.exists());
        assert!(md_path.with_extension("png").exists());
        assert!(md_path.with_extension("avatar.png").exists());
        assert!(gallery.join("1.png").exists());

        // 改名後舊對話仍顯示舊名快照（2026-07-27 拍板）
        let events = read_transcript(root.path(), &world_id, 0).unwrap();
        assert_eq!(events[0].speaker_id, card.id);
        assert_eq!(events[0].speaker_name, "舊名");
        assert_eq!(events[0].text, "舊名說了一句話");

        // 世界書可見性存 id，改名後條目不需回填仍可見（測試清單 #11）
        assert_eq!(
            read_worldbook(root.path(), &world_id).unwrap()[0].visibility,
            Visibility::Characters(vec![card.id.clone(), "別的代碼".to_owned()])
        );
        assert_eq!(
            read_state(root.path(), &world_id)
                .unwrap()
                .model_bindings
                .get(&card.id)
                .map(String::as_str),
            Some("model-x")
        );
    }

    #[test]
    fn rename_rejects_bad_id_and_multiline_name_but_allows_duplicate() {
        let root = TestRoot::new("rename-character-guard");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut first = character_card(&new_id(), "甲");
        let second = character_card(&new_id(), "乙");
        write_character(root.path(), &world_id, &first).unwrap();
        write_character(root.path(), &world_id, &second).unwrap();

        let mut bad_id = first.clone();
        bad_id.id = "not-a-real-id".to_owned();
        assert!(write_character(root.path(), &world_id, &bad_id).is_err());
        let mut multiline = first.clone();
        multiline.name = "含換行\n的名字".to_owned();
        assert!(write_character(root.path(), &world_id, &multiline).is_err());

        // 同名不再擋——甲可以改名成跟乙一樣
        first.name = "乙".to_owned();
        write_character(root.path(), &world_id, &first).unwrap();
        assert_eq!(
            read_character(root.path(), &world_id, &first.id)
                .unwrap()
                .name,
            "乙"
        );
    }

    /// 測試清單 #10：reorder_characters 以 id 排序，封存角色仍接在後面
    #[test]
    fn reordering_characters_by_id_keeps_unlisted_after_listed() {
        let root = TestRoot::new("character-reorder");
        let world_id = create_world(root.path(), "世界").unwrap();
        let cards: Vec<_> = ["甲", "乙", "丙"]
            .into_iter()
            .map(|name| character_card(&new_id(), name))
            .collect();
        for card in &cards {
            write_character(root.path(), &world_id, card).unwrap();
        }
        let ids = |root: &Path| {
            list_characters(root, &world_id)
                .unwrap()
                .into_iter()
                .map(|meta| meta.id)
                .collect::<Vec<_>>()
        };
        // 建卡順序即初始順序
        assert_eq!(
            ids(root.path()),
            vec![
                cards[0].id.clone(),
                cards[1].id.clone(),
                cards[2].id.clone()
            ]
        );

        reorder_characters(
            root.path(),
            &world_id,
            &[cards[2].id.clone(), cards[0].id.clone()],
        )
        .unwrap();
        // 沒送到的「乙」接在後面
        assert_eq!(
            ids(root.path()),
            vec![
                cards[2].id.clone(),
                cards[0].id.clone(),
                cards[1].id.clone()
            ]
        );

        // 改名不動排序位置
        let mut renamed = read_character(root.path(), &world_id, &cards[0].id).unwrap();
        renamed.name = "甲二".to_owned();
        write_character(root.path(), &world_id, &renamed).unwrap();
        assert_eq!(
            ids(root.path()),
            vec![
                cards[2].id.clone(),
                cards[0].id.clone(),
                cards[1].id.clone()
            ]
        );

        // 重存不動位置，新卡排到最後
        set_character_archived(root.path(), &world_id, &cards[2].id, true).unwrap();
        let fourth = character_card(&new_id(), "丁");
        write_character(root.path(), &world_id, &fourth).unwrap();
        assert_eq!(
            ids(root.path()),
            vec![
                cards[2].id.clone(),
                cards[0].id.clone(),
                cards[1].id.clone(),
                fourth.id.clone()
            ]
        );
    }

    #[test]
    fn saving_one_card_without_display_index_does_not_reshuffle_the_others() {
        let root = TestRoot::new("character-legacy-order");
        let world_id = create_world(root.path(), "世界").unwrap();
        let ids: Vec<String> = ["甲", "乙", "丙"]
            .into_iter()
            .map(|name| {
                let id = new_id();
                // 有 id 但沒有 display_index，模擬這個欄位加入前存的卡
                fs::write(
                    root.path()
                        .join(format!("worlds/{world_id}/characters/{id}.md")),
                    format!(
                        "---\nid: {id}\nname: {name}\ncolor: #000000\navatar: 🎭\ntier: default\n---\n## 公開\n"
                    ),
                )
                .unwrap();
                id
            })
            .collect();
        // 沒有 display_index 的卡，顯示順序＝名字排序
        let names = |root: &Path| {
            list_characters(root, &world_id)
                .unwrap()
                .into_iter()
                .map(|meta| meta.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(root.path()), ["丙", "乙", "甲"]);

        set_character_archived(root.path(), &world_id, &ids[0], true).unwrap();

        assert_eq!(names(root.path()), ["丙", "乙", "甲"]);
    }

    #[test]
    fn frontmatter_accepts_spacing_and_order_but_rejects_invalid_tier() {
        let root = TestRoot::new("frontmatter");
        let world_id = create_world(root.path(), "世界").unwrap();
        let character_id = new_id();
        let path = root
            .path()
            .join(format!("worlds/{world_id}/characters/{character_id}.md"));
        fs::write(
            &path,
            format!(
                "---\ntier : fast\nunknown: ignored\navatar: 🐕\n color : #abcdef\nname : 角色\nid : {character_id}\n---\n## 私有\n私密"
            ),
        )
        .unwrap();
        assert_eq!(
            read_character(root.path(), &world_id, &character_id)
                .unwrap()
                .tier,
            Tier::Fast
        );

        fs::write(
            path,
            format!(
                "---\nid: {character_id}\nname: 角色\ncolor: #abcdef\navatar: 🐕\ntier: impossible\n---\n"
            ),
        )
        .unwrap();
        assert!(read_character(root.path(), &world_id, &character_id).is_err());
    }

    /// 舊角色卡 meta 沒有 auto_hidden（AI 卡重構包 4b 新欄位，封存三態的其中一態）
    /// 也要讀得起來，落回預設值 false。
    #[test]
    fn old_character_meta_json_without_auto_hidden_still_deserializes() {
        let json = r##"{
            "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "name": "阿藍",
            "color": "#3366ff",
            "avatar": "avatars/blue.png",
            "tier": "balanced"
        }"##;
        let meta: CharacterMeta = serde_json::from_str(json).unwrap();
        assert!(!meta.auto_hidden);
        assert!(!meta.archived);
    }

    /// AI 卡重構包 4b：write_character（前端編輯表單走的路徑，CharacterCard 本身不帶
    /// auto_hidden）改其他欄位時，延續磁碟上原有的 auto_hidden，不會被編輯表單悄悄清掉。
    #[test]
    fn write_character_preserves_auto_hidden_across_unrelated_edit() {
        let root = TestRoot::new("preserve-auto-hidden");
        let world_id = create_world(root.path(), "測試桌").unwrap();
        let mut card = character_card(&new_id(), "狐狸");
        write_character(root.path(), &world_id, &card).unwrap();
        set_character_auto_hidden(root.path(), &world_id, &card.id, true).unwrap();

        card.color = "#ff0000".to_owned();
        write_character(root.path(), &world_id, &card).unwrap();

        let meta = list_characters(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .find(|meta| meta.id == card.id)
            .unwrap();
        assert!(meta.auto_hidden);
        assert_eq!(meta.color, "#ff0000");
    }

}
