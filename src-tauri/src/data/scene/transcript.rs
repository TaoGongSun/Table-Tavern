use crate::mechanism::{self, Outcome};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use super::super::message_vars;
use super::super::paths::world_dir;
use super::super::state::{read_state, read_state_cache, write_state, TableState, WorldState};
use super::super::state_commit::{with_commit, CommitTx};
use super::super::{invalid_data, DataResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptKind {
    Dialogue,
    Narration,
    Player,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptEvent {
    pub ts: String,
    /// 角色事件存角色 id；GM 旁白／系統訊息／玩家發言存空字串（kind 已足以區分）
    pub speaker_id: String,
    /// 發言當下的顯示名快照——改名後舊事件不動，這是既有拍板行為
    pub speaker_name: String,
    pub kind: TranscriptKind,
    pub text: String,
    /// 剝殼前的模型原文：狀態區塊與點名行都還在，供卡片自帶的面板重畫歷史訊息用。
    /// 與 text 相同（沒剝到東西）時不存，舊檔沒有這欄也照樣讀得起來。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<TableState>,
    /// AI 有正文但被供應商內容過濾或長度上限中途中斷；舊紀錄預設 false。
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
    /// 這則系統事件的全文只給 GM 看；chars 續聊線遇到只留第一行（AI 卡重構包 4b，
    /// 補 4a 遺留的 visibility 洩漏——非 Public 世界書人物的登場全文不該流進扮演引擎）。
    #[serde(default)]
    pub gm_only: bool,
    /// 固定標頭代碼（換幕摘要、登場、私設、狀態更新、點名）；`text` 只存本文，標頭在顯示、
    /// 匯出、送 AI 時才照語系組（見 `marker.rs`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker: Option<super::marker::EventMarker>,
    /// 開場白（`append_opening` 寫的那則）：卡片介面的 MVU 墊片靠它分辨開場與 AI 回覆——
    /// 開場由 initvar 初始化、不補狀態欄占位。舊紀錄預設 false。
    #[serde(default, skip_serializing_if = "is_false")]
    pub opening: bool,
    /// 穩定 ID（ULID），落檔時由後端配發；舊事件沒有，第一次被卡片寫入時補上（計畫 8.1）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 這樓完整的 MVU 變數表（卡片變數模式）。None＝這樓尚無表；有值就是明確的表（可以沒有 stat_data）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_vars: Option<crate::data::message_vars::VarsTable>,
    /// 這樓表的版本 token（ULID），表每次變更或復原帶回時重新產生。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vars_rev: Option<String>,
    /// 寫入這張表當下那一幕的 epoch；與控制檔裡這一幕的 epoch 相同才算初始化來源。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vars_epoch: Option<String>,
    /// GM 回合落檔的冪等鍵（turn_id＋turn_part），前端重試時靠它認出已提交的那則。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_key: Option<crate::data::message_vars::TurnKey>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

pub(crate) fn transcript_path(root: &Path, world_id: &str, scene: u64) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?
        .join("transcript")
        .join(format!("{scene}.jsonl")))
}

/// 追加一則新事件，回傳追加收據（這一行在逐字稿檔裡的起始位元組）。不屬於進行中回合：上一回合沒落成的
/// 部分先交接（代落），新事件才不會搶到舊回覆前面。
pub fn append_transcript(
    root: &Path,
    world_id: &str,
    scene: u64,
    event: &TranscriptEvent,
) -> DataResult<u64> {
    with_commit(root, world_id, |tx| {
        append_transcript_tx(tx, scene, event).map(|(offset, _)| offset)
    })
}

/// 同一回合內部的附屬追加（GM 回合提交後的登場紀錄等）：憑證對得上進行中的回合就不交接，照原本順序排在
/// 正文前；對不上（回合已結束或被換掉）就當一般新事件，先交接。
pub fn append_within_turn(
    root: &Path,
    world_id: &str,
    scene: u64,
    event: &TranscriptEvent,
    turn: &message_vars::TurnTicket,
) -> DataResult<u64> {
    with_commit(root, world_id, |tx| {
        let appended = if message_vars::turn_owns(tx, turn) {
            append_line_tx(tx, scene, event)
        } else {
            append_transcript_tx(tx, scene, event)
        };
        appended.map(|(offset, _)| offset)
    })
}

/// 鎖內版的新事件追加（開場、換幕摘要等也走這裡）：先交接上一回合沒落成的部分。
pub(crate) fn append_transcript_tx(
    tx: &CommitTx<'_>,
    scene: u64,
    event: &TranscriptEvent,
) -> DataResult<(u64, TranscriptEvent)> {
    message_vars::settle_before_append(tx, None)?;
    append_line_tx(tx, scene, event)
}

/// 實際追加一行（不交接，只給本模組的回合路徑與上面兩個入口）：沒有 ID 就配發、帶表的（復原帶回來的）
/// 換新版本 token、沒快照就蓋上目前有效狀態。回傳收據與實際落檔的那則。
fn append_line_tx(
    tx: &CommitTx<'_>,
    scene: u64,
    event: &TranscriptEvent,
) -> DataResult<(u64, TranscriptEvent)> {
    let (root, world_id) = (tx.root, tx.world_id);
    let mut event = event.clone();
    if event.id.is_none() {
        event.id = Some(message_vars::new_token());
    }
    if event.message_vars.is_some() {
        event.vars_rev = Some(message_vars::new_token());
    } else {
        event.vars_rev = None;
        event.vars_epoch = None;
    }
    if event.state.is_none() {
        // 復原舊句子會帶回當時快照，只有新事件才借用目前檯面。讀不到就停，不再照寫。
        event.state = Some(read_state(root, world_id)?.state);
    }
    let mut line = serde_json::to_vec(&event)?;
    line.push(b'\n');
    let offset = super::super::world_file::commit_world_append(
        &transcript_path(root, world_id, scene)?,
        &line,
    )?;
    // 目前值恆等於最後一則事件的快照，復原舊句時狀態才會跟著回到那一刻。
    // 快取寫失敗不該把「事件已經寫進去了」這件事變成錯誤，權威在 transcript。
    if let Some(snapshot) = &event.state {
        if let Ok(mut world) = read_state_cache(root, world_id) {
            if world.state != *snapshot {
                world.state = snapshot.clone();
                let _ = write_state(root, world_id, &world);
            }
        }
    }
    message_vars::refresh_cache(tx);
    Ok((offset, event))
}

/// 前端落一則（玩家句、GM 正文、系統事件、復原）：GM 回合的部分帶 `turn` 冪等鍵——同鍵已落過就回那則，
/// `main` 在回合紀錄等落檔時掛上算好的表（不收前端傳來的變數）。回傳落檔的那則與收據（冪等命中時沒有收據）。
/// 不是這一回合的新事件追加前，上一回合沒落成的部分先代落（回合交接），順序不會錯。
pub fn append_event(
    root: &Path,
    world_id: &str,
    scene: u64,
    event: &TranscriptEvent,
    turn: Option<&message_vars::TurnKey>,
) -> DataResult<(TranscriptEvent, Option<u64>)> {
    with_commit(root, world_id, |tx| {
        message_vars::settle_before_append(tx, turn)?;
        let mut event = event.clone();
        let mut table = None;
        if let Some(key) = turn {
            match message_vars::prepare_turn_append(tx, scene, key)? {
                message_vars::TurnAppend::Existing(existing) => {
                    message_vars::mark_turn_appended(tx, key);
                    return Ok((existing, None));
                }
                message_vars::TurnAppend::New { table: attach } => table = attach,
            }
            event.turn_key = Some(key.clone());
            event.message_vars = None;
        }
        if let Some(table) = &table {
            if event.state.is_none() {
                event.state = Some(read_state(root, world_id)?.state);
            }
            if let Some(snapshot) = event.state.as_mut() {
                snapshot.tree = message_vars::Source::Seed {
                    table: table.clone(),
                }
                .tree();
            }
            event.message_vars = Some(message_vars::VarsTable::from_json(table));
            event.vars_epoch = message_vars::scene_epoch(tx, scene)?;
        }
        let (offset, event) = append_line_tx(tx, scene, &event)?;
        if let Some(key) = turn {
            message_vars::mark_turn_appended(tx, key);
        }
        Ok((event, Some(offset)))
    })
}

/// 上一回合沒落成的部分先代落（貼開場這類要先存檢查點的操作，在存檢查點之前呼叫）。
pub fn settle_pending_turn(root: &Path, world_id: &str) -> DataResult<()> {
    with_commit(root, world_id, |tx| {
        message_vars::settle_before_append(tx, None)
    })
}

/// 貼開場白之前這一幕逐字稿、state.json 與卡片變數控制檔的原始位元組。逐字稿是直接 append，失敗時可能
/// 已留下半行，呼叫端靠它寫回並確認回到原樣，才能當成「什麼都沒貼上」。
pub struct OpeningCheckpoint {
    root: PathBuf,
    world_id: String,
    transcript_path: PathBuf,
    transcript: Option<Vec<u8>>,
    state_path: PathBuf,
    state: Vec<u8>,
    control_path: PathBuf,
    control: Option<Vec<u8>>,
}

fn read_optional(path: &Path) -> DataResult<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn opening_checkpoint(
    root: &Path,
    world_id: &str,
    scene: u64,
) -> DataResult<OpeningCheckpoint> {
    let transcript_path = transcript_path(root, world_id, scene)?;
    let state_path = world_dir(root, world_id)?.join("state.json");
    let control_path = message_vars::control_path(root, world_id)?;
    Ok(OpeningCheckpoint {
        root: root.to_path_buf(),
        world_id: world_id.to_owned(),
        transcript: read_optional(&transcript_path)?,
        state: fs::read(&state_path)?,
        control: read_optional(&control_path)?,
        transcript_path,
        state_path,
        control_path,
    })
}

/// 不一樣的寫回去（原本沒有就刪掉），再讀回比對；回到原樣才回 true。
fn restore_optional(path: &Path, original: &Option<Vec<u8>>) -> bool {
    use super::super::world_file::{commit_world_remove, commit_world_write_atomic};
    match read_optional(path) {
        Ok(now) if now == *original => true,
        _ => {
            let written = match original {
                Some(bytes) => commit_world_write_atomic(path, bytes),
                None => commit_world_remove(path),
            };
            written.is_ok() && read_optional(path).is_ok_and(|now| now == *original)
        }
    }
}

impl OpeningCheckpoint {
    /// 整桌還原（計畫 8.4）：鎖內寫回三份檔、清回合紀錄、桌世代加一；全部回到原樣才回 true。
    pub fn restore(&self) -> bool {
        with_commit(&self.root, &self.world_id, |tx| {
            message_vars::bump_generation(tx);
            let transcript_back = restore_optional(&self.transcript_path, &self.transcript);
            let state_back = restore_optional(&self.state_path, &Some(self.state.clone()));
            let control_back = restore_optional(&self.control_path, &self.control);
            transcript_back && state_back && control_back
        })
    }
}

/// 開場白也要存成快照，收回時檯面才能回到貼上前的最後一句；狀態區塊走與 GM 回覆同一條
/// 本地權威（mechanism::apply_block），增量桌的數值一開場就是本機在算。
/// 卡片變數模式（第一次開場就啟用）：以初始化來源為底套開場狀態塊，事件帶新表。
pub fn append_opening(
    root: &Path,
    world_id: &str,
    scene: u64,
    ts: &str,
    raw: &str,
    block: &crate::transport::StateBlock,
    user_name: &str,
) -> DataResult<(TranscriptEvent, Outcome)> {
    with_commit(root, world_id, |tx| {
        // 先交接：上一回合沒落成的回覆（含新表）先落，開場表才以它為底，不會被舊來源算出的表蓋回
        message_vars::settle_before_append(tx, None)?;
        let macros = message_vars::Macros {
            user: user_name.to_owned(),
            char: None,
        };
        message_vars::ensure_active(tx, Some(&macros))?;
        let mut world = read_state(root, world_id)?;
        let before = world.state.tree.clone();
        let outcome = mechanism::apply_block(&mut world, block, user_name);
        let table = message_vars::opening_table(
            tx,
            scene,
            &before,
            &world.state.tree,
            &world.mechanism.value_types,
        )?;
        let event = TranscriptEvent {
            ts: ts.to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: block.display.clone(),
            raw: (raw != block.display).then(|| raw.to_owned()),
            state: Some(world.state),
            truncated: false,
            gm_only: false,
            marker: None,
            opening: true,
            id: None,
            vars_epoch: match &table {
                Some(_) => message_vars::scene_epoch(tx, scene)?,
                None => None,
            },
            message_vars: table.as_ref().map(message_vars::VarsTable::from_json),
            vars_rev: None,
            turn_key: None,
        };
        let (_, event) = append_transcript_tx(tx, scene, &event)?;
        Ok((event, outcome))
    })
}

fn serialize_events(events: &[TranscriptEvent]) -> DataResult<Vec<u8>> {
    let mut buffer = Vec::new();
    for event in events {
        buffer.extend_from_slice(&serde_json::to_vec(event)?);
        buffer.push(b'\n');
    }
    Ok(buffer)
}

/// 讀改寫這一幕：在逐字稿的同檔鎖內讀出事件交給 `edit`，`edit` 回 `Some` 就整檔寫回，
/// 讀與寫之間不會插進別的追加。檯面回捲等其他檔的事，呼叫端在放鎖之後再做。
fn edit_scene<T>(
    root: &Path,
    world_id: &str,
    scene: u64,
    edit: impl FnOnce(Vec<TranscriptEvent>) -> DataResult<(T, Option<Vec<TranscriptEvent>>)>,
) -> DataResult<T> {
    let path = transcript_path(root, world_id, scene)?;
    super::super::world_file::with_file_lock(&path, |file| {
        let events = match file.read()? {
            Some(bytes) => parse_transcript(&bytes)?,
            None => Vec::new(),
        };
        let (result, rewrite) = edit(events)?;
        if let Some(events) = rewrite {
            file.write_atomic(&serialize_events(&events)?)?;
        }
        Ok(result)
    })
}

/// 刪掉事件之後把檯面退回剩下事件的最後一份快照（這一幕沒了就往前一幕找）。
/// 刪事件的幾條路（收回上一句、復原匯入收掉開場白、收回沒有回覆的玩家句）共用。
/// 變數模式時樹另外照投影重建（收光帶表事件就回到這一幕的種子）。
fn rewind_state(tx: &CommitTx<'_>, scene: u64, events: &[TranscriptEvent]) -> DataResult<()> {
    let (root, world_id) = (tx.root, tx.world_id);
    let mut state = read_state_cache(root, world_id)?;
    state.state = events
        .iter()
        .rev()
        .find_map(|entry| entry.state.clone())
        .or_else(|| {
            scene.checked_sub(1).and_then(|previous_scene| {
                read_transcript(root, world_id, previous_scene)
                    .ok()
                    .and_then(|previous_events| {
                        previous_events
                            .iter()
                            .rev()
                            .find_map(|entry| entry.state.clone())
                    })
            })
        })
        .unwrap_or_default();
    write_state(root, world_id, &state)?;
    message_vars::refresh_cache(tx);
    Ok(())
}

/// 狀態樹被逐字稿以外的路徑換掉（重構套用重建欄位）之後，把新樹補進這一幕每一則事件的快照。
/// 收回上一句與換幕都拿事件快照當回捲基準，不補的話玩家一收回，介面就被打回重構前的舊欄位。
/// 補整幕而不是只補最後一則：連按收回會一路往前吃，任何一則留著舊欄位都會在那一下現形。
/// 只換 tree／jumps——劇情面的欄位（table、changes、notes）照舊跟著各自那一刻走。
pub fn sync_scene_state_tree(root: &Path, world_id: &str, state: &WorldState) -> DataResult<()> {
    with_commit(root, world_id, |tx| {
        let scene = state.current_scene;
        let rewritten = edit_scene(root, world_id, scene, |mut events| {
            let mut touched = false;
            for event in events.iter_mut() {
                let Some(snapshot) = event.state.as_mut() else {
                    continue;
                };
                if snapshot.tree != state.state.tree || snapshot.jumps != state.state.jumps {
                    snapshot.tree = state.state.tree.clone();
                    snapshot.jumps = state.state.jumps.clone();
                    touched = true;
                }
            }
            Ok(if touched {
                (Some(events.clone()), Some(events))
            } else {
                (None, None)
            })
        })?;
        if let Some(events) = rewritten {
            rewind_state(tx, scene, &events)?;
        }
        Ok(())
    })
}

/// 收回上一句（可連按）：砍掉這一幕最後一筆事件後整檔重寫（帶的表跟著事件一起走）。
/// 回傳是否真的刪了——這一幕已經空了就是 false，收不會倒退咬到上一幕。
pub fn pop_transcript(root: &Path, world_id: &str, scene: u64) -> DataResult<bool> {
    with_commit(root, world_id, |tx| {
        let remaining = edit_scene(root, world_id, scene, |mut events| {
            Ok(if events.pop().is_none() {
                (None, None)
            } else {
                (Some(events.clone()), Some(events))
            })
        })?;
        let Some(events) = remaining else {
            return Ok(false);
        };
        rewind_state(tx, scene, &events)?;
        Ok(true)
    })
}

/// 復原匯入用：從這一幕刪掉時間戳相符的那一則（貼出的開場白），其餘事件原位不動。
/// 回傳是否真的刪到——玩家自己先收回過就是 false。
pub fn remove_transcript_event(
    root: &Path,
    world_id: &str,
    scene: u64,
    ts: &str,
) -> DataResult<bool> {
    with_commit(root, world_id, |tx| {
        let remaining = edit_scene(root, world_id, scene, |mut events| {
            let before = events.len();
            events.retain(|event| event.ts != ts);
            Ok(if events.len() == before {
                (None, None)
            } else {
                (Some(events.clone()), Some(events))
            })
        })?;
        let Some(events) = remaining else {
            return Ok(false);
        };
        rewind_state(tx, scene, &events)?;
        Ok(true)
    })
}

pub fn set_last_transcript_state(
    root: &Path,
    world_id: &str,
    scene: u64,
    state: &TableState,
) -> DataResult<bool> {
    with_commit(root, world_id, |_| {
        edit_scene(root, world_id, scene, |mut events| {
            let Some(entry) = events.last_mut() else {
                return Ok((false, None));
            };
            entry.state = Some(state.clone());
            Ok((true, Some(events)))
        })
    })
}

/// 打字送出後 AI 沒回成：收掉剛追加、還沒有任何回覆的那句玩家發言。
/// 整段在逐字稿同檔鎖內：`offset`（追加收據）必須是行首、從那裡到檔尾恰為最後一行，
/// 且解析後是玩家句、`ts` 與 `text` 都相符，才截檔；任何一項不符回 false 不動。
/// 截檔回錯就在鎖內量實際長度，剛好等於 `offset` 才算已刪，量不到也回 false。
/// 刪成功後檯面回捲盡力而為（權威在逐字稿），失敗不翻成刪除失敗。
pub fn discard_unanswered_player(
    root: &Path,
    world_id: &str,
    scene: u64,
    offset: u64,
    ts: &str,
    text: &str,
) -> DataResult<bool> {
    with_commit(root, world_id, |tx| {
        // 回覆已產生、只是還沒落檔（後端會代落）：這句不是沒回成，不收
        if message_vars::has_unlanded_reply(tx) {
            return Ok(false);
        }
        let path = transcript_path(root, world_id, scene)?;
        let removed =
            super::super::world_file::with_file_lock(&path, |file| -> DataResult<bool> {
                let Some(bytes) = file.read()? else {
                    return Ok(false);
                };
                let Ok(start) = usize::try_from(offset) else {
                    return Ok(false);
                };
                if start >= bytes.len()
                    || (start > 0 && bytes[start - 1] != b'\n')
                    || !bytes.ends_with(b"\n")
                {
                    return Ok(false);
                }
                let line = &bytes[start..bytes.len() - 1];
                if line.contains(&b'\n') {
                    return Ok(false);
                }
                let Ok(event) = serde_json::from_slice::<TranscriptEvent>(line) else {
                    return Ok(false);
                };
                if event.kind != TranscriptKind::Player || event.ts != ts || event.text != text {
                    return Ok(false);
                }
                if file.truncate(offset).is_ok() {
                    return Ok(true);
                }
                Ok(matches!(file.len(), Ok(len) if len == offset))
            })?;
        if removed {
            if let Ok(events) = read_transcript(root, world_id, scene) {
                let _ = rewind_state(tx, scene, &events);
            }
        }
        Ok(removed)
    })
}

/// 解析一幕逐字稿。最後一行沒有換行而且解析失敗＝追加到一半的殘段，略過；
/// 其他位置的壞行照舊報錯。
pub(super) fn parse_transcript(bytes: &[u8]) -> DataResult<Vec<TranscriptEvent>> {
    let text = String::from_utf8_lossy(bytes);
    let unterminated = !text.is_empty() && !text.ends_with('\n');
    let lines: Vec<&str> = text.lines().collect();
    let mut events = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        match serde_json::from_str(line) {
            Ok(event) => events.push(event),
            Err(_) if unterminated && index + 1 == lines.len() => {}
            Err(error) => {
                return Err(invalid_data(format!(
                    "invalid transcript line {}: {error}",
                    index + 1
                )))
            }
        }
    }
    Ok(events)
}

/// 在同檔鎖內一次讀整檔、鎖外解析，不會讀到追加與截回之間的半行。
pub fn read_transcript(
    root: &Path,
    world_id: &str,
    scene: u64,
) -> DataResult<Vec<TranscriptEvent>> {
    let path = transcript_path(root, world_id, scene)?;
    let bytes = super::super::world_file::with_file_lock(&path, |file| file.read())?;
    match bytes {
        Some(bytes) => parse_transcript(&bytes),
        None => Ok(Vec::new()),
    }
}

mod lines;
pub(crate) use lines::{edit_line, find_event_rev, find_rev, LineHead};

#[cfg(test)]
mod write_safety_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::test_support::*;
    use crate::data::*;
    use std::collections::BTreeMap;
    use std::fs::OpenOptions;
    use std::io::Write;

    #[test]
    fn transcript_round_trip_is_ordered_jsonl_and_rejects_invalid_kind() {
        let root = TestRoot::new("transcript");
        let world_id = create_world(root.path(), "劇場").unwrap();
        let events = vec![
            TranscriptEvent {
                id: None,
                message_vars: None,
                vars_rev: None,
                vars_epoch: None,
                turn_key: None,
                raw: None,
                ts: "2026-07-19T10:00:00+08:00".to_owned(),
                speaker_id: String::new(),
                speaker_name: "旁白".to_owned(),
                kind: TranscriptKind::Narration,
                text: "序幕".to_owned(),
                state: None,
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
            TranscriptEvent {
                id: None,
                message_vars: None,
                vars_rev: None,
                vars_epoch: None,
                turn_key: None,
                raw: None,
                ts: "2026-07-19T10:00:01+08:00".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "第一行\n仍是同一事件".to_owned(),
                state: None,
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
            TranscriptEvent {
                id: None,
                message_vars: None,
                vars_rev: None,
                vars_epoch: None,
                turn_key: None,
                raw: None,
                ts: "2026-07-19T10:00:02+08:00".to_owned(),
                speaker_id: "角色代碼".to_owned(),
                speaker_name: "角色".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "你好".to_owned(),
                state: None,
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
        ];
        for event in &events {
            append_transcript(root.path(), &world_id, 7, event).unwrap();
        }
        let read = read_transcript(root.path(), &world_id, 7).unwrap();
        // 每則落檔時都配發了各自的穩定 ID
        let ids: std::collections::BTreeSet<_> =
            read.iter().map(|event| event.id.clone()).collect();
        assert_eq!(ids.len(), 3);
        assert!(read
            .iter()
            .all(|event| event.id.as_ref().is_some_and(|id| id.len() == 26)));
        let expected: Vec<_> = events
            .iter()
            .cloned()
            .zip(&read)
            .map(|(mut event, stored)| {
                event.state = Some(TableState::default());
                event.id = stored.id.clone();
                event
            })
            .collect();
        assert_eq!(read, expected);

        let path = root
            .path()
            .join(format!("worlds/{world_id}/transcript/7.jsonl"));
        let raw = fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = raw.lines().collect();
        assert_eq!(lines.len(), 3);
        for line in lines {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(value.is_object());
            assert!(["dialogue", "narration", "player", "system"]
                .contains(&value["kind"].as_str().unwrap()));
        }

        OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(b"{\"ts\":\"now\",\"speaker_id\":\"\",\"speaker_name\":\"x\",\"kind\":\"bad\",\"text\":\"x\"}\n")
            .unwrap();
        let error = read_transcript(root.path(), &world_id, 7)
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 4"), "{error}");
    }

    #[test]
    fn pop_transcript_removes_last_event_until_scene_is_empty() {
        let root = TestRoot::new("transcript-pop");
        let world_id = create_world(root.path(), "收回桌").unwrap();
        let events: Vec<TranscriptEvent> = ["序幕", "我推開門", "誰在那裡？"]
            .iter()
            .enumerate()
            .map(|(index, text)| TranscriptEvent {
                id: None,
                message_vars: None,
                vars_rev: None,
                vars_epoch: None,
                turn_key: None,
                raw: None,
                ts: format!("2026-08-01T10:00:0{index}+08:00"),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: (*text).to_owned(),
                state: None,
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            })
            .collect();
        for event in &events {
            append_transcript(root.path(), &world_id, 0, event).unwrap();
        }

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        let read = read_transcript(root.path(), &world_id, 0).unwrap();
        let expected: Vec<_> = events[..2]
            .iter()
            .cloned()
            .zip(&read)
            .map(|(mut event, stored)| {
                event.state = Some(TableState::default());
                event.id = stored.id.clone();
                event
            })
            .collect();
        assert_eq!(read, expected);
        // 重寫後仍是合法 JSONL：行數對齊事件數，沒有殘留的半行
        let path = root
            .path()
            .join(format!("worlds/{world_id}/transcript/0.jsonl"));
        assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 2);

        // 連按到底：收乾淨後再按回 false，不會倒退咬到別的幕
        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert!(!pop_transcript(root.path(), &world_id, 0).unwrap());
        assert!(read_transcript(root.path(), &world_id, 0)
            .unwrap()
            .is_empty());

        // 沒開始過的幕：不建檔也不報錯
        assert!(!pop_transcript(root.path(), &world_id, 9).unwrap());
        assert!(!root
            .path()
            .join(format!("worlds/{world_id}/transcript/9.jsonl"))
            .exists());
    }

    #[test]
    fn append_transcript_uses_current_snapshot_without_overwriting_supplied_state() {
        let root = TestRoot::new("transcript-state-snapshot");
        let world_id = create_world(root.path(), "狀態桌").unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state
            .state
            .table
            .insert("time".to_owned(), "清晨".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();
        let event = TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            raw: None,
            ts: "now".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: "第一句".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        };
        append_transcript(root.path(), &world_id, 0, &event).unwrap();
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap()[0]
                .state
                .as_ref()
                .unwrap()
                .table
                .get("time"),
            Some(&"清晨".to_owned())
        );

        let supplied = TableState {
            table: BTreeMap::from([("time".to_owned(), "午夜".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                id: None,
                message_vars: None,
                vars_rev: None,
                vars_epoch: None,
                turn_key: None,
                raw: None,
                ts: "later".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: "第二句".to_owned(),
                state: Some(supplied.clone()),
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
        )
        .unwrap();
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap()[1].state,
            Some(supplied)
        );
    }

    #[test]
    fn append_opening_skips_raw_when_nothing_was_stripped() {
        let root = TestRoot::new("opening-raw");
        let world_id = create_world(root.path(), "純正文桌").unwrap();
        let raw = "只有旁白，沒有狀態欄。";
        let (event, _) = append_opening(
            root.path(),
            &world_id,
            0,
            "opening",
            raw,
            &crate::transport::extract_state_block(raw),
            "阿濤",
        )
        .unwrap();
        assert_eq!(event.text, raw);
        assert_eq!(event.raw, None);
        // 舊檔沒有 raw 欄位也讀得起來，序列化時同樣不憑空多一欄
        let line = serde_json::to_string(&event).unwrap();
        assert!(!line.contains("\"raw\""));
        // 開場標記落檔（MVU 墊片靠它分辨開場）；一般事件與舊檔不帶這欄
        assert!(event.opening);
        assert!(line.contains("\"opening\":true"));
        let old: TranscriptEvent = serde_json::from_str(
            r#"{"ts":"t","speaker_id":"","speaker_name":"GM","kind":"narration","text":"x"}"#,
        )
        .unwrap();
        assert!(!old.opening);
        assert!(!serde_json::to_string(&old).unwrap().contains("opening"));
    }

    #[test]
    fn append_opening_merges_state_and_pop_restores_previous_snapshot() {
        let root = TestRoot::new("opening-state");
        let world_id = create_world(root.path(), "開場狀態桌").unwrap();
        let previous = TableState {
            table: BTreeMap::from([("place".to_owned(), "酒館".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                id: None,
                message_vars: None,
                vars_rev: None,
                vars_epoch: None,
                turn_key: None,
                raw: None,
                ts: "before".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: "前一則".to_owned(),
                state: Some(previous.clone()),
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
        )
        .unwrap();

        let raw = "開場旁白<status>place: 碼頭\ntime: 午夜</status>";
        let (event, outcome) = append_opening(
            root.path(),
            &world_id,
            0,
            "opening",
            raw,
            &crate::transport::extract_state_block(raw),
            "阿濤",
        )
        .unwrap();
        assert!(outcome.records.is_empty());
        // 畫面只留正文，模型原文整段另存一份（面板要靠它重畫歷史訊息）
        assert_eq!(event.text, "開場旁白");
        assert_eq!(event.raw.as_deref(), Some(raw));
        let expected = TableState {
            table: BTreeMap::from([
                ("place".to_owned(), "碼頭".to_owned()),
                ("time".to_owned(), "午夜".to_owned()),
            ]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        assert_eq!(event.state, Some(expected.clone()));
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, expected);
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap()[1],
            event
        );

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, previous);
    }

    #[test]
    fn pop_transcript_restores_the_previous_event_snapshot() {
        let root = TestRoot::new("transcript-state-pop");
        let world_id = create_world(root.path(), "回收狀態桌").unwrap();
        let first = TableState {
            table: BTreeMap::from([("place".to_owned(), "酒館".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        let second = TableState {
            table: BTreeMap::from([("place".to_owned(), "碼頭".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        for (text, snapshot) in [("第一句", first.clone()), ("第二句", second.clone())] {
            append_transcript(
                root.path(),
                &world_id,
                0,
                &TranscriptEvent {
                    id: None,
                    message_vars: None,
                    vars_rev: None,
                    vars_epoch: None,
                    turn_key: None,
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: text.to_owned(),
                    state: Some(snapshot),
                    truncated: false,
                    gm_only: false,
                    marker: None,
                    opening: false,
                },
            )
            .unwrap();
        }
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.state = second;
        write_state(root.path(), &world_id, &state).unwrap();

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, first);
    }

    /// 復原＝把帶著自身快照的舊事件原樣寫回，目前值要跟著回到那一刻
    /// （否則狀態欄會停在收回後的舊值，跟桌上最後一句對不起來）
    #[test]
    fn restoring_an_undone_event_puts_its_snapshot_back() {
        let root = TestRoot::new("transcript-state-restore");
        let world_id = create_world(root.path(), "復原狀態桌").unwrap();
        let snapshots = ["清晨", "午夜"].map(|time| TableState {
            table: BTreeMap::from([("time".to_owned(), time.to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        });
        let event = |text: &str, snapshot: &TableState| TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            raw: None,
            ts: "now".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: text.to_owned(),
            state: Some(snapshot.clone()),
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        };
        for (text, snapshot) in [("第一句", &snapshots[0]), ("第二句", &snapshots[1])] {
            append_transcript(root.path(), &world_id, 0, &event(text, snapshot)).unwrap();
        }
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().state,
            snapshots[1]
        );

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().state,
            snapshots[0]
        );

        append_transcript(root.path(), &world_id, 0, &event("第二句", &snapshots[1])).unwrap();
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().state,
            snapshots[1]
        );
    }

    #[test]
    fn pop_transcript_restores_entire_nested_tree_snapshot() {
        let root = TestRoot::new("nested-state-pop");
        let world_id = create_world(root.path(), "巢狀桌").unwrap();
        let first = TableState {
            table: BTreeMap::new(),
            tree: BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "城市".to_owned(),
                    StateNode::Branch(BTreeMap::from([(
                        "聲望".to_owned(),
                        StateNode::Leaf("10".to_owned()),
                    )])),
                )])),
            )]),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        let second = TableState {
            table: BTreeMap::new(),
            tree: BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "城市".to_owned(),
                    StateNode::Branch(BTreeMap::from([(
                        "聲望".to_owned(),
                        StateNode::Leaf("20".to_owned()),
                    )])),
                )])),
            )]),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        for snapshot in [first.clone(), second.clone()] {
            append_transcript(
                root.path(),
                &world_id,
                0,
                &TranscriptEvent {
                    id: None,
                    message_vars: None,
                    vars_rev: None,
                    vars_epoch: None,
                    turn_key: None,
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "旁白".to_owned(),
                    state: Some(snapshot),
                    truncated: false,
                    gm_only: false,
                    marker: None,
                    opening: false,
                },
            )
            .unwrap();
        }
        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, first);
    }
}
