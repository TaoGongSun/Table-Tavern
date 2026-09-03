use crate::data::{self, FieldKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use super::apply::{apply_updates, signed_delta_mark};
use super::derive::{recompute_derived, reroll};
use super::parse::parse_updates;
use super::rules::rule_for_path;
use super::tree::{format_num, leaf_at, numeric_value};
use super::triggers::evaluate_triggers;
use super::types::{Outcome, Patch, Record, RecordKind};

/// 全量桌跳動標記的門檻：兩個條件都要達到才算「跳」，寧可少標也不要一直誤報
/// （模型每回合都會有些許措辭差異造成的小數字漂移，不該被當成幻覺）。
/// 絕對幅度：小數值欄位（如個位數好感度）漲跌幾點很正常，不到 30 不算異常。
const JUMP_ABS_THRESHOLD: f64 = 30.0;
/// 相對幅度：大數值欄位（如上千的聲望）漲跌 30 只是零頭，要佔舊值／新值中較大者的四成才算異常。
const JUMP_RATIO_THRESHOLD: f64 = 0.4;

// ---------------------------------------------------------------------
// 回合套用：平欄／樹欄照舊套用＋增量走本地權威＋骰值每回合重擲＋觸發表求值
// ---------------------------------------------------------------------

/// 全量桌跳動比對用：這個路徑目前的舊值（平欄查 table，樹查現有 leaf_at）。
fn old_field_value(world: &data::WorldState, path: &[String]) -> Option<String> {
    if path.len() == 1 {
        world.state.table.get(&path[0]).cloned()
    } else {
        leaf_at(&world.state.tree, &path.join(".")).map(str::to_owned)
    }
}

/// 全量桌跳動比對：這一輪有報的每個路徑，新舊值都抽得出數字、不是計數器欄、
/// 幅度同時過絕對與相對門檻才算「跳」——命中就標上面板記號、記一筆 Jump 給玩家看。
fn detect_jumps(
    world: &mut data::WorldState,
    fields: &[(Vec<String>, String)],
    old_values: &[Option<String>],
    records: &mut Vec<Record>,
) {
    world.state.jumps.clear();
    for ((path, new_value), old_value) in fields.iter().zip(old_values) {
        let Some(old_value) = old_value else { continue };
        let (Some(old_num), Some(new_num)) = (numeric_value(old_value), numeric_value(new_value))
        else {
            continue;
        };
        let rule = rule_for_path(&world.mechanism, path, Some(old_value.as_str()));
        if rule.kind == FieldKind::Counter {
            continue;
        }
        let delta = new_num - old_num;
        if delta.abs() < JUMP_ABS_THRESHOLD
            || delta.abs() < JUMP_RATIO_THRESHOLD * old_num.abs().max(new_num.abs())
        {
            continue;
        }
        let path_str = path.join(".");
        let mark = signed_delta_mark(delta);
        world.state.jumps.insert(path_str.clone(), mark.clone());
        records.push(Record::new(
            RecordKind::Jump,
            path_str.clone(),
            format!(
                "{path_str} 一回合內從 {} 跳到 {}（{mark}），疑似模型算錯；\
                 若這欄本來就該大幅變動（例如天數計數器），可在面板點記號標成計數器，之後不再提醒。",
                format_num(old_num),
                format_num(new_num)
            ),
        ));
    }
}

/// 把一則回覆的狀態區塊套進這桌：平欄照舊、增量走本地權威、骰值每回合重擲、
/// 觸發表求值（模型套用到樹之後才查表）、全量桌跳動比對（只給玩家看，不進提示詞）。
/// `user_name` 供觸發文本的 `{{user}}` 代換。
pub fn apply_block(
    world: &mut data::WorldState,
    block: &crate::transport::StateBlock,
    user_name: &str,
) -> Outcome {
    let jump_check = !world.mechanism.incremental;
    let old_values: Vec<Option<String>> = if jump_check {
        block
            .fields
            .iter()
            .map(|(path, _)| old_field_value(world, path))
            .collect()
    } else {
        Vec::new()
    };

    for (path, value) in &block.fields {
        if path.len() == 1 {
            world.state.table.insert(path[0].clone(), value.clone());
        } else {
            data::set_tree_value(&mut world.state.tree, path, value);
        }
    }
    let patches: Vec<Patch> = block
        .updates
        .iter()
        .flat_map(|update| parse_updates(update))
        .collect();
    let mut outcome = apply_updates(&mut world.state.tree, &world.mechanism, &patches);
    if jump_check {
        detect_jumps(world, &block.fields, &old_values, &mut outcome.records);
    }
    if world.mechanism.incremental {
        reroll(&mut world.state.tree, &world.mechanism);
        let triggered = evaluate_triggers(&world.state.tree, &world.mechanism, user_name);
        world.state.triggers = triggered.hits;
        for flag in &triggered.flags {
            let segments: Vec<String> = flag.split('.').map(str::to_owned).collect();
            data::set_tree_value(&mut world.state.tree, &segments, "true");
            outcome.records.push(Record::new(
                RecordKind::Absorbed,
                flag.clone(),
                format!("一次性事件已觸發，旗標 {flag} 釘死為 true，不再重演。"),
            ));
        }
    }
    // 衍生值不分全量／增量桌都要重算：模型看不到 rare 欄位、也算不出它的值，
    // 全靠本地用這一輪剛套用完的樹重新求一次。
    outcome
        .records
        .extend(recompute_derived(&mut world.state.tree, &world.mechanism));
    world.state.notes = outcome.notes.clone();
    world.state.changes = outcome.changes.clone();
    outcome
}

// ---------------------------------------------------------------------
// 記帳落檔：worlds/<world_id>/mechanism-log.jsonl
// ---------------------------------------------------------------------

/// 每筆記錄落一行 JSON；寫檔失敗一律吞掉，記帳設施不該反過來中斷遊戲。
pub fn append_log(root: &Path, world_id: &str, scene: u64, records: &[Record]) {
    if records.is_empty() {
        return;
    }
    let Ok(path) = data::mechanism_log_path(root, world_id) else {
        return;
    };
    let ts = data::local_timestamp_seconds().unwrap_or_else(|_| "unknown-time".to_owned());
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    else {
        return;
    };
    use std::io::Write;
    for record in records {
        let line = serde_json::json!({
            "ts": ts,
            "scene": scene,
            "kind": record.kind,
            "path": record.path,
            "detail": record.detail,
        });
        if let Ok(text) = serde_json::to_string(&line) {
            let _ = writeln!(file, "{text}");
        }
    }
}

// ---------------------------------------------------------------------
// 帳本讀取：世界書分頁「機制帳本」面板用，彙總 mechanism-log.jsonl
// ---------------------------------------------------------------------

/// 帳本一列：一條可切換開關的機制條目（接管或跳過）。`uid` 供面板呼叫既有
/// `upsert_worldbook_entry` 切換 `disabled` 用；`sent` 是目前是否照原文送模型。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub uid: u64,
    pub title: String,
    pub kind: RecordKind,
    pub detail: String,
    pub sent: bool,
}

/// 世界書分頁「機制帳本」面板用：對得上目前世界書條目的接管／跳過清單，
/// 加上另外四類記帳（拒收／夾邊界／格式錯誤／跳動）的次數。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Ledger {
    pub entries: Vec<LedgerEntry>,
    pub rejected: usize,
    pub clamped: usize,
    pub errors: usize,
    pub jumps: usize,
}

fn ledger_rank(kind: RecordKind) -> u8 {
    match kind {
        RecordKind::Absorbed => 0,
        RecordKind::Skipped => 1,
        _ => 2,
    }
}

/// 讀 `mechanism-log.jsonl` 彙總成面板用帳本。容錯是紅線：檔案不存在、讀不到、
/// 壞行一律跳過，絕不 panic。`Absorbed`／`Skipped` 以 `path`（條目標題，trim 後）為 key
/// 去重，同一條目重複記帳只留最新那筆；再拿目前世界書比對標題，對不上的（例如一次性
/// 事件旗標這類不是條目的記錄）沒有開關可切，不列進 `entries`。其餘四種只累計次數。
pub fn read_ledger(root: &Path, world_id: &str) -> Ledger {
    let mut ledger = Ledger::default();
    let Ok(path) = data::mechanism_log_path(root, world_id) else {
        return ledger;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return ledger;
    };

    let mut absorbed_or_skipped: BTreeMap<String, Record> = BTreeMap::new();
    for line in text.lines() {
        let Ok(record) = serde_json::from_str::<Record>(line) else {
            continue;
        };
        match record.kind {
            RecordKind::Absorbed | RecordKind::Skipped => {
                absorbed_or_skipped.insert(record.path.trim().to_owned(), record);
            }
            RecordKind::Rejected => ledger.rejected += 1,
            RecordKind::Clamped => ledger.clamped += 1,
            RecordKind::Error => ledger.errors += 1,
            RecordKind::Jump => ledger.jumps += 1,
        }
    }
    if absorbed_or_skipped.is_empty() {
        return ledger;
    }

    let worldbook = data::read_worldbook(root, world_id).unwrap_or_default();
    let mut entries: Vec<LedgerEntry> = worldbook
        .iter()
        .filter_map(|entry| {
            let record = absorbed_or_skipped.get(entry.title.trim())?;
            Some(LedgerEntry {
                uid: entry.uid,
                title: entry.title.clone(),
                kind: record.kind,
                detail: record.detail.clone(),
                sent: !entry.disabled,
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        ledger_rank(a.kind)
            .cmp(&ledger_rank(b.kind))
            .then_with(|| a.title.cmp(&b.title))
    });
    ledger.entries = entries;
    ledger
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{self, Mechanism, StateNode};
    use crate::mechanism::test_support::{mechanism_with, once_mechanism, rule, world_with};
    use crate::mechanism::tree::leaf_at;
    use std::path::PathBuf;

    // ---- apply_block：平欄／樹欄套用＋增量本地權威一次跑完 ----

    #[test]
    fn apply_block_merges_fields_applies_updates_and_records_notes_onto_state() {
        let mechanism =
            mechanism_with(&[("World.HP", rule(FieldKind::Number, Some(0.0), Some(100.0)))]);
        let mut world = world_with(&[("World.HP", "80")], mechanism);
        let block = crate::transport::StateBlock {
            fields: vec![(
                vec!["World".to_owned(), "Location".to_owned()],
                "晨港".to_owned(),
            )],
            updates: vec![
                r#"<JSONPatch>[{"op":"replace","path":"/World/HP","value":999}]</JSONPatch>"#
                    .to_owned(),
            ],
            display: String::new(),
        };

        let outcome = apply_block(&mut world, &block, "阿濤");

        assert_eq!(outcome.records.len(), 1);
        assert_eq!(outcome.records[0].kind, RecordKind::Rejected);
        assert!(!world.state.notes.is_empty());
        assert_eq!(world.state.notes, outcome.notes);

        let StateNode::Branch(world_branch) = world.state.tree.get("World").unwrap() else {
            panic!("World 應該是分支");
        };
        // 絕對值被拒收，本地帳沿用舊值；平欄套用的 Location 正常寫入。
        assert_eq!(
            world_branch.get("HP"),
            Some(&StateNode::Leaf("80".to_owned()))
        );
        assert_eq!(
            world_branch.get("Location"),
            Some(&StateNode::Leaf("晨港".to_owned()))
        );
    }

    // ---- append_log：記帳落檔 ----

    #[test]
    fn append_log_writes_one_json_line_per_record_with_all_fields() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-mechanism-log-{}",
            ulid::Ulid::generate()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();

        let records = vec![
            Record::new(
                RecordKind::Rejected,
                "World.HP".to_owned(),
                "World.HP 現值 80，請用增減量（delta）而不是絕對值。".to_owned(),
            ),
            Record::new(
                RecordKind::Absorbed,
                "[mvu_update] 規則".to_owned(),
                "機制鷹架條目，已由本地機制接管，不再送入提示詞。".to_owned(),
            ),
        ];
        append_log(&root, &world_id, 3, &records);

        let text =
            std::fs::read_to_string(data::mechanism_log_path(&root, &world_id).unwrap()).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        for (line, record) in lines.iter().zip(&records) {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(value
                .get("ts")
                .and_then(serde_json::Value::as_str)
                .is_some());
            assert_eq!(value["scene"].as_u64(), Some(3));
            assert_eq!(value["path"].as_str(), Some(record.path.as_str()));
            assert_eq!(value["detail"].as_str(), Some(record.detail.as_str()));
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    // ---- read_ledger：帳本讀取彙總 ----

    fn ledger_test_world(name: &str) -> (PathBuf, String) {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-mechanism-ledger-{name}-{}",
            ulid::Ulid::generate()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();
        (root, world_id)
    }

    #[test]
    fn read_ledger_matches_titles_counts_rejected_and_skips_broken_lines() {
        let (root, world_id) = ledger_test_world("basic");
        data::upsert_worldbook_entry(
            &root,
            &world_id,
            data::WorldbookEntry {
                uid: u64::MAX,
                title: "宝物栏初始化".to_owned(),
                keys: Vec::new(),
                content: String::new(),
                constant: false,
                order: 0,
                disabled: true,
                visibility: data::Visibility::Gm,
                is_person: false,
                locked: false,
            },
        )
        .unwrap();
        data::upsert_worldbook_entry(
            &root,
            &world_id,
            data::WorldbookEntry {
                uid: u64::MAX,
                title: "随机事件表".to_owned(),
                keys: Vec::new(),
                content: String::new(),
                constant: false,
                order: 0,
                disabled: false,
                visibility: data::Visibility::Gm,
                is_person: false,
                locked: false,
            },
        )
        .unwrap();

        append_log(
            &root,
            &world_id,
            1,
            &[
                Record::new(
                    RecordKind::Absorbed,
                    "宝物栏初始化".to_owned(),
                    "機制鷹架條目，已由本地機制接管。".to_owned(),
                ),
                Record::new(
                    RecordKind::Skipped,
                    "随机事件表".to_owned(),
                    "卡片腳本認不出來，預設不送模型。".to_owned(),
                ),
                Record::new(
                    RecordKind::Rejected,
                    "World.HP".to_owned(),
                    "拒收".to_owned(),
                ),
            ],
        );
        // 壞行：非 JSON，讀檔時要跳過而不影響其餘行。
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(data::mechanism_log_path(&root, &world_id).unwrap())
            .unwrap();
        writeln!(file, "這不是 JSON").unwrap();

        let ledger = read_ledger(&root, &world_id);
        assert_eq!(ledger.entries.len(), 2);
        assert_eq!(ledger.entries[0].title, "宝物栏初始化");
        assert_eq!(ledger.entries[0].kind, RecordKind::Absorbed);
        assert!(!ledger.entries[0].sent); // disabled=true → 不送模型
        assert_eq!(ledger.entries[1].title, "随机事件表");
        assert_eq!(ledger.entries[1].kind, RecordKind::Skipped);
        assert!(ledger.entries[1].sent); // disabled=false → 照原文送
        assert_eq!(ledger.rejected, 1);
        assert_eq!(ledger.clamped, 0);
        assert_eq!(ledger.errors, 0);
        assert_eq!(ledger.jumps, 0);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn read_ledger_keeps_only_the_latest_record_for_a_repeated_entry() {
        let (root, world_id) = ledger_test_world("dedupe");
        data::upsert_worldbook_entry(
            &root,
            &world_id,
            data::WorldbookEntry {
                uid: u64::MAX,
                title: "机制条目A".to_owned(),
                keys: Vec::new(),
                content: String::new(),
                constant: false,
                order: 0,
                disabled: true,
                visibility: data::Visibility::Gm,
                is_person: false,
                locked: false,
            },
        )
        .unwrap();

        // 同一條目重複匯入會記兩筆帳，後面那筆才是最新狀態。
        append_log(
            &root,
            &world_id,
            1,
            &[Record::new(
                RecordKind::Absorbed,
                "机制条目A".to_owned(),
                "第一次匯入".to_owned(),
            )],
        );
        append_log(
            &root,
            &world_id,
            2,
            &[Record::new(
                RecordKind::Absorbed,
                "机制条目A".to_owned(),
                "第二次匯入".to_owned(),
            )],
        );

        let ledger = read_ledger(&root, &world_id);
        assert_eq!(ledger.entries.len(), 1);
        assert_eq!(ledger.entries[0].detail, "第二次匯入");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn read_ledger_returns_empty_when_log_file_is_missing() {
        // 不建世界目錄：mechanism_log_path 組得出路徑，但檔案不存在。
        let root = std::env::temp_dir().join(format!(
            "table-tavern-mechanism-ledger-missing-{}",
            ulid::Ulid::generate()
        ));
        let world_id = data::new_id();
        let ledger = read_ledger(&root, &world_id);
        assert_eq!(ledger, Ledger::default());
    }

    /// 一次性事件全流程：第一次求值命中→文本有了、旗標被釘成 true、記一筆 Absorbed；
    /// 同一棵樹（旗標已釘）再求值一次→不再命中，模型翻不了案。
    #[test]
    fn once_event_pins_the_flag_and_never_fires_again_on_the_same_tree() {
        let mechanism = once_mechanism();
        let mut world = world_with(&[("World.Invasion", "90")], mechanism);
        let block = crate::transport::StateBlock {
            fields: Vec::new(),
            updates: Vec::new(),
            display: String::new(),
        };

        let outcome = apply_block(&mut world, &block, "阿濤");
        assert_eq!(
            world.state.triggers.get("國變"),
            Some(&"國都淪陷。".to_owned())
        );
        assert_eq!(leaf_at(&world.state.tree, "Events.國變"), Some("true"));
        assert!(outcome
            .records
            .iter()
            .any(|record| record.kind == RecordKind::Absorbed && record.path == "Events.國變"));

        let second = apply_block(&mut world, &block, "阿濤");
        assert!(!world.state.triggers.contains_key("國變"));
        assert!(!second
            .records
            .iter()
            .any(|record| record.kind == RecordKind::Absorbed));
    }

    /// 全量桌（`!mechanism.incremental`）逐字維持現狀：不做觸發表求值。
    #[test]
    fn apply_block_skips_trigger_evaluation_for_a_full_snapshot_table() {
        let mut mechanism = once_mechanism();
        mechanism.incremental = false;
        let mut world = world_with(&[("World.Invasion", "90")], mechanism);
        let block = crate::transport::StateBlock {
            fields: Vec::new(),
            updates: Vec::new(),
            display: String::new(),
        };
        apply_block(&mut world, &block, "阿濤");
        assert!(world.state.triggers.is_empty());
        assert!(leaf_at(&world.state.tree, "Events.國變").is_none());
    }

    // ---- 全量桌跳動標記（狀態欄二期包 6）----

    #[test]
    fn full_snapshot_jump_over_threshold_is_marked_and_recorded() {
        let mechanism = mechanism_with(&[]);
        let mut world = world_with(&[("World.HP", "60")], mechanism);
        let block = crate::transport::StateBlock {
            fields: vec![(vec!["World".to_owned(), "HP".to_owned()], "100".to_owned())],
            updates: Vec::new(),
            display: String::new(),
        };
        let outcome = apply_block(&mut world, &block, "阿濤");
        assert_eq!(world.state.jumps.get("World.HP"), Some(&"+40".to_owned()));
        assert!(outcome
            .records
            .iter()
            .any(|record| record.kind == RecordKind::Jump && record.path == "World.HP"));
    }

    /// 幅度沒過絕對門檻（3→10 只差 7）：不標，不管相對幅度多誇張。
    #[test]
    fn full_snapshot_small_change_under_absolute_threshold_is_not_marked() {
        let mechanism = mechanism_with(&[]);
        let mut world = world_with(&[("World.HP", "3")], mechanism);
        let block = crate::transport::StateBlock {
            fields: vec![(vec!["World".to_owned(), "HP".to_owned()], "10".to_owned())],
            updates: Vec::new(),
            display: String::new(),
        };
        apply_block(&mut world, &block, "阿濤");
        assert!(world.state.jumps.is_empty());
    }

    /// 已標成計數器的欄位（例如卡片自己的「第 N 天」）就算幅度誇張也不標，
    /// 這是玩家點記號之後的效果，時間跳躍是那張卡的明文功能。
    #[test]
    fn full_snapshot_counter_field_is_never_marked_even_with_a_huge_jump() {
        let mechanism = mechanism_with(&[("World.Day", rule(FieldKind::Counter, None, None))]);
        let mut world = world_with(&[("World.Day", "3")], mechanism);
        let block = crate::transport::StateBlock {
            fields: vec![(vec!["World".to_owned(), "Day".to_owned()], "100".to_owned())],
            updates: Vec::new(),
            display: String::new(),
        };
        apply_block(&mut world, &block, "阿濤");
        assert!(world.state.jumps.is_empty());
    }

    /// 增量桌（本地算術權威，模型只回報變動量）一律不做跳動比對，`jumps` 維持空。
    #[test]
    fn incremental_table_never_populates_jumps() {
        let mechanism = Mechanism {
            incremental: true,
            ..Mechanism::default()
        };
        let mut world = world_with(&[("World.HP", "60")], mechanism);
        let block = crate::transport::StateBlock {
            fields: vec![(vec!["World".to_owned(), "HP".to_owned()], "100".to_owned())],
            updates: Vec::new(),
            display: String::new(),
        };
        apply_block(&mut world, &block, "阿濤");
        assert!(world.state.jumps.is_empty());
    }

    /// 真卡格式端到端：donass 的 `<StatusData>` 與 orc-cave 的 `<details>` 摺疊狀態欄，
    /// 從剝殼一路走到套用——比對吃的是模型真的會吐的字串（全形冒號、「第 N 天」這種前後綴、
    /// 中文數字的純文字欄），不是理想化的純數字。
    #[test]
    fn full_snapshot_jump_reads_real_card_state_blocks() {
        let mut world = world_with(&[], mechanism_with(&[]));
        let opening = "陆辰咬牙。\n<StatusData>\n体力:60\n好感:20\n层数:第一层\n</StatusData>";
        apply_block(
            &mut world,
            &crate::transport::extract_state_block(opening),
            "阿濤",
        );
        assert!(world.state.jumps.is_empty());

        let next = "他愣了一下。\n<StatusData>\n体力:55\n好感:70\n层数:第一层\n</StatusData>";
        apply_block(
            &mut world,
            &crate::transport::extract_state_block(next),
            "阿濤",
        );
        // 好感一輪跳 50 標出來；体力只掉 5、层数是中文數字抽不出數，兩個都不標
        assert_eq!(world.state.jumps.get("好感"), Some(&"+50".to_owned()));
        assert_eq!(world.state.jumps.len(), 1);

        let day = |n: u32| {
            format!(
                "……\n<details>\n<summary>状态栏</summary>\n<hr>\n\n- 沦陷天数：第 {n} 天\n- 当前环境：洞穴深处\n\n</details>"
            )
        };
        let mut cave = world_with(&[], mechanism_with(&[]));
        apply_block(
            &mut cave,
            &crate::transport::extract_state_block(&day(1)),
            "阿濤",
        );
        apply_block(
            &mut cave,
            &crate::transport::extract_state_block(&day(10)),
            "阿濤",
        );
        assert!(cave.state.jumps.is_empty(), "尋常推進幾天不該示警");

        apply_block(
            &mut cave,
            &crate::transport::extract_state_block(&day(60)),
            "阿濤",
        );
        assert_eq!(
            cave.state.jumps.get("沦陷天数"),
            Some(&"+50".to_owned()),
            "一口氣跳 50 天先示警，玩家點記號標成計數器之後才不再提醒"
        );
    }
}
