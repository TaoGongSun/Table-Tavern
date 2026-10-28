//! 角色卡自動隱藏跟著退回／分岔回溯（revert-fork-auto-hidden）。清單見 .ai/plans/revert-fork-auto-hidden.md。

use std::collections::BTreeMap;
use std::path::Path;

use crate::data::test_support::*;
use crate::data::*;

struct Table {
    root: TestRoot,
    world: String,
    ids: BTreeMap<String, String>,
}

impl Table {
    fn new(label: &str, names: &[&str]) -> Self {
        let root = TestRoot::new(label);
        let world = create_world(root.path(), "桌").unwrap();
        let mut table = Self {
            root,
            world,
            ids: BTreeMap::new(),
        };
        for name in names {
            table.add_card(name);
        }
        table
    }

    fn root(&self) -> &Path {
        self.root.path()
    }

    fn add_card(&mut self, name: &str) {
        let card = character_card(&new_id(), name);
        write_character(self.root(), &self.world, &card).unwrap();
        self.ids.insert(name.to_owned(), card.id);
    }

    fn id(&self, name: &str) -> &str {
        &self.ids[name]
    }

    fn set_hidden(&self, name: &str, hidden: bool) {
        set_character_auto_hidden(self.root(), &self.world, self.id(name), hidden).unwrap();
    }

    /// 名字 → auto_hidden（只列解析得到的卡）
    fn hidden(&self) -> BTreeMap<String, bool> {
        list_characters(self.root(), &self.world)
            .unwrap()
            .into_iter()
            .map(|meta| (meta.name, meta.auto_hidden))
            .collect()
    }

    fn is_hidden(&self, name: &str) -> bool {
        self.hidden()[name]
    }

    fn state(&self) -> WorldState {
        read_state(self.root(), &self.world).unwrap()
    }

    fn scene(&self) -> u64 {
        self.state().current_scene
    }

    fn say(&self) {
        let scene = self.scene();
        append_transcript(self.root(), &self.world, scene, &event("一句話")).unwrap();
    }

    /// 帶著 present 換幕（None＝不寫 present）
    fn change(&self, present: Option<&str>) {
        self.try_change(present).unwrap();
    }

    fn try_change(&self, present: Option<&str>) -> DataResult<u64> {
        let mut state = self.state();
        match present {
            Some(present) => {
                state
                    .state
                    .table
                    .insert("present".to_owned(), present.to_owned());
            }
            None => {
                state.state.table.remove("present");
            }
        }
        write_state(self.root(), &self.world, &state).unwrap();
        begin_next_scene(self.root(), &self.world, "摘要", None)
    }

    fn revert(&self) -> DataResult<u64> {
        revert_scene(self.root(), &self.world)
    }

    fn fork(&self, from: u64) -> DataResult<u64> {
        fork_scene(self.root(), &self.world, from)
    }

    fn snapshot(&self, scene: u64) -> Option<BTreeMap<String, bool>> {
        let by_id = self
            .state()
            .scene_card_visibility
            .get(&scene.to_string())?
            .clone();
        Some(
            by_id
                .into_iter()
                .map(|(id, hidden)| {
                    let name = self
                        .ids
                        .iter()
                        .find(|(_, card)| **card == id)
                        .map(|(name, _)| name.clone())
                        .unwrap_or(id);
                    (name, hidden)
                })
                .collect(),
        )
    }
}

fn event(text: &str) -> TranscriptEvent {
    TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        ts: "now".to_owned(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind: TranscriptKind::Narration,
        text: text.to_owned(),
        raw: None,
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    }
}

fn map(pairs: &[(&str, bool)]) -> BTreeMap<String, bool> {
    pairs
        .iter()
        .map(|(name, hidden)| ((*name).to_owned(), *hidden))
        .collect()
}

/// 1. 退回雙向還原（重現一）：結算收起的回到顯示、結算放出的回到收起
#[test]
fn revert_restores_both_directions() {
    let table = Table::new("vis-revert-both", &["甲", "乙", "丙"]);
    table.set_hidden("丙", true);
    table.say();
    table.change(Some("甲、丙"));
    assert_eq!(
        table.hidden(),
        map(&[("甲", false), ("乙", true), ("丙", false)])
    );
    assert_eq!(table.revert().unwrap(), 0);
    assert_eq!(
        table.hidden(),
        map(&[("甲", false), ("乙", false), ("丙", true)])
    );
    assert!(table.snapshot(1).is_none(), "退掉的子幕不留 key");
}

/// 1／7. 分岔還原（重現二）：全員可見的幕，仍能把後來被收的卡寫回 false
#[test]
fn fork_restores_all_visible_source_scene() {
    let table = Table::new("vis-fork-all-visible", &["甲", "乙"]);
    table.say();
    table.change(Some("甲、乙"));
    assert_eq!(
        table.snapshot(0),
        Some(map(&[("甲", false), ("乙", false)]))
    );
    table.say();
    table.change(Some("甲"));
    assert!(table.is_hidden("乙"));
    assert_eq!(table.fork(0).unwrap(), 3);
    assert_eq!(table.hidden(), map(&[("甲", false), ("乙", false)]));
    assert_eq!(table.snapshot(2), Some(map(&[("甲", false), ("乙", true)])));
}

/// 2. 幕內手動拉回保留；只在子幕拉回的被父幕快照蓋掉
#[test]
fn manual_pull_in_scene_kept_pull_in_child_overwritten() {
    let table = Table::new("vis-manual", &["甲", "乙", "丙"]);
    table.set_hidden("乙", true);
    table.set_hidden("丙", true);
    table.say();
    table.set_hidden("丙", false); // 幕 0 裡手動拉回
    table.change(Some("甲"));
    assert_eq!(
        table.hidden(),
        map(&[("甲", false), ("乙", true), ("丙", true)])
    );
    table.set_hidden("乙", false); // 只在子幕拉回
    table.revert().unwrap();
    assert_eq!(
        table.hidden(),
        map(&[("甲", false), ("乙", true), ("丙", false)])
    );
}

/// 3. 現在已封存的卡不動；5. 已刪的卡跳過
#[test]
fn archived_and_deleted_cards_are_skipped() {
    let table = Table::new("vis-archived-deleted", &["甲", "乙", "丙"]);
    table.say();
    table.change(Some("甲"));
    // 乙：子幕裡被封存（auto_hidden 仍 true）；丙：子幕裡被刪
    set_character_archived(table.root(), &table.world, table.id("乙"), true).unwrap();
    delete_character(table.root(), &table.world, table.id("丙")).unwrap();
    table.revert().unwrap();
    assert_eq!(table.hidden(), map(&[("甲", false), ("乙", true)]));
}

/// 4. 快照之後才建的卡，從早幕分岔後維持原值
#[test]
fn card_created_after_snapshot_keeps_its_value_on_fork() {
    let mut table = Table::new("vis-late-card", &["甲"]);
    table.say();
    table.change(Some("甲"));
    table.add_card("戊");
    table.say();
    table.change(Some("甲"));
    assert!(table.is_hidden("戊"));
    table.fork(0).unwrap();
    assert!(table.is_hidden("戊"), "快照外的卡不動");
}

/// 6. 沒有 key 不還原；零張卡的幕留空表、序列化後仍在，表外的卡照樣不動
#[test]
fn missing_key_noop_and_empty_snapshot_is_kept() {
    let mut table = Table::new("vis-empty", &[]);
    table.say();
    table.change(Some(""));
    let raw = std::fs::read_to_string(
        table
            .root()
            .join("worlds")
            .join(&table.world)
            .join("state.json"),
    )
    .unwrap();
    assert!(raw.contains("\"scene_card_visibility\""), "{raw}");
    assert_eq!(table.snapshot(0), Some(BTreeMap::new()));
    table.add_card("甲");
    table.set_hidden("甲", true);
    table.revert().unwrap();
    assert!(table.is_hidden("甲"), "空快照：表外的卡不動");

    // 沒有 key（舊桌）：不還原
    let table = Table::new("vis-no-key", &["甲", "乙"]);
    table.say();
    table.change(Some("甲"));
    let mut state = table.state();
    state.scene_card_visibility.clear();
    write_state(table.root(), &table.world, &state).unwrap();
    table.revert().unwrap();
    assert!(table.is_hidden("乙"));
}

/// 8. 不結算的桌（介面接管、沒有 present）也有快照，退回照快照寫回
#[test]
fn non_settling_table_still_snapshots() {
    let table = Table::new("vis-takeover", &["甲", "乙"]);
    write_interface_shell(table.root(), &table.world, "<UI>{{本回合.正文}}</UI>").unwrap();
    let mut state = table.state();
    state.refactor_mode = Some("interface".to_owned());
    write_state(table.root(), &table.world, &state).unwrap();
    table.set_hidden("乙", true);
    table.say();
    table.change(None);
    assert_eq!(
        table.hidden(),
        map(&[("甲", false), ("乙", true)]),
        "沒結算"
    );
    assert_eq!(table.snapshot(0), Some(map(&[("甲", false), ("乙", true)])));
    table.set_hidden("乙", false);
    table.set_hidden("甲", true);
    table.revert().unwrap();
    assert_eq!(table.hidden(), map(&[("甲", false), ("乙", true)]));
}

/// 9. 分岔再分岔：從分岔出來的幕再分岔，照那幕自己的快照
#[test]
fn fork_of_forked_scene() {
    let table = Table::new("vis-fork-fork", &["甲", "乙"]);
    table.say();
    table.change(Some("甲、乙")); // 幕 0：都顯示
    table.say();
    table.change(Some("甲")); // 幕 1 離開時：都顯示；結算收乙
    table.say();
    assert_eq!(table.fork(0).unwrap(), 3); // 幕 3＝幕 0 的分岔
    assert_eq!(table.hidden(), map(&[("甲", false), ("乙", false)]));
    table.set_hidden("甲", true);
    table.say();
    assert_eq!(table.fork(1).unwrap(), 4);
    assert_eq!(table.hidden(), map(&[("甲", false), ("乙", false)]));
    table.say();
    assert_eq!(table.fork(2).unwrap(), 5);
    assert_eq!(table.hidden(), map(&[("甲", false), ("乙", true)]));
    table.say();
    assert_eq!(table.fork(3).unwrap(), 6);
    assert_eq!(table.hidden(), map(&[("甲", true), ("乙", false)]));
}

/// 10. 分岔後立刻退回：恢復的是分岔當下所在的幕，不是來源幕
#[test]
fn revert_right_after_fork_returns_to_parent_not_source() {
    let table = Table::new("vis-fork-revert", &["甲", "乙"]);
    table.say(); // 幕 0 只有一則，分岔出來的幕才退得回去
    table.change(Some("甲、乙"));
    table.say();
    table.set_hidden("乙", true);
    assert_eq!(table.fork(0).unwrap(), 2);
    assert_eq!(table.hidden(), map(&[("甲", false), ("乙", false)]));
    assert_eq!(table.revert().unwrap(), 1);
    assert_eq!(table.hidden(), map(&[("甲", false), ("乙", true)]));
}

/// 11. 退回後修改再離開，覆寫父幕快照
#[test]
fn leaving_again_after_revert_overwrites_snapshot() {
    let table = Table::new("vis-overwrite", &["甲", "乙"]);
    table.say();
    table.change(Some("甲、乙"));
    table.revert().unwrap();
    table.set_hidden("甲", true);
    table.change(Some("甲、乙"));
    assert_eq!(table.snapshot(0), Some(map(&[("甲", true), ("乙", false)])));
    table.set_hidden("乙", true);
    table.revert().unwrap();
    assert_eq!(table.hidden(), map(&[("甲", true), ("乙", false)]));
}

/// 12／16. pending_leave 殘留時先退回、再重用那個幕號、然後離開：快照是新拍的
#[test]
fn reverting_clears_pending_and_reused_scene_snapshots_fresh() {
    let table = Table::new("vis-pending-revert", &["甲", "乙"]);
    table.say();
    table.change(Some("甲")); // 幕 1：乙被收
    {
        let _guard = WriteFailGuard::partial_ending(".md.tmp", 1);
        assert!(table.fork(0).is_err());
    }
    let state = table.state();
    assert_eq!(state.current_scene, 1);
    assert_eq!(state.pending_leave, Some(1));
    assert_eq!(table.snapshot(1), Some(map(&[("甲", false), ("乙", true)])));

    assert_eq!(table.revert().unwrap(), 0);
    assert_eq!(table.state().pending_leave, None);
    assert!(table.snapshot(1).is_none());
    table.change(Some("甲、乙")); // 重用幕號 1
    table.set_hidden("甲", true);
    table.say();
    table.change(Some("甲、乙"));
    assert_eq!(table.snapshot(1), Some(map(&[("甲", true), ("乙", false)])));
}

/// 16. 分岔失敗（改了一半）後改走一般換幕：沿用原快照、清掉 pending_leave
#[test]
fn change_after_failed_fork_reuses_snapshot() {
    let table = Table::new("vis-pending-change", &["甲", "乙"]);
    table.say();
    table.change(Some("甲、乙")); // 幕 0：都顯示
    table.set_hidden("甲", true);
    table.set_hidden("乙", true);
    table.say();
    {
        // 第二張卡寫失敗：第一張已改回 false
        let _guard = WriteFailGuard::partial_ending_after(".md.tmp", 1, 1);
        assert!(table.fork(0).is_err());
    }
    assert_eq!(
        table.hidden().values().filter(|hidden| !**hidden).count(),
        1
    );
    table.change(Some("甲、乙"));
    assert_eq!(table.state().pending_leave, None);
    assert_eq!(table.snapshot(1), Some(map(&[("甲", true), ("乙", true)])));
    table.revert().unwrap();
    assert_eq!(table.hidden(), map(&[("甲", true), ("乙", true)]));
}

/// 13. 退回時第二張卡寫失敗、或 state 寫失敗：場景不前進、卡檔完整、重試補完
#[test]
fn revert_failures_leave_scene_and_cards_intact_then_retry() {
    let table = Table::new("vis-revert-fail", &["甲", "乙"]);
    table.say();
    table.change(Some(""));
    assert_eq!(table.hidden(), map(&[("甲", true), ("乙", true)]));
    {
        let _guard = WriteFailGuard::partial_ending_after(".md.tmp", 1, 1);
        assert!(table.revert().is_err());
    }
    assert_eq!(table.scene(), 1);
    assert_eq!(table.hidden().len(), 2, "兩張卡都還解析得到");
    {
        let _guard = WriteFailGuard::partial_ending("state.json.tmp", 1);
        assert!(table.revert().is_err());
    }
    assert_eq!(table.scene(), 1);
    assert_eq!(table.revert().unwrap(), 0);
    assert_eq!(table.hidden(), map(&[("甲", false), ("乙", false)]));
}

/// 13／15. 拍快照讀卡失敗、state 讀失敗：換幕中止、不寫空表；修好後重試正常
#[cfg(unix)]
#[test]
fn snapshot_read_failure_aborts_leave() {
    use std::os::unix::fs::PermissionsExt;
    let table = Table::new("vis-read-fail", &["甲", "乙"]);
    table.say();
    let card = table
        .root()
        .join("worlds")
        .join(&table.world)
        .join("characters")
        .join(format!("{}.md", table.id("乙")));
    std::fs::set_permissions(&card, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_to_string(&card).is_ok() {
        return; // 以 root 執行時權限擋不住，這項測不了
    }
    assert!(table.try_change(Some("甲")).is_err());
    std::fs::set_permissions(&card, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(table.scene(), 0);
    assert!(table.snapshot(0).is_none(), "讀卡失敗不記成空表");
    assert_eq!(table.state().pending_leave, None);

    let state_file = table
        .root()
        .join("worlds")
        .join(&table.world)
        .join("state.json");
    let good = std::fs::read(&state_file).unwrap();
    std::fs::write(&state_file, b"{ broken").unwrap();
    assert!(begin_next_scene(table.root(), &table.world, "摘要", None).is_err());
    std::fs::write(&state_file, good).unwrap();

    table.change(Some("甲"));
    assert_eq!(
        table.snapshot(0),
        Some(map(&[("甲", false), ("乙", false)]))
    );
}

/// 14. 分岔時卡寫完、推進場景的 state 寫失敗 → 重試成功 → 退回 M：卡恢復成第一次操作前的值
#[test]
fn fork_retry_after_state_write_failure_keeps_leaving_snapshot() {
    let table = Table::new("vis-fork-retry", &["甲", "乙"]);
    table.say(); // 幕 0 只有一則（甲顯示）
    table.change(Some("甲、乙"));
    table.set_hidden("甲", true); // M＝幕 1：甲收起
    table.say();
    {
        // 放過第一段（記快照＋pending）那次，打在推進場景那次
        let _guard = WriteFailGuard::partial_ending_after("state.json.tmp", 1, 1);
        assert!(table.fork(0).is_err());
    }
    assert_eq!(table.scene(), 1);
    assert!(!table.is_hidden("甲"), "卡已照來源幕寫回");
    assert_eq!(table.fork(0).unwrap(), 2);
    assert_eq!(table.state().pending_leave, None);
    assert_eq!(table.revert().unwrap(), 1);
    assert_eq!(table.hidden(), map(&[("甲", true), ("乙", false)]));
}

/// 舞台卡（stage-card-in-speaker-list）：換幕被收進隱藏區、之後開舞台（清掉隱藏），退回到「它還藏著」
/// 的那幕時不把舊快照寫回；一般角色照常還原
#[test]
fn revert_does_not_rehide_stage_card() {
    let table = Table::new("vis-stage", &["甲", "要塞"]);
    table.say();
    table.change(Some("甲"));
    table.say();
    table.change(Some("甲"));
    assert!(table.is_hidden("要塞"), "還沒開舞台前，缺席照常被收");
    assert_eq!(table.snapshot(1).unwrap()["要塞"], true);
    set_character_stage(table.root(), &table.world, table.id("要塞"), true).unwrap();
    table.set_hidden("甲", true);
    assert_eq!(table.revert().unwrap(), 1);
    assert!(!table.is_hidden("要塞"), "舞台卡不照舊快照寫回隱藏");
    assert!(!table.is_hidden("甲"), "一般角色照快照還原");
}
