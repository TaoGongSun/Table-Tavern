use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use super::{DataResult, Tier, invalid_data, new_id};
use super::character::{CharacterCard, list_characters, read_character, write_character};
use super::paths::{interface_shell_path, refactor_outcome_path, validate_single_line, world_dir, worlds_dir};
use super::scene::{TranscriptEvent, TranscriptKind, append_transcript};
use super::state::{Mechanism, TableState, WorldState, read_state, write_state};
use super::worldbook::{read_worldbook, read_worldbook_value};

/// 側欄桌列表用的精簡視圖
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldMeta {
    pub id: String,
    pub name: String,
}

/// 最後活動時間＝transcript 內最新檔案 mtime，退而求其次用世界目錄 mtime
fn last_active(world_directory: &Path) -> std::time::SystemTime {
    let mut latest = fs::metadata(world_directory)
        .and_then(|meta| meta.modified())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    if let Ok(entries) = fs::read_dir(world_directory.join("transcript")) {
        for entry in entries.flatten() {
            if let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) {
                latest = latest.max(modified);
            }
        }
    }
    latest
}

/// 依最後活動排序（新的在前），供側欄桌列表用（NewPlan §9.3）。
/// state.json 解析失敗（含舊格式缺 id/name）的桌一律略過，不寫遷移、不做偵測提示。
pub fn list_worlds(root: &Path) -> DataResult<Vec<WorldMeta>> {
    let directory = worlds_dir(root);
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut worlds = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let state_path = entry.path().join("state.json");
        let state = fs::read_to_string(&state_path)
            .ok()
            .and_then(|text| serde_json::from_str::<WorldState>(&text).ok());
        match state {
            Some(state) => worlds.push((
                last_active(&entry.path()),
                WorldMeta {
                    id: state.id,
                    name: state.name,
                },
            )),
            None => eprintln!("略過無法解析的桌：{}", entry.path().display()),
        }
    }
    worlds.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.name.cmp(&b.1.name))
            .then_with(|| a.1.id.cmp(&b.1.id))
    });
    Ok(worlds.into_iter().map(|(_, meta)| meta).collect())
}

pub fn create_world(root: &Path, name: &str) -> DataResult<String> {
    validate_single_line("world name", name)?;
    let id = new_id();
    let directory = worlds_dir(root).join(&id);
    fs::create_dir_all(worlds_dir(root))?;
    fs::create_dir(&directory)?;
    fs::create_dir(directory.join("characters"))?;
    fs::create_dir(directory.join("transcript"))?;
    fs::write(directory.join("world.md"), "")?;
    let state = WorldState {
        id: id.clone(),
        name: name.to_owned(),
        model_bindings: BTreeMap::new(),
        player_card_id: None,
        current_scene: 0,
        catchup_summaries: BTreeMap::new(),
        scene_titles: BTreeMap::new(),
        scene_labels: BTreeMap::new(),
        state: TableState::default(),
        mechanism: Mechanism::default(),
        aligned_scene: None,
        branch_bindings: BTreeMap::new(),
        refactor_mode: None,
    };
    fs::write(
        directory.join("state.json"),
        serde_json::to_string_pretty(&state)?,
    )?;
    Ok(id)
}

#[derive(Deserialize)]
struct SampleCharacterText {
    name: String,
    public_md: String,
    private_md: String,
}

#[derive(Deserialize)]
struct SampleWorldText {
    world_name: String,
    world_md: String,
    opening: String,
    characters: Vec<SampleCharacterText>,
}

fn sample_world_text(lang: &str) -> DataResult<SampleWorldText> {
    // 新增語系時只需新增 JSON 檔，並在這張對應表加一行；範例內容會隨執行檔靜態內嵌。
    let source = match lang {
        "zh-CN" => include_str!("../../samples/zh-CN.json"),
        "en" => include_str!("../../samples/en.json"),
        "ja" => include_str!("../../samples/ja.json"),
        "ko" => include_str!("../../samples/ko.json"),
        "es" => include_str!("../../samples/es.json"),
        "pt-BR" => include_str!("../../samples/pt-BR.json"),
        "de" => include_str!("../../samples/de.json"),
        "fr" => include_str!("../../samples/fr.json"),
        "ru" => include_str!("../../samples/ru.json"),
        _ => include_str!("../../samples/zh-TW.json"),
    };
    serde_json::from_str(source)
        .map_err(|error| invalid_data(format!("invalid embedded sample world JSON: {error}")))
}

/// 範例桌內容依語系產生（首開先選語言再建桌）；lang 非 en 一律走 zh-TW
pub fn create_sample_world(root: &Path, lang: &str) -> DataResult<String> {
    let sample = sample_world_text(lang)?;
    // 冪等：範例桌已在就直接沿用，避免重複呼叫（dev 的 StrictMode 雙跑）噴重複資料
    if let Some(existing) = list_worlds(root)?
        .into_iter()
        .find(|meta| meta.name == sample.world_name)
    {
        return Ok(existing.id);
    }
    let world_id = create_world(root, &sample.world_name)?;
    write_world_md(root, &world_id, &sample.world_md)?;

    let style = [
        ("#e07a5f", "🦊", Tier::Balanced),
        ("#3d84a8", "🛡️", Tier::Balanced),
        ("#f2a541", "🪕", Tier::Fast),
    ];
    if sample.characters.len() != style.len() {
        return Err(invalid_data(
            "sample world must contain exactly three characters",
        ));
    }
    for (text, (color, avatar, tier)) in sample.characters.into_iter().zip(style) {
        write_character(
            root,
            &world_id,
            &CharacterCard {
                id: new_id(),
                name: text.name,
                color: color.to_owned(),
                avatar: avatar.to_owned(),
                tier,
                show_image: true,
                archived: false,
                gen_prompt: String::new(),
                public_md: text.public_md,
                private_md: text.private_md,
            },
        )?;
    }

    append_transcript(
        root,
        &world_id,
        0,
        &TranscriptEvent {
            raw: None,
            ts: "2026-07-20T00:00:00+08:00".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: sample.opening,
            state: None,
            gm_only: false,
        },
    )?;

    Ok(world_id)
}

/// 空桌回收（NewPlan §9.3）：只回收完全未動過的桌——零訊息、零角色、零世界書條目、
/// world.md 空白；任一項有內容即保留，防資料遺失。回傳是否真的刪了。
pub fn reclaim_world_if_empty(root: &Path, world_id: &str) -> DataResult<bool> {
    let directory = world_dir(root, world_id)?;
    if !directory.exists() {
        return Ok(false);
    }
    let has_messages = fs::read_dir(directory.join("transcript"))
        .map(|entries| {
            entries
                .flatten()
                .any(|entry| entry.metadata().map(|meta| meta.len() > 0).unwrap_or(true))
        })
        .unwrap_or(false);
    let has_characters = fs::read_dir(directory.join("characters"))
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    // 世界書讀不動（檔案壞了）就當作有內容，寧可留著讓使用者自己處理，也不要刪掉
    let has_worldbook = read_worldbook_value(root, world_id)
        .map(|value| {
            value
                .get("entries")
                .and_then(serde_json::Value::as_object)
                .is_none_or(|entries| !entries.is_empty())
        })
        .unwrap_or(true);
    let world_md = fs::read_to_string(directory.join("world.md")).unwrap_or_default();
    if has_messages || has_characters || has_worldbook || !world_md.trim().is_empty() {
        return Ok(false);
    }
    fs::remove_dir_all(directory)?;
    Ok(true)
}

/// 刪桌：世界資料夾整包清掉（生成圖庫已收在世界目錄內，一併刪除）。不可復原。
pub fn delete_world(root: &Path, world_id: &str) -> DataResult<()> {
    let directory = world_dir(root, world_id)?;
    if directory.exists() {
        fs::remove_dir_all(&directory)?;
    }
    Ok(())
}

/// 桌名隨時可改（NewPlan §9.3）：只改 state.json 的 name，目錄路徑（world_id）不動。
pub fn rename_world(root: &Path, world_id: &str, new_name: &str) -> DataResult<()> {
    validate_single_line("world name", new_name)?;
    let mut state = read_state(root, world_id)?;
    state.name = new_name.to_owned();
    write_state(root, world_id, &state)
}

pub fn read_world_md(root: &Path, world_id: &str) -> DataResult<String> {
    Ok(fs::read_to_string(
        world_dir(root, world_id)?.join("world.md"),
    )?)
}

pub fn write_world_md(root: &Path, world_id: &str, content: &str) -> DataResult<()> {
    fs::write(world_dir(root, world_id)?.join("world.md"), content)?;
    Ok(())
}

/// 讀介面渲染殼檔；沒產過或還沒套用就是 None（前端退回既有沙盒殼／保底狀態欄，不是錯誤）。
pub fn read_interface_shell(root: &Path, world_id: &str) -> DataResult<Option<String>> {
    let path = interface_shell_path(root, world_id)?;
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(fs::read_to_string(path)?))
}

pub fn write_interface_shell(root: &Path, world_id: &str, content: &str) -> DataResult<()> {
    fs::write(interface_shell_path(root, world_id)?, content)?;
    Ok(())
}

/// 讀 AI 卡重構套用成功時落下的完整產物（已是 to_string_pretty 過的 JSON 原文）；沒套用過
/// 就是 None（前端匯出鈕靠這個判斷要不要顯示「這桌還沒有重構產物」）。
pub fn read_refactor_outcome(root: &Path, world_id: &str) -> DataResult<Option<String>> {
    let path = refactor_outcome_path(root, world_id)?;
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(fs::read_to_string(path)?))
}

pub fn write_refactor_outcome(root: &Path, world_id: &str, content: &str) -> DataResult<()> {
    fs::write(refactor_outcome_path(root, world_id)?, content)?;
    Ok(())
}

/// 狀態列的顯示格式一律由匯入的內容自己帶。比對詞跟 transport::extract_state_block
/// 認得的區塊一致——認得的才剝得出欄位，也才有東西可顯示。
const STATE_BAR_MARKERS: [&str; 12] = [
    "状态栏",
    "狀態欄",
    "状态条",
    "狀態條",
    "状态面板",
    "狀態面板",
    "status bar",
    "statusbar",
    // 標籤名各家自取，`<status` 開頭一律算（`<StatusData>`、`<Status_block>`）
    "<status",
    "<updatevariable",
    "```state",
    "```status",
];

fn declares_state_bar(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    STATE_BAR_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}

/// 這桌要不要顯示狀態列：世界設定、世界書條目、角色卡任一處講到狀態列就顯示。
/// 匯入的卡有可能把狀態列規則放在世界書、也可能放在卡片內文，三處都掃才不會漏。
pub fn world_has_state_bar(root: &Path, world_id: &str) -> DataResult<bool> {
    if read_world_md(root, world_id).is_ok_and(|world_md| declares_state_bar(&world_md)) {
        return Ok(true);
    }
    if read_worldbook(root, world_id)?.iter().any(|entry| {
        !entry.disabled && (declares_state_bar(&entry.content) || declares_state_bar(&entry.title))
    }) {
        return Ok(true);
    }
    Ok(list_characters(root, world_id)?.iter().any(|meta| {
        read_character(root, world_id, &meta.id).is_ok_and(|card| {
            declares_state_bar(&card.public_md) || declares_state_bar(&card.private_md)
        })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::*;
    use crate::data::test_support::*;

    /// 測試清單 #1：create_world 回 id；state.json 含 id/name；list_worlds 回 WorldMeta
    #[test]
    fn create_world_returns_id_with_state_and_meta() {
        let root = TestRoot::new("worlds");
        assert!(list_worlds(root.path()).unwrap().is_empty());

        let world_id = create_world(root.path(), "群島").unwrap();
        assert!(root.path().join("worlds").join(&world_id).is_dir());
        assert!(root
            .path()
            .join("worlds")
            .join(&world_id)
            .join("characters")
            .is_dir());
        assert!(root
            .path()
            .join("worlds")
            .join(&world_id)
            .join("transcript")
            .is_dir());
        assert!(root
            .path()
            .join("worlds")
            .join(&world_id)
            .join("world.md")
            .is_file());
        let state_raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(
                root.path()
                    .join("worlds")
                    .join(&world_id)
                    .join("state.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(state_raw["id"], world_id);
        assert_eq!(state_raw["name"], "群島");

        let worlds = list_worlds(root.path()).unwrap();
        assert_eq!(
            worlds,
            vec![WorldMeta {
                id: world_id,
                name: "群島".to_owned()
            }]
        );
    }

    /// 測試清單 #2：兩桌同名可並存，各自獨立 id 與內容
    #[test]
    fn two_worlds_with_same_name_coexist_with_independent_ids() {
        let root = TestRoot::new("worlds-same-name");
        let first = create_world(root.path(), "同名桌").unwrap();
        let second = create_world(root.path(), "同名桌").unwrap();
        assert_ne!(first, second);

        write_world_md(root.path(), &first, "第一桌的設定").unwrap();
        write_world_md(root.path(), &second, "第二桌的設定").unwrap();
        assert_eq!(read_world_md(root.path(), &first).unwrap(), "第一桌的設定");
        assert_eq!(read_world_md(root.path(), &second).unwrap(), "第二桌的設定");

        let names: Vec<_> = list_worlds(root.path())
            .unwrap()
            .into_iter()
            .map(|meta| meta.name)
            .collect();
        assert_eq!(names, vec!["同名桌", "同名桌"]);
    }

    /// 測試清單 #3：rename_world 後目錄路徑不變，只有 state.json 的 name 變
    #[test]
    fn rename_world_keeps_directory_and_changes_only_name() {
        let root = TestRoot::new("rename-world");
        let world_id = create_world(root.path(), "舊名").unwrap();
        let directory = root.path().join("worlds").join(&world_id);
        assert!(directory.is_dir());

        rename_world(root.path(), &world_id, "新名").unwrap();

        assert!(directory.is_dir());
        assert_eq!(read_state(root.path(), &world_id).unwrap().name, "新名");
        assert_eq!(read_state(root.path(), &world_id).unwrap().id, world_id);

        assert!(rename_world(root.path(), &world_id, "含換行\n的名字").is_err());
    }

    #[test]
    fn sample_world_is_ready_to_play() {
        let root = TestRoot::new("sample-world");
        let world_id = create_sample_world(root.path(), "zh-TW").unwrap();

        let worlds = list_worlds(root.path()).unwrap();
        assert!(worlds.iter().any(|meta| meta.id == world_id));

        let characters = list_characters(root.path(), &world_id).unwrap();
        assert_eq!(characters.len(), 3);
        for name in ["狐狸", "騎士", "吟遊詩人"] {
            assert!(characters.iter().any(|character| character.name == name));
        }

        let world_md = read_world_md(root.path(), &world_id).unwrap();
        assert!(!world_md.is_empty());
        assert!(world_md.contains("霧口鎮"));

        let transcript = read_transcript(root.path(), &world_id, 0).unwrap();
        assert_eq!(transcript.len(), 1);
        assert_eq!(transcript[0].kind, TranscriptKind::Narration);
        assert_eq!(transcript[0].speaker_name, "GM");
        assert_eq!(transcript[0].speaker_id, "");

        // 測試清單 #13：重複呼叫要沿用既有那桌，不重複塞開場旁白
        assert_eq!(create_sample_world(root.path(), "zh-TW").unwrap(), world_id);
        assert_eq!(read_transcript(root.path(), &world_id, 0).unwrap().len(), 1);
        assert_eq!(list_worlds(root.path()).unwrap().len(), worlds.len());
    }

    #[test]
    fn sample_world_english_content_follows_lang() {
        let root = TestRoot::new("sample-world-en");
        let world_id = create_sample_world(root.path(), "en").unwrap();

        let characters = list_characters(root.path(), &world_id).unwrap();
        assert_eq!(characters.len(), 3);
        for name in ["Fox", "Knight", "Bard"] {
            assert!(characters.iter().any(|character| character.name == name));
        }
        assert!(read_world_md(root.path(), &world_id)
            .unwrap()
            .contains("Mistmouth"));
        let transcript = read_transcript(root.path(), &world_id, 0).unwrap();
        assert!(transcript[0].text.starts_with("Rain hammers"));
    }

    /// 驗收：每個上架語系都有自己的範例桌內容且建得起來——
    /// 少一個 samples/<lang>.json、欄位漏了、或桌名忘了翻，都會在這裡爆
    #[test]
    fn sample_world_ready_in_every_language() {
        let zh_root = TestRoot::new("sample-world-lang-zh");
        let zh_id = create_sample_world(zh_root.path(), "zh-TW").unwrap();
        let zh_name = read_state(zh_root.path(), &zh_id).unwrap().name;

        for lang in ["zh-CN", "en", "ja", "ko", "es", "pt-BR", "de", "fr", "ru"] {
            let root = TestRoot::new(&format!("sample-world-lang-{lang}"));
            let world_id = create_sample_world(root.path(), lang).unwrap();

            let characters = list_characters(root.path(), &world_id).unwrap();
            assert_eq!(characters.len(), 3, "{lang} 角色數不對");
            for meta in &characters {
                assert!(!meta.name.trim().is_empty(), "{lang} 角色沒名字");
                let card = read_character(root.path(), &world_id, &meta.id).unwrap();
                assert!(!card.public_md.trim().is_empty(), "{lang} 缺公開設定");
                assert!(!card.private_md.trim().is_empty(), "{lang} 缺 GM 秘密");
            }

            assert!(
                !read_world_md(root.path(), &world_id)
                    .unwrap()
                    .trim()
                    .is_empty(),
                "{lang} 世界設定是空的"
            );

            let transcript = read_transcript(root.path(), &world_id, 0).unwrap();
            assert_eq!(transcript.len(), 1, "{lang} 開場旁白數不對");
            assert!(
                !transcript[0].text.trim().is_empty(),
                "{lang} 開場旁白是空的"
            );

            assert_ne!(
                read_state(root.path(), &world_id).unwrap().name,
                zh_name,
                "{lang} 桌名沒翻"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn lists_worlds_by_last_activity_descending() {
        let root = TestRoot::new("activity");
        let first = create_world(root.path(), "甲桌").unwrap();
        let second = create_world(root.path(), "乙桌").unwrap();

        // 兩桌目錄 mtime 撥回一小時前：同時間時按顯示名升冪（乙 U+4E59 < 甲 U+7532）
        let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        for id in [&first, &second] {
            let directory = fs::File::open(root.path().join("worlds").join(id)).unwrap();
            directory.set_modified(hour_ago).unwrap();
        }
        assert_eq!(
            list_worlds(root.path())
                .unwrap()
                .into_iter()
                .map(|meta| meta.name)
                .collect::<Vec<_>>(),
            vec!["乙桌", "甲桌"]
        );

        // 對名稱排序居後的甲桌寫一筆訊息，活動排序應把它推到最前
        let event = TranscriptEvent {
            raw: None,
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "你好".to_owned(),
            state: None,
            gm_only: false,
        };
        append_transcript(root.path(), &first, 0, &event).unwrap();
        assert_eq!(
            list_worlds(root.path())
                .unwrap()
                .into_iter()
                .map(|meta| meta.name)
                .collect::<Vec<_>>(),
            vec!["甲桌", "乙桌"]
        );
    }

    #[test]
    fn reclaims_only_untouched_worlds() {
        let root = TestRoot::new("reclaim");
        let empty = create_world(root.path(), "空桌").unwrap();
        assert!(reclaim_world_if_empty(root.path(), &empty).unwrap());
        assert!(list_worlds(root.path()).unwrap().is_empty());
        // 已刪的桌再回收一次應為 no-op
        assert!(!reclaim_world_if_empty(root.path(), &empty).unwrap());

        let has_message = create_world(root.path(), "有訊息").unwrap();
        let event = TranscriptEvent {
            raw: None,
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "留著".to_owned(),
            state: None,
            gm_only: false,
        };
        append_transcript(root.path(), &has_message, 0, &event).unwrap();
        assert!(!reclaim_world_if_empty(root.path(), &has_message).unwrap());

        let has_character = create_world(root.path(), "有角色").unwrap();
        write_character(
            root.path(),
            &has_character,
            &character_card(&new_id(), "旅人"),
        )
        .unwrap();
        assert!(!reclaim_world_if_empty(root.path(), &has_character).unwrap());

        let has_setting = create_world(root.path(), "有設定").unwrap();
        write_world_md(root.path(), &has_setting, "海島世界").unwrap();
        assert!(!reclaim_world_if_empty(root.path(), &has_setting).unwrap());

        // 匯入世界書但還沒改桌名、也還沒開聊，一樣算動過（回歸：曾被誤刪整桌）
        let has_worldbook = create_world(root.path(), "有世界書").unwrap();
        upsert_worldbook_entry(root.path(), &has_worldbook, worldbook_entry(1, "霧之港")).unwrap();
        assert!(!reclaim_world_if_empty(root.path(), &has_worldbook).unwrap());

        // 世界書檔案壞掉時保守保留，不刪
        let broken_worldbook = create_world(root.path(), "壞世界書").unwrap();
        fs::write(
            root.path()
                .join(format!("worlds/{broken_worldbook}/worldbook.json")),
            "{ not json",
        )
        .unwrap();
        assert!(!reclaim_world_if_empty(root.path(), &broken_worldbook).unwrap());
    }

    #[test]
    fn delete_world_removes_directory_including_gallery() {
        let root = TestRoot::new("delete-world");
        let to_delete = create_world(root.path(), "要刪的桌").unwrap();
        let to_keep = create_world(root.path(), "留著的桌").unwrap();
        let character_id = new_id();
        let gallery = gallery_dir(root.path(), &to_delete, &character_id).unwrap();
        fs::create_dir_all(&gallery).unwrap();
        fs::write(gallery.join("1.png"), b"gen").unwrap();

        delete_world(root.path(), &to_delete).unwrap();

        assert_eq!(
            list_worlds(root.path())
                .unwrap()
                .into_iter()
                .map(|meta| meta.id)
                .collect::<Vec<_>>(),
            vec![to_keep]
        );
        assert!(!root.path().join("worlds").join(&to_delete).exists());
        // 已刪的桌再刪一次應為 no-op；非法 id 擋下
        delete_world(root.path(), &to_delete).unwrap();
        assert!(delete_world(root.path(), "not-a-valid-ulid").is_err());
    }

    /// 狀態列只跟著匯入內容走：光提到「狀態」不算，要有狀態列輸出格式才算。
    #[test]
    fn state_bar_follows_imported_content() {
        let root = TestRoot::new("state-bar-detection");
        let world_id = create_world(root.path(), "世界").unwrap();
        assert!(!world_has_state_bar(root.path(), &world_id).unwrap());

        let mut prose = worldbook_entry(u64::MAX, "獵物狀態設定");
        prose.content = "User 的身體狀態具備超高耐受。".to_owned();
        upsert_worldbook_entry(root.path(), &world_id, prose).unwrap();
        assert!(!world_has_state_bar(root.path(), &world_id).unwrap());

        let mut rules = worldbook_entry(u64::MAX, "Day Counter");
        rules.content =
            "<details>\n<summary>状态栏</summary>\n- 沦陷天数：第 [X] 天\n</details>".to_owned();
        upsert_worldbook_entry(root.path(), &world_id, rules).unwrap();
        assert!(world_has_state_bar(root.path(), &world_id).unwrap());
    }

    /// 狀態列規則也可能寫在卡片內文（匯入角色卡時世界書會併進私有段）
    #[test]
    fn state_bar_detected_in_character_card() {
        let root = TestRoot::new("state-bar-character");
        let world_id = create_world(root.path(), "世界").unwrap();
        let mut card = character_card(&new_id(), "教官");
        card.private_md = "每次回覆結尾輸出 <UpdateVariable> 區塊。".to_owned();
        write_character(root.path(), &world_id, &card).unwrap();

        assert!(world_has_state_bar(root.path(), &world_id).unwrap());
    }
}
