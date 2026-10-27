//! zod 卡引擎路徑的記帳（mvu-zod-semantics 規格 2.4 節）：引擎逐條結果映射成 `Record`，回饋句沿用
//! `build_notes`；套用後的本地規則（重擲、觸發、衍生）與一般路徑共用 `local_rules`。
use super::apply::signed_delta_mark;
use super::ledger::{local_rules, REJECTED_BLOCK_PATH};
use super::note::build_notes;
use super::types::{Outcome, Record, RecordKind};
use crate::data;
use crate::mvu_engine::{Change, CommandResult, CommandStatus};
use std::collections::BTreeMap;

/// 回饋模型的路徑：分段照原樣接（鍵名含點的段用 `["…"]` 包起來，同核心的錯誤文字）
fn path_text(segments: &[String]) -> String {
    let mut text = String::new();
    for (index, segment) in segments.iter().enumerate() {
        let plain = !segment.is_empty() && !segment.contains(['.', '[', ']', '"', '\'', '\\']);
        match (plain, index) {
            (true, 0) => text.push_str(segment),
            (true, _) => {
                text.push('.');
                text.push_str(segment);
            }
            (false, _) => {
                text.push('[');
                text.push_str(&serde_json::to_string(segment).unwrap_or_default());
                text.push(']');
            }
        }
    }
    text
}

/// 逐條結果 → 帳本。rejected 與 no-change 都回饋模型（no-change 是過了 schema 卻沒寫進去，免得模型以為已寫入）；
/// applied 不記；ignored（唯讀 `_` 路徑等，上游靜默丟掉）的回饋留給 P6。
pub fn engine_records(results: &[CommandResult]) -> Vec<Record> {
    results
        .iter()
        .filter_map(|result| {
            let path = path_text(&result.path);
            let detail = match result.status {
                CommandStatus::Applied | CommandStatus::Ignored => return None,
                CommandStatus::NoChange => format!(
                    "{path} 的 {} 通過了卡片 schema，但值沒有變（schema 沒列的鍵會被剝掉、被刪的會補回預設值），沒有寫入。",
                    result.op
                ),
                CommandStatus::Rejected => {
                    let issues = result.issues.as_deref().unwrap_or_default().join("；");
                    let more = match result.issue_count {
                        Some(count) if count > result.issues.as_ref().map_or(0, Vec::len) => {
                            format!("（共 {count} 則）")
                        }
                        _ => String::new(),
                    };
                    format!(
                        "{path} 的 {} 被卡片 schema 拒收、維持原值：{issues}{more}",
                        result.op
                    )
                }
            };
            Some(Record::new(RecordKind::Rejected, path, detail))
        })
        .collect()
}

/// 引擎路徑不套本 app 的狀態區塊欄位與清單（上游只認指令）：有寫就記一筆，讓模型改用指令。
pub fn state_block_ignored(block: &crate::transport::StateBlock) -> Option<Record> {
    (!block.fields.is_empty() || !block.lists.is_empty()).then(|| {
        Record::new(
            RecordKind::Rejected,
            REJECTED_BLOCK_PATH.to_owned(),
            "狀態區塊未套用，請以 UpdateVariable 回報變動。".to_owned(),
        )
    })
}

/// 給玩家看、不回饋模型的一筆（引擎整次失敗、開局驗不過、後處理退回、第二張卡的 schema 沒執行）
pub fn engine_notice(path: &str, detail: String) -> Record {
    Record::new(RecordKind::Error, path.to_owned(), detail)
}

/// 引擎的逐葉變動 → 回合尾「哪裡變了」的標記（同一般路徑：數字標帶號增量、刪掉標移除、其餘標更新）
pub fn engine_changes(changes: &[Change]) -> BTreeMap<String, String> {
    changes
        .iter()
        .map(|change| {
            let mark = match (&change.before, &change.after) {
                (_, None) if !change.truncated => "移除".to_owned(),
                (Some(before), Some(after)) => match (before.as_f64(), after.as_f64()) {
                    (Some(from), Some(to)) => signed_delta_mark(to - from),
                    _ => "更新".to_owned(),
                },
                _ => "更新".to_owned(),
            };
            (change.path.join("."), mark)
        })
        .collect()
}

/// 收尾：`local_rules` 為真就跑重擲、觸發、衍生；照記錄重建回饋，寫回 `world.state` 的回饋與變動。
pub fn finish_engine(
    world: &mut data::WorldState,
    user_name: &str,
    mut records: Vec<Record>,
    changes: BTreeMap<String, String>,
    run_local_rules: bool,
) -> Outcome {
    if run_local_rules {
        local_rules(world, user_name, &mut records);
    }
    let notes = build_notes(&records);
    world.state.notes = notes.clone();
    world.state.changes = changes.clone();
    Outcome {
        records,
        notes,
        changes,
        typed: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(
        status: CommandStatus,
        path: &[&str],
        issues: Option<Vec<&str>>,
        count: Option<usize>,
    ) -> CommandResult {
        CommandResult {
            index: 0,
            op: "set".into(),
            path: path.iter().map(|segment| segment.to_string()).collect(),
            to: None,
            status,
            reason: None,
            issues: issues.map(|items| items.into_iter().map(str::to_owned).collect()),
            issue_count: count,
        }
    }

    #[test]
    fn rejected_and_unchanged_feed_back_applied_and_ignored_do_not() {
        let records = engine_records(&[
            result(CommandStatus::Applied, &["a"], None, None),
            result(
                CommandStatus::Rejected,
                &["世界", "a.b"],
                Some(vec!["✖ 錯一", "✖ 錯二"]),
                Some(7),
            ),
            result(CommandStatus::NoChange, &["隊員", "甲"], None, None),
            result(CommandStatus::Ignored, &["_秘密"], None, None),
        ]);
        assert_eq!(records.len(), 2);
        assert!(records
            .iter()
            .all(|record| record.kind == RecordKind::Rejected));
        assert_eq!(records[0].path, "世界[\"a.b\"]");
        assert!(
            records[0].detail.contains("✖ 錯一；✖ 錯二（共 7 則）"),
            "{}",
            records[0].detail
        );
        assert!(records[1].detail.contains("沒有寫入"));
        assert_eq!(build_notes(&records).len(), 2);
    }
}
