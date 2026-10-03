//! 逐字稿的單行讀改：卡寫、改初始化來源、回合冪等查找只碰一則事件，其他行只做輕量解析、原樣拼回，
//! 不必把整幕（每則都帶狀態快照與變數表）展開成物件（計畫 8.1 效能閘）。
use super::{transcript_path, TranscriptEvent};
use crate::data::message_vars::{TurnKey, VarsTable};
use crate::data::state_commit::CommitTx;
use crate::data::world_file::with_file_lock;
use crate::data::{invalid_data, DataResult};
use serde::Deserialize;
use std::path::Path;

/// 一行只解析這幾欄；其餘欄位跳過不建物件。
#[derive(Debug, Deserialize)]
pub(crate) struct LineHead {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub message_vars: Option<VarsTable>,
    #[serde(default)]
    pub vars_rev: Option<String>,
    #[serde(default)]
    pub vars_epoch: Option<String>,
    #[serde(default)]
    pub turn_key: Option<TurnKey>,
}

/// 每一行的位元組範圍（不含換行）。最後一段沒有換行＝追加到一半的殘段候選。
fn spans(bytes: &[u8]) -> (Vec<(usize, usize)>, bool) {
    let mut spans = Vec::new();
    let mut start = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            spans.push((start, index));
            start = index + 1;
        }
    }
    let unterminated = start < bytes.len();
    if unterminated {
        spans.push((start, bytes.len()));
    }
    (spans, unterminated)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    let Some(&first) = needle.first() else {
        return true;
    };
    let mut from = 0;
    while let Some(offset) = haystack[from..].iter().position(|&byte| byte == first) {
        let at = from + offset;
        if haystack[at..].starts_with(needle) {
            return true;
        }
        from = at + 1;
    }
    false
}

/// 從檔尾往前逐行輕量解析，`visit` 回 true 就停在那一行，回傳它的位置與範圍。`needle`（id、版本、epoch、
/// turn_id 的字面）有給時，不含它的行直接跳過不解析。壞行規則同 `parse_transcript`：最後一行沒換行又解析
/// 失敗就略過，其他（有解析到的）壞行回錯。
pub(crate) fn find_rev(
    bytes: &[u8],
    needle: Option<&str>,
    mut visit: impl FnMut(usize, LineHead) -> bool,
) -> DataResult<Option<(usize, (usize, usize))>> {
    let (spans, unterminated) = spans(bytes);
    let last = spans.len().saturating_sub(1);
    for (index, &(start, end)) in spans.iter().enumerate().rev() {
        let line = &bytes[start..end];
        if needle.is_some_and(|needle| !contains(line, needle.as_bytes())) {
            continue;
        }
        let head: LineHead = match serde_json::from_slice(line) {
            Ok(head) => head,
            Err(_) if unterminated && index == last => continue,
            Err(error) => {
                return Err(invalid_data(format!(
                    "invalid transcript line {}: {error}",
                    index + 1
                )))
            }
        };
        if visit(index, head) {
            return Ok(Some((index, (start, end))));
        }
    }
    Ok(None)
}

fn read_scene(root: &Path, world_id: &str, scene: u64) -> DataResult<Option<Vec<u8>>> {
    with_file_lock(&transcript_path(root, world_id, scene)?, |file| file.read())
}

/// 鎖外唯讀查找：回命中那一行的完整事件。
pub(crate) fn find_event_rev(
    root: &Path,
    world_id: &str,
    scene: u64,
    needle: Option<&str>,
    mut pick: impl FnMut(usize, &LineHead) -> bool,
) -> DataResult<Option<(usize, TranscriptEvent)>> {
    let Some(bytes) = read_scene(root, world_id, scene)? else {
        return Ok(None);
    };
    match find_rev(&bytes, needle, |index, head| pick(index, &head))? {
        Some((index, (start, end))) => {
            Ok(Some((index, serde_json::from_slice(&bytes[start..end])?)))
        }
        None => Ok(None),
    }
}

/// 同檔鎖內改一則：`pick` 從檔尾往前選中一行，`edit` 拿到那則的完整事件，回 `Some(新事件)` 就把那一行
/// 換掉、其他行原樣拼回整檔寫回。沒選中回 None。
pub(crate) fn edit_line<T>(
    tx: &CommitTx<'_>,
    scene: u64,
    needle: Option<&str>,
    mut pick: impl FnMut(usize, &LineHead) -> bool,
    edit: impl FnOnce(usize, TranscriptEvent) -> DataResult<(T, Option<TranscriptEvent>)>,
) -> DataResult<Option<T>> {
    let path = transcript_path(tx.root, tx.world_id, scene)?;
    with_file_lock(&path, |file| {
        let Some(bytes) = file.read()? else {
            return Ok(None);
        };
        let Some((index, (start, end))) =
            find_rev(&bytes, needle, |index, head| pick(index, &head))?
        else {
            return Ok(None);
        };
        let event: TranscriptEvent = serde_json::from_slice(&bytes[start..end])?;
        let (result, replaced) = edit(index, event)?;
        if let Some(replaced) = replaced {
            let line = serde_json::to_vec(&replaced)?;
            let mut out = Vec::with_capacity(bytes.len() + line.len());
            out.extend_from_slice(&bytes[..start]);
            out.extend_from_slice(&line);
            if end < bytes.len() {
                out.extend_from_slice(&bytes[end..]);
            } else {
                out.push(b'\n');
            }
            file.write_atomic(&out)?;
        }
        Ok(Some(result))
    })
}
