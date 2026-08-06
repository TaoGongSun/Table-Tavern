//! AI 卡重構套用：AI 讀整張匯入卡，把內容拆成角色／介面／機制三類產物（RefactorOutcome），
//! 玩家人審勾選（RefactorSelection）後套用落檔，可一鍵倒退。AI 呼叫是下一包的事，這裡只管
//! 「已經有一份 RefactorOutcome，怎麼套用、怎麼復原」——手寫 JSON 餵進 apply() 就能驗證整條路。

use crate::data::{
    self, CharacterCard, DataResult, FieldRule, StateNode, Tier, Trigger, Visibility,
    WorldbookEntry,
};
use crate::mechanism;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// 新角色卡色票，跟前端 App.tsx 的 PALETTE 同一組；新卡依桌上目前角色數輪替。
const PALETTE: [&str; 6] = [
    "#e07a5f", "#3d84a8", "#81b29a", "#f2a541", "#9b5de5", "#e56399",
];

/// 人物合集條目切出來的一位角色候選。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorCharacter {
    pub name: String,
    pub emoji: String,
    pub public_md: String,
    pub private_md: String,
    /// 這位角色是從哪條世界書條目切出來的；同一條目切出多人時 uid 重複。
    pub source_uid: String,
    /// 此人不升格為角色卡時，自己獨立世界書條目的全文。
    pub solo_entry_md: String,
}

/// 散文介面指令抽成的狀態樹候選。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorInterface {
    pub state_fields: serde_json::Value,
    pub source_uids: Vec<String>,
    /// 解析失敗退原文的雙軌保底。
    pub raw: String,
}

/// 欄位規則＋觸發表候選；rules／triggers 直接複用 data.rs 既有機制型別，不新造平行型別。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorMechanism {
    pub source_uid: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rules: BTreeMap<String, FieldRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<Trigger>,
}

/// 來源條目套用後剩下的總述。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRewrite {
    pub uid: String,
    pub remainder_md: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorOutcome {
    #[serde(default)]
    pub characters: Vec<RefactorCharacter>,
    #[serde(default)]
    pub interface: Option<RefactorInterface>,
    #[serde(default)]
    pub mechanisms: Vec<RefactorMechanism>,
    #[serde(default)]
    pub rewrites: Vec<SourceRewrite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorSelection {
    #[serde(default)]
    pub character_indices: Vec<usize>,
    #[serde(default)]
    pub apply_interface: bool,
    #[serde(default)]
    pub mechanism_indices: Vec<usize>,
}

/// 套用摘要，前端顯示用。
#[derive(Debug, Clone, Default, Serialize)]
pub struct RefactorApplySummary {
    pub new_characters: usize,
    pub new_entries: usize,
    pub rewritten_entries: usize,
    pub interface_applied: bool,
    pub mechanisms_applied: usize,
}

/// apply() 的完整結果：summary 給前端，其餘給呼叫端組收據（receipts::record_refactor_apply）。
pub struct RefactorApplyResult {
    pub summary: RefactorApplySummary,
    pub character_ids: Vec<String>,
    pub rewritten_entries: Vec<WorldbookEntry>,
}

/// 套用一份重構產物。落檔規則：
/// - 勾中的角色 → 新增角色卡（emoji 進頭像欄，其餘欄位比照 worldbook_entry_to_character 的預設）。
/// - 同來源條目沒勾的人：那條目至少一人被勾才逐一新增獨立條目（is_person=true）；
///   一人都沒勾＝條目原樣不動，也不產獨立條目。
/// - 有人被勾的來源條目 → 內容整條改寫成對應的 remainder_md。
/// - 介面勾了套用 → state_fields 併入狀態樹頂層、來源條目停用。
/// - 勾中的機制 → rules／triggers 併入 mechanism、來源條目停用。
pub fn apply(
    root: &Path,
    world_id: &str,
    outcome: &RefactorOutcome,
    selection: &RefactorSelection,
) -> DataResult<RefactorApplyResult> {
    let mut character_ids = Vec::new();
    let mut new_entries = 0usize;
    let mut rewritten: BTreeMap<u64, WorldbookEntry> = BTreeMap::new();
    let existing_character_count = data::list_characters(root, world_id)?.len();

    let mut by_source: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, character) in outcome.characters.iter().enumerate() {
        by_source
            .entry(character.source_uid.as_str())
            .or_default()
            .push(index);
    }

    for (source_uid, indices) in &by_source {
        let selected: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|index| selection.character_indices.contains(index))
            .collect();
        if selected.is_empty() {
            continue; // 沒勾的一律不動：這條目沒人被勾，原樣留著，也不產獨立條目。
        }

        for &index in &selected {
            let character = &outcome.characters[index];
            let card = CharacterCard {
                id: data::new_id(),
                name: character.name.clone(),
                color: PALETTE[(existing_character_count + character_ids.len()) % PALETTE.len()]
                    .to_owned(),
                avatar: character.emoji.clone(),
                tier: Tier::Balanced,
                show_image: true,
                archived: false,
                gen_prompt: String::new(),
                public_md: character.public_md.clone(),
                private_md: character.private_md.clone(),
            };
            data::write_character(root, world_id, &card)?;
            character_ids.push(card.id);
        }

        for &index in indices {
            if selected.contains(&index) {
                continue;
            }
            let character = &outcome.characters[index];
            // uid: u64::MAX 是「一定不會撞到既有條目」的哨兵——upsert 找不到既有 uid 才會
            // 真的新建，實際落檔的 uid 由 upsert_worldbook_entry 內部重新分配（見 data.rs）。
            data::upsert_worldbook_entry(
                root,
                world_id,
                WorldbookEntry {
                    uid: u64::MAX,
                    title: character.name.clone(),
                    keys: Vec::new(),
                    content: character.solo_entry_md.clone(),
                    constant: false,
                    order: 100,
                    disabled: false,
                    visibility: Visibility::Gm,
                    is_person: true,
                },
            )?;
            new_entries += 1;
        }

        rewrite_source_entry(root, world_id, source_uid, outcome, &mut rewritten)?;
    }

    let mut state = data::read_state(root, world_id)?;
    let mut state_dirty = false;

    let mut interface_applied = false;
    if selection.apply_interface {
        if let Some(interface) = &outcome.interface {
            merge_state_fields(&mut state.state.tree, &interface.state_fields);
            state_dirty = true;
            for uid in &interface.source_uids {
                disable_source_entry(root, world_id, uid, &mut rewritten)?;
            }
            interface_applied = true;
        }
    }

    let mut mechanisms_applied = 0usize;
    let mut ledger_records = Vec::new();
    for &index in &selection.mechanism_indices {
        let Some(mechanism) = outcome.mechanisms.get(index) else {
            continue;
        };
        for (path, rule) in &mechanism.rules {
            state.mechanism.rules.insert(path.clone(), rule.clone());
        }
        state.mechanism.triggers.extend(mechanism.triggers.iter().cloned());
        state_dirty = true;
        if let Some(record) = absorbed_ledger_record(root, world_id, &mechanism.source_uid) {
            ledger_records.push(record);
        }
        disable_source_entry(root, world_id, &mechanism.source_uid, &mut rewritten)?;
        mechanisms_applied += 1;
    }

    if state_dirty {
        data::write_state(root, world_id, &state)?;
    }
    if !ledger_records.is_empty() {
        mechanism::append_log(root, world_id, state.current_scene, &ledger_records);
    }

    Ok(RefactorApplyResult {
        summary: RefactorApplySummary {
            new_characters: character_ids.len(),
            new_entries,
            rewritten_entries: rewritten.len(),
            interface_applied,
            mechanisms_applied,
        },
        character_ids,
        rewritten_entries: rewritten.into_values().collect(),
    })
}

/// 有人被勾的來源條目：內容整條改寫成對應的 remainder_md；找不到 uid 或沒有對應的 rewrite
/// 就略過（不讓一條資料缺角拖垮整包套用）。改寫前的原文先記進 `rewritten`，undo 用。
fn rewrite_source_entry(
    root: &Path,
    world_id: &str,
    source_uid: &str,
    outcome: &RefactorOutcome,
    rewritten: &mut BTreeMap<u64, WorldbookEntry>,
) -> DataResult<()> {
    let Some(rewrite) = outcome.rewrites.iter().find(|item| item.uid == source_uid) else {
        return Ok(());
    };
    let Ok(uid) = source_uid.parse::<u64>() else {
        return Ok(());
    };
    let Some(entry) = data::read_worldbook(root, world_id)?
        .into_iter()
        .find(|entry| entry.uid == uid)
    else {
        return Ok(());
    };
    rewritten.entry(uid).or_insert_with(|| entry.clone());
    let mut updated = entry;
    updated.content = rewrite.remainder_md.clone();
    data::upsert_worldbook_entry(root, world_id, updated)?;
    Ok(())
}

/// 介面／機制套用後，來源條目停用（disabled=true）；已經是停用狀態就不重複記一筆
/// （undo 不該把「匯入前就關著」的條目意外打開）。
fn disable_source_entry(
    root: &Path,
    world_id: &str,
    uid_str: &str,
    rewritten: &mut BTreeMap<u64, WorldbookEntry>,
) -> DataResult<()> {
    let Ok(uid) = uid_str.parse::<u64>() else {
        return Ok(());
    };
    let Some(entry) = data::read_worldbook(root, world_id)?
        .into_iter()
        .find(|entry| entry.uid == uid)
    else {
        return Ok(());
    };
    if entry.disabled {
        return Ok(());
    }
    rewritten.entry(uid).or_insert_with(|| entry.clone());
    let mut updated = entry;
    updated.disabled = true;
    data::upsert_worldbook_entry(root, world_id, updated)?;
    Ok(())
}

/// 機制套用後記一筆已接管：來源條目原本在帳本裡是 Skipped，append_log 落檔後 read_ledger
/// 取「同標題最新一筆」會直接蓋成 Absorbed；原本不在帳本裡的純散文條目則等於新增一筆，
/// 讓玩家在帳本分頁看得到這條被收編了。uid 解不出來或條目已經不在就不記。
fn absorbed_ledger_record(root: &Path, world_id: &str, uid_str: &str) -> Option<mechanism::Record> {
    let uid: u64 = uid_str.parse().ok()?;
    let entry = data::read_worldbook(root, world_id)
        .ok()?
        .into_iter()
        .find(|entry| entry.uid == uid)?;
    Some(mechanism::Record {
        kind: mechanism::RecordKind::Absorbed,
        path: entry.title,
        detail: "AI 卡重構已收編此機制條目，併入欄位規則／觸發表，不再送入提示詞。".to_owned(),
    })
}

/// state_fields 併入狀態樹：頂層鍵整支覆寫——AI 產出的介面欄位最小可懂、也最好復原的合併
/// 語意，undo 端的 diff_mechanism 拿前後快照就能算出要退回的鍵，不用另外設計合併演算法。
fn merge_state_fields(tree: &mut BTreeMap<String, StateNode>, state_fields: &serde_json::Value) {
    let Some(object) = state_fields.as_object() else {
        return;
    };
    for (key, value) in object {
        tree.insert(key.clone(), json_to_state_node(value));
    }
}

fn json_to_state_node(value: &serde_json::Value) -> StateNode {
    match value {
        serde_json::Value::Object(object) => StateNode::Branch(
            object
                .iter()
                .map(|(key, value)| (key.clone(), json_to_state_node(value)))
                .collect(),
        ),
        serde_json::Value::Array(items) => StateNode::Branch(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| (index.to_string(), json_to_state_node(item)))
                .collect(),
        ),
        serde_json::Value::String(text) => StateNode::Leaf(text.clone()),
        serde_json::Value::Number(number) => StateNode::Leaf(number.to_string()),
        serde_json::Value::Bool(flag) => StateNode::Leaf(flag.to_string()),
        serde_json::Value::Null => StateNode::Leaf(String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipts;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "table-tavern-refactor-{label}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn seed_entry(root: &Path, world_id: &str, title: &str, content: &str) -> u64 {
        data::upsert_worldbook_entry(
            root,
            world_id,
            WorldbookEntry {
                uid: u64::MAX,
                title: title.to_owned(),
                keys: Vec::new(),
                content: content.to_owned(),
                constant: false,
                order: 1,
                disabled: false,
                visibility: Visibility::Gm,
                is_person: false,
            },
        )
        .unwrap()
    }

    fn character(name: &str, source_uid: u64) -> RefactorCharacter {
        RefactorCharacter {
            name: name.to_owned(),
            emoji: "🙂".to_owned(),
            public_md: format!("{name}的公開設定"),
            private_md: format!("{name}的私密設定"),
            source_uid: source_uid.to_string(),
            solo_entry_md: format!("{name}的獨立條目"),
        }
    }

    /// 比照 receipts.rs 既有測試的作法：套用前先 snapshot，套用後記收據，undo 走 receipts 那條路。
    fn apply_recorded(
        root: &Path,
        world_id: &str,
        outcome: &RefactorOutcome,
        selection: &RefactorSelection,
    ) -> RefactorApplyResult {
        let before = receipts::snapshot(root, world_id);
        let result = apply(root, world_id, outcome, selection).unwrap();
        receipts::record_refactor_apply(
            root,
            world_id,
            "AI 卡重構",
            result.character_ids.clone(),
            result.rewritten_entries.clone(),
            before,
        );
        result
    }

    /// (a) 全勾套用：角色卡落檔、來源條目＝remainder、機制併入 state → undo → 逐項回原樣。
    #[test]
    fn apply_all_selected_then_undo_restores_everything() {
        let root = TestRoot::new("all-selected");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "旅人們", "莉亞與可可的合集設定");
        let mechanism_uid = seed_entry(root.path(), &world_id, "[mvu_update]规则", "HP 規則腳本");

        let outcome = RefactorOutcome {
            characters: vec![character("莉亞", source_uid)],
            interface: None,
            mechanisms: vec![RefactorMechanism {
                source_uid: mechanism_uid.to_string(),
                rules: BTreeMap::from([(
                    "Player.HP".to_owned(),
                    FieldRule::for_kind(data::FieldKind::Number),
                )]),
                triggers: Vec::new(),
            }],
            rewrites: vec![SourceRewrite {
                uid: source_uid.to_string(),
                remainder_md: "莉亞已升格，剩下的旅人待補".to_owned(),
            }],
        };
        let selection = RefactorSelection {
            character_indices: vec![0],
            apply_interface: false,
            mechanism_indices: vec![0],
        };

        let result = apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert_eq!(result.summary.new_characters, 1);
        assert_eq!(result.summary.new_entries, 0);
        assert_eq!(result.summary.rewritten_entries, 2); // 來源條目改寫 + 機制來源停用
        assert_eq!(result.summary.mechanisms_applied, 1);

        let character_id = result.character_ids[0].clone();
        assert!(data::read_character(root.path(), &world_id, &character_id).is_ok());
        let rewritten = data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .find(|entry| entry.uid == source_uid)
            .unwrap();
        assert_eq!(rewritten.content, "莉亞已升格，剩下的旅人待補");
        let state = data::read_state(root.path(), &world_id).unwrap();
        assert!(state.mechanism.rules.contains_key("Player.HP"));
        let mechanism_entry = data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .find(|entry| entry.uid == mechanism_uid)
            .unwrap();
        assert!(mechanism_entry.disabled);

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        assert!(data::read_character(root.path(), &world_id, &character_id).is_err());
        let restored = data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .find(|entry| entry.uid == source_uid)
            .unwrap();
        assert_eq!(restored.content, "莉亞與可可的合集設定");
        let restored_mechanism_entry = data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .find(|entry| entry.uid == mechanism_uid)
            .unwrap();
        assert!(!restored_mechanism_entry.disabled);
        let state_after = data::read_state(root.path(), &world_id).unwrap();
        assert!(!state_after.mechanism.rules.contains_key("Player.HP"));
    }

    /// (b) 一條來源條目切出七人只勾兩人：角色卡 +2、沒勾的五人各生一條 is_person 條目、
    /// 原條目＝remainder → undo → 逐項回原樣。也驗證新條目靠既有 uid 集合 diff 就能刪乾淨，
    /// 不需要額外補記 uid 清單。
    #[test]
    fn apply_partial_group_selection_creates_person_entries_for_the_rest() {
        let root = TestRoot::new("partial-group");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "旅團", "七人旅團的合集設定");

        let names = ["甲", "乙", "丙", "丁", "戊", "己", "庚"];
        let characters: Vec<RefactorCharacter> =
            names.iter().map(|name| character(name, source_uid)).collect();
        let outcome = RefactorOutcome {
            characters,
            interface: None,
            mechanisms: Vec::new(),
            rewrites: vec![SourceRewrite {
                uid: source_uid.to_string(),
                remainder_md: "剩下五人待補".to_owned(),
            }],
        };
        let selection = RefactorSelection {
            character_indices: vec![0, 1], // 甲、乙
            apply_interface: false,
            mechanism_indices: Vec::new(),
        };

        let before_entries = data::read_worldbook(root.path(), &world_id).unwrap().len();
        let result = apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert_eq!(result.summary.new_characters, 2);
        assert_eq!(result.summary.new_entries, 5);
        assert_eq!(result.summary.rewritten_entries, 1);
        assert_eq!(data::list_characters(root.path(), &world_id).unwrap().len(), 2);

        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), before_entries + 5);
        let person_names: Vec<&str> = entries
            .iter()
            .filter(|entry| entry.is_person)
            .map(|entry| entry.title.as_str())
            .collect();
        assert_eq!(person_names.len(), 5);
        for name in ["丙", "丁", "戊", "己", "庚"] {
            assert!(person_names.contains(&name));
        }
        let rewritten = entries.iter().find(|entry| entry.uid == source_uid).unwrap();
        assert_eq!(rewritten.content, "剩下五人待補");

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        assert_eq!(data::list_characters(root.path(), &world_id).unwrap().len(), 0);
        let after_undo = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(after_undo.len(), before_entries);
        let restored = after_undo.iter().find(|entry| entry.uid == source_uid).unwrap();
        assert_eq!(restored.content, "七人旅團的合集設定");
    }

    /// (c) 某來源條目一人都沒勾：原樣不動，也不產獨立條目——即使該 uid 有對應的 rewrite。
    #[test]
    fn apply_skips_untouched_source_entry_when_nobody_selected() {
        let root = TestRoot::new("zero-selected");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let picked_uid = seed_entry(root.path(), &world_id, "被選中的條目", "會被升格的人");
        let ignored_uid = seed_entry(root.path(), &world_id, "沒被勾的條目", "沒人被勾的合集設定");

        let outcome = RefactorOutcome {
            characters: vec![character("阿明", picked_uid), character("小華", ignored_uid)],
            interface: None,
            mechanisms: Vec::new(),
            rewrites: vec![
                SourceRewrite {
                    uid: picked_uid.to_string(),
                    remainder_md: "阿明已升格".to_owned(),
                },
                SourceRewrite {
                    uid: ignored_uid.to_string(),
                    remainder_md: "不該套用到這裡".to_owned(),
                },
            ],
        };
        let selection = RefactorSelection {
            character_indices: vec![0], // 只勾阿明；小華（index 1）沒勾
            apply_interface: false,
            mechanism_indices: Vec::new(),
        };

        let before_len = data::read_worldbook(root.path(), &world_id).unwrap().len();
        let result = apply(root.path(), &world_id, &outcome, &selection).unwrap();
        assert_eq!(result.summary.new_characters, 1);
        assert_eq!(result.summary.new_entries, 0); // 小華那條目沒人被勾，不產獨立條目
        assert_eq!(result.summary.rewritten_entries, 1); // 只有 picked_uid 被改寫

        let entries = data::read_worldbook(root.path(), &world_id).unwrap();
        assert_eq!(entries.len(), before_len); // 沒有新增條目
        let ignored_entry = entries.iter().find(|entry| entry.uid == ignored_uid).unwrap();
        assert_eq!(ignored_entry.content, "沒人被勾的合集設定"); // 原樣不動
        assert!(!ignored_entry.is_person);
    }

    /// (d) 介面套用：state_fields 併入狀態樹頂層、來源條目停用 → undo → 都退回去。
    #[test]
    fn apply_interface_merges_state_and_disables_source_then_undo_restores() {
        let root = TestRoot::new("interface");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "介面腳本", "描述如何顯示狀態欄的散文");

        let outcome = RefactorOutcome {
            characters: Vec::new(),
            interface: Some(RefactorInterface {
                state_fields: serde_json::json!({
                    "World": { "Time": "清晨", "Weather": "晴" }
                }),
                source_uids: vec![source_uid.to_string()],
                raw: "描述如何顯示狀態欄的散文".to_owned(),
            }),
            mechanisms: Vec::new(),
            rewrites: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: true,
            mechanism_indices: Vec::new(),
        };

        let result = apply_recorded(root.path(), &world_id, &outcome, &selection);
        assert!(result.summary.interface_applied);
        assert_eq!(result.summary.rewritten_entries, 1);

        let state = data::read_state(root.path(), &world_id).unwrap();
        assert_eq!(
            state.state.tree.get("World"),
            Some(&StateNode::Branch(BTreeMap::from([
                ("Time".to_owned(), StateNode::Leaf("清晨".to_owned())),
                ("Weather".to_owned(), StateNode::Leaf("晴".to_owned())),
            ])))
        );
        let source_entry = data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .find(|entry| entry.uid == source_uid)
            .unwrap();
        assert!(source_entry.disabled);

        receipts::undo_last_import(root.path(), &world_id).unwrap();
        let state_after = data::read_state(root.path(), &world_id).unwrap();
        assert!(state_after.state.tree.get("World").is_none());
        let source_entry_after = data::read_worldbook(root.path(), &world_id)
            .unwrap()
            .into_iter()
            .find(|entry| entry.uid == source_uid)
            .unwrap();
        assert!(!source_entry_after.disabled);
    }

    /// (e) 帳本轉換：來源條目原本在帳本裡是 Skipped（例如認不出的 EJS），套用機制後帳本要
    /// 改記 Absorbed——玩家在帳本分頁看到的是「已被收編」，不再是「跳過」。
    #[test]
    fn apply_mechanism_converts_ledger_entry_from_skipped_to_absorbed() {
        let root = TestRoot::new("ledger-skipped-to-absorbed");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "詭異的機制腳本", "<% 認不出的 EJS %>");
        mechanism::append_log(
            root.path(),
            &world_id,
            0,
            &[mechanism::Record {
                kind: mechanism::RecordKind::Skipped,
                path: "詭異的機制腳本".to_owned(),
                detail: "卡片腳本認不出來，沒轉成觸發表，預設不送模型。".to_owned(),
            }],
        );

        let outcome = RefactorOutcome {
            characters: Vec::new(),
            interface: None,
            mechanisms: vec![RefactorMechanism {
                source_uid: source_uid.to_string(),
                rules: BTreeMap::from([(
                    "World.詭異值".to_owned(),
                    FieldRule::for_kind(data::FieldKind::Number),
                )]),
                triggers: Vec::new(),
            }],
            rewrites: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: vec![0],
        };

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        let ledger = mechanism::read_ledger(root.path(), &world_id);
        let entry = ledger
            .entries
            .iter()
            .find(|entry| entry.title == "詭異的機制腳本")
            .unwrap();
        assert_eq!(entry.kind, mechanism::RecordKind::Absorbed);
    }

    /// (f) 帳本新增：純散文機制條目（帳本裡原本沒有這條）套用後要新增一筆 Absorbed 記錄。
    #[test]
    fn apply_mechanism_adds_absorbed_ledger_entry_for_entry_with_no_prior_record() {
        let root = TestRoot::new("ledger-new-absorbed");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "純散文機制", "打鬥時擲骰決勝負。");

        let outcome = RefactorOutcome {
            characters: Vec::new(),
            interface: None,
            mechanisms: vec![RefactorMechanism {
                source_uid: source_uid.to_string(),
                rules: BTreeMap::from([(
                    "World.戰鬥值".to_owned(),
                    FieldRule::for_kind(data::FieldKind::Number),
                )]),
                triggers: Vec::new(),
            }],
            rewrites: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: vec![0],
        };

        assert!(mechanism::read_ledger(root.path(), &world_id).entries.is_empty());

        apply(root.path(), &world_id, &outcome, &selection).unwrap();

        let ledger = mechanism::read_ledger(root.path(), &world_id);
        let entry = ledger
            .entries
            .iter()
            .find(|entry| entry.title == "純散文機制")
            .unwrap();
        assert_eq!(entry.kind, mechanism::RecordKind::Absorbed);
    }

    /// (g) undo 帳本回退：套用前是 Skipped，套用後變 Absorbed，undo 之後帳本要退回原本的
    /// Skipped 記錄。
    #[test]
    fn apply_mechanism_then_undo_restores_ledger_to_previous_state() {
        let root = TestRoot::new("ledger-undo");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let source_uid = seed_entry(root.path(), &world_id, "詭異的機制腳本二號", "<% 認不出的 EJS %>");
        mechanism::append_log(
            root.path(),
            &world_id,
            0,
            &[mechanism::Record {
                kind: mechanism::RecordKind::Skipped,
                path: "詭異的機制腳本二號".to_owned(),
                detail: "卡片腳本認不出來，沒轉成觸發表，預設不送模型。".to_owned(),
            }],
        );

        let outcome = RefactorOutcome {
            characters: Vec::new(),
            interface: None,
            mechanisms: vec![RefactorMechanism {
                source_uid: source_uid.to_string(),
                rules: BTreeMap::from([(
                    "World.詭異值二".to_owned(),
                    FieldRule::for_kind(data::FieldKind::Number),
                )]),
                triggers: Vec::new(),
            }],
            rewrites: Vec::new(),
        };
        let selection = RefactorSelection {
            character_indices: Vec::new(),
            apply_interface: false,
            mechanism_indices: vec![0],
        };

        apply_recorded(root.path(), &world_id, &outcome, &selection);
        let applied = mechanism::read_ledger(root.path(), &world_id);
        assert_eq!(
            applied
                .entries
                .iter()
                .find(|entry| entry.title == "詭異的機制腳本二號")
                .unwrap()
                .kind,
            mechanism::RecordKind::Absorbed
        );

        receipts::undo_last_import(root.path(), &world_id).unwrap();

        let after_undo = mechanism::read_ledger(root.path(), &world_id);
        let entry = after_undo
            .entries
            .iter()
            .find(|entry| entry.title == "詭異的機制腳本二號")
            .unwrap();
        assert_eq!(entry.kind, mechanism::RecordKind::Skipped);
    }
}
