//! 重開補送（room-host-actions §3.3）：房主關房後在桌上單人玩了一段，重開時把桌上公開劇情跟房間故事串對齊。
//! 房間本幕故事串（`welcome` 快照＋重送佇列裡還沒落地的段）跟桌上同一幕的公開投影找最長共同前綴，
//! 之後的桌上段落當補送項；單人時換過幕就依序補「舊幕剩下的 → 舊幕摘要 → 下一幕 …」。
use super::{seed, Room};
use crate::data;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

/// 每片 ≤ 512 KiB（同 `room.seed`）
const PART_BYTES: usize = 480 * 1024;

/// 比對鍵：（說話者種類, 卡 id, 去頭尾空白的正文）。重送佇列裡沒有 begin 的段落只比正文
pub(super) type Key = (Option<(String, String)>, String);

fn key_of(speaker: &Value, text: &str) -> Key {
    let kind = speaker["kind"].as_str().unwrap_or_default().to_owned();
    let card = speaker["cardId"].as_str().unwrap_or_default().to_owned();
    (Some((kind, card)), text.trim().to_owned())
}

fn same(a: &Key, b: &Key) -> bool {
    a.1 == b.1 && (a.0.is_none() || b.0.is_none() || a.0 == b.0)
}

/// 房間本幕的一段：位置與比對鍵；系統段（觀望、收回提示、分隔）不比
pub(super) struct RoomLine {
    pub(super) at: Value,
    pub(super) key: Option<Key>,
}

/// 快照的本幕故事串（上一幕摘要之後）＋重送佇列裡同一回合派工的 `gen.part.end`（伺服器處理 sync 前會先收它們）
pub(super) fn room_lines(snapshot: &Value, queue: &[Value]) -> Vec<RoomLine> {
    let story = snapshot["story"].as_array().cloned().unwrap_or_default();
    let from = story
        .iter()
        .rposition(|p| p["part"] == 32)
        .map_or(0, |i| i + 1);
    let mut out: Vec<RoomLine> = story[from..]
        .iter()
        .map(|p| RoomLine {
            at: json!({ "round": p["round"], "part": p["part"] }),
            key: (p["speaker"]["kind"] != "system")
                .then(|| key_of(&p["speaker"], p["text"].as_str().unwrap_or_default())),
        })
        .collect();
    let round = &snapshot["round"];
    let mut speakers: HashMap<(String, i64), Value> = HashMap::new();
    for frame in queue {
        let p = &frame["p"];
        let id = p["dispatchId"].as_str().unwrap_or_default().to_owned();
        let part = p["part"].as_i64().unwrap_or(-1);
        match frame["t"].as_str() {
            Some("gen.part.begin") => {
                speakers.insert((id, part), p["speaker"].clone());
            }
            Some("gen.part.end") if round["dispatchId"] == id.as_str() => {
                let text = p["text"].as_str().unwrap_or_default();
                let key = match speakers.get(&(id, part)) {
                    Some(speaker) => key_of(speaker, text),
                    None => (None, text.trim().to_owned()),
                };
                out.push(RoomLine {
                    at: json!({ "round": round["round"], "part": part }),
                    key: Some(key),
                });
            }
            _ => {}
        }
    }
    out
}

/// 補送內容：`keepThrough` 與依序的補送項
fn plan(
    room: &Room,
    snapshot: Option<&Value>,
    queue: &[Value],
) -> Result<(Value, Vec<Value>), String> {
    let (_, root) = room.engine.roots();
    let world = room.world();
    let current = data::read_state(&root, &world)
        .map_err(|error| error.to_string())?
        .current_scene;
    let synced = room.saved().synced_scene;
    let start = synced.filter(|scene| *scene <= current).unwrap_or(current);
    let cards = seed::roster(&root, &world)?;
    let seats = room.work().seats.clone();
    let scene = |n: u64| seed::scene(&root, &world, n, &cards, &seats);
    let first = scene(start);
    let part = |line: Value| json!({ "part": line });
    // 全量重送（伺服器那邊對不上）：本幕整段換掉，不放分隔
    let (keep, skip) = match snapshot {
        None => (Value::Null, 0),
        Some(snapshot) => {
            let base = room_lines(snapshot, queue);
            let compared: Vec<(usize, &Key)> = base
                .iter()
                .enumerate()
                .filter_map(|(i, line)| line.key.as_ref().map(|key| (i, key)))
                .collect();
            let m = compared
                .iter()
                .zip(&first.lines)
                .take_while(|((_, a), line)| {
                    same(
                        a,
                        &key_of(&line["speaker"], line["text"].as_str().unwrap_or_default()),
                    )
                })
                .count();
            // 保留到第一個對不上的房間段之前（中間的系統段跟著上一回合留著）；全對上就整幕都留
            let cut = compared.get(m).map_or(base.len(), |(i, _)| *i);
            (
                cut.checked_sub(1)
                    .map_or(Value::Null, |i| base[i].at.clone()),
                m,
            )
        }
    };
    let framed = snapshot.is_some();
    let mut items: Vec<Value> = first.lines.into_iter().skip(skip).map(part).collect();
    for n in start + 1..=current {
        let next = scene(n);
        items.push(json!({ "summary": next.summary.unwrap_or_default() }));
        if framed && !next.lines.is_empty() {
            items.push(part(seed::marker("solo_paused"))); // 新幕的快照看不到舊幕那條分隔
        }
        items.extend(next.lines.into_iter().map(part));
    }
    if framed && !items.is_empty() {
        items.insert(0, part(seed::marker("solo_paused")));
        items.push(part(seed::marker("room_reopened")));
    }
    Ok((keep, items))
}

/// `story.sync` 分片；id＝`sync-<片>-<總片數>`（全量重送用 `syncf-`）。第 0 片帶 `keepThrough`
pub(super) fn frames(
    room: &Room,
    snapshot: Option<&Value>,
    queue: &[Value],
) -> Result<Vec<Value>, String> {
    let (keep, items) = plan(room, snapshot, queue)?;
    let mut parts: Vec<Vec<Value>> = vec![Vec::new()];
    let mut size = 0;
    for item in items {
        let n = item.to_string().len();
        if size + n > PART_BYTES && !parts.last().is_some_and(Vec::is_empty) {
            parts.push(Vec::new());
            size = 0;
        }
        size += n;
        parts.last_mut().expect("至少一片").push(item);
    }
    let total = parts.len();
    let prefix = if snapshot.is_some() { "sync" } else { "syncf" };
    Ok(parts
        .into_iter()
        .enumerate()
        .map(|(part, items)| {
            let mut p = json!({ "part": part, "total": total, "items": items });
            if part == 0 {
                p["keepThrough"] = keep.clone();
            }
            json!({ "t": "story.sync", "id": format!("{prefix}-{part}-{total}"), "p": p })
        })
        .collect())
}

/// 最後一片收下了：閘門解除，記下房間現在對應的桌內幕號
pub(super) fn synced(room: &Room, root: &Path) {
    let scene = data::read_state(root, &room.world())
        .ok()
        .map(|state| state.current_scene);
    room.update(|saved| {
        saved.paused = false;
        saved.synced_scene = scene;
    });
}

/// `story.sync` 的回覆：最後一片 ok 才算補完；伺服器說對不上（`keepThrough` 指不到、緩衝遺失）就改全量重送一次；
/// 伺服器說不需要（沒上閘門）也算補完
pub(super) fn on_reply(room: &Room, id: &str, error: Option<&str>, detail: &str) {
    let full = id.starts_with("syncf-");
    let nums: Vec<usize> = id
        .split('-')
        .skip(1)
        .filter_map(|n| n.parse().ok())
        .collect();
    let last = matches!(nums[..], [part, total] if part + 1 == total);
    let (_, root) = room.engine.roots();
    match error {
        None if last => synced(room, &root),
        None => {}
        Some(_) if detail == "sync_not_needed" => synced(room, &root),
        Some(_) if !full && (detail == "sync_keep_missing" || detail == "sync_restart") => {
            match frames(room, None, &[]) {
                Ok(list) => list.into_iter().for_each(|frame| {
                    let _ = room.out.send(frame);
                }),
                Err(error) => room.note_error(&format!("story.sync：{error}")),
            }
        }
        Some(_) => {}
    }
}
