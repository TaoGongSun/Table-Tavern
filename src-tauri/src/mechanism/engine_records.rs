//! zod 卡引擎路徑的記帳（mvu-zod-semantics 規格 2.4、6.1 節）：引擎逐條結果與共用核心給的回饋句映射成 `Record`，
//! 上限沿用 `build_notes`；套用後的本地規則（重擲、觸發、衍生）與一般路徑共用 `local_rules`。
use super::apply::signed_delta_mark;
use super::ledger::{local_rules, REJECTED_BLOCK_PATH};
use super::note::build_notes;
use super::types::{Outcome, Record, RecordKind};
use crate::data;
use crate::mvu_engine::{Change, CommandResult};
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

/// 逐條結果 → 帳本：句子由共用核心出（`zod-feedback.ts`，網頁同一份，經引擎回傳的 `feedback` 帶過來），這裡只映射成
/// Rejected 記錄；None（applied、不回饋的 ignored）不記。
pub fn engine_records(results: &[CommandResult], feedback: &[Option<String>]) -> Vec<Record> {
    results
        .iter()
        .zip(feedback)
        .filter_map(|(result, line)| {
            let line = line.as_ref()?;
            Some(Record::new(
                RecordKind::Rejected,
                path_text(&result.path),
                line.clone(),
            ))
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
    use serde_json::Value;

    /// 與網頁同一份契約（src/shared/contracts/mvu-zod-feedback.json）：共用核心給的句子映射成記錄後，
    /// build_notes 的去重、5 條、240 字上限得出同一份 notes
    #[test]
    fn notes_match_the_shared_feedback_contract() {
        let contract: Value = serde_json::from_str(include_str!(
            "../../../src/shared/contracts/mvu-zod-feedback.json"
        ))
        .unwrap();
        for case in contract["cases"].as_array().unwrap() {
            let results: Vec<CommandResult> =
                serde_json::from_value(case["results"].clone()).unwrap();
            let lines: Vec<Option<String>> = serde_json::from_value(case["lines"].clone()).unwrap();
            let notes: Vec<String> = serde_json::from_value(case["notes"].clone()).unwrap();
            let records = engine_records(&results, &lines);
            assert!(records
                .iter()
                .all(|record| record.kind == RecordKind::Rejected));
            assert_eq!(build_notes(&records), notes, "{}", case["name"]);
        }
        assert_eq!(path_text(&["世界".into(), "a.b".into()]), "世界[\"a.b\"]");
    }
}
