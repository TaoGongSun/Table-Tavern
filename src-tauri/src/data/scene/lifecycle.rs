use std::path::Path;

use super::super::message_vars;
use super::super::state::{read_state, write_state, SceneLabel, WorldState};
use super::super::state_commit::{with_commit, CommitTx};
use super::super::{local_timestamp, DataResult};
use super::marker::EventMarker;
use super::presence::settle_card_visibility;
use super::transcript::{
    append_transcript_tx, parse_transcript, read_transcript, transcript_path, TranscriptEvent,
    TranscriptKind,
};
use crate::ui_msg::UiMsg;

/// 沒進 scene_labels 的幕＝原線（舊存檔也走這條）：顯示編號就是內部幕號，第 1 版，上一幕是前一號。
pub fn scene_label(state: &WorldState, scene: u64) -> SceneLabel {
    state
        .scene_labels
        .get(&scene.to_string())
        .copied()
        .unwrap_or(SceneLabel {
            base: scene,
            version: 1,
            parent: scene.checked_sub(1),
            forked: false,
        })
}

/// 算「某個 base 目前該排第幾個版本」：掃 0..=upto 每一幕的顯示 base，數出撞號的幕數再 +1。
/// begin_next_scene 與 fork_scene 都靠它算新標籤，掃描範圍在插入新標籤之前的呼叫端已經固定。
fn next_scene_version(state: &WorldState, upto: u64, base: u64) -> u32 {
    (0..=upto)
        .filter(|&scene| scene_label(state, scene).base == base)
        .count() as u32
        + 1
}

/// 分岔：把某一幕的紀錄原樣複製成新的一幕接著玩，原本歷史一個字都不動。
/// 顯示編號跟隨來源幕（從分岔幕再分岔＝跟著源頭走，不是跟著內部號走），
/// parent 記分岔當下所在的幕，退回時回到這裡而不是來源幕。
pub fn fork_scene(root: &Path, world_id: &str, from_scene: u64) -> DataResult<u64> {
    let forked = with_commit(root, world_id, |tx| fork_scene_tx(tx, from_scene));
    crate::data::bump_capacity_epoch();
    forked
}

/// 發布順序（計畫 8.4）：①新幕逐字稿（事件 id 與表版本重新配發、來源幕有效 epoch 的表改成新幕 epoch）
/// ②控制檔寫新幕種子 ③寫 current_scene。任一步失敗就停；新幕號仍是「目前幕＋1」，重試覆寫上次的殘留。
fn fork_scene_tx(tx: &CommitTx<'_>, from_scene: u64) -> DataResult<u64> {
    let (root, world_id) = (tx.root, tx.world_id);
    message_vars::refuse_during_turn(tx)?;
    let mut state = read_state(root, world_id)?;
    if from_scene >= state.current_scene {
        return Err(UiMsg::SceneForkNotEarlier.into_error());
    }
    let mut events = read_transcript(root, world_id, from_scene)?;
    if events.is_empty() {
        return Err(UiMsg::SceneNothingToContinue.into_error());
    }

    let current_scene = state.current_scene;
    let new_scene = current_scene + 1;
    let plan = message_vars::scene_seed_for_fork(tx, from_scene)?;
    for event in events.iter_mut() {
        event.id = Some(message_vars::new_token());
        event.turn_key = None;
        if event.message_vars.is_some() {
            event.vars_rev = Some(message_vars::new_token());
            if let Some(plan) = &plan {
                if event.vars_epoch.as_deref() == Some(plan.source_epoch.as_str()) {
                    event.vars_epoch = Some(plan.vars.epoch.clone());
                }
            }
        }
    }
    let mut buffer = String::new();
    for event in &events {
        buffer.push_str(&serde_json::to_string(event)?);
        buffer.push('\n');
    }
    super::super::world_file::commit_world_write(
        &transcript_path(root, world_id, new_scene)?,
        buffer.as_bytes(),
    )?;
    if let Some(plan) = plan {
        message_vars::publish_scene_seed(tx, new_scene, plan.vars)?;
    }

    let base = scene_label(&state, from_scene).base;
    let version = next_scene_version(&state, current_scene, base);
    state.scene_labels.insert(
        new_scene.to_string(),
        SceneLabel {
            base,
            version,
            parent: Some(current_scene),
            forked: true,
        },
    );
    state.current_scene = new_scene;
    state.state = events
        .iter()
        .rev()
        .find_map(|event| event.state.clone())
        .unwrap_or_default();
    write_state(root, world_id, &state)?;
    message_vars::refresh_cache(tx);
    Ok(new_scene)
}

/// 換場：把摘要包成一則 GM 旁白 append 到下一場景開頭，再把 current_scene +1 並存檔。
/// 回傳新場景號。摘要文字本身由呼叫端（單發 LLM）產生，這裡只負責落地與推進場次。
/// title 有值就存進「舊場景」（bump 前的 current_scene）的 scene_titles，與場次 +1 同一次 write_state。
pub fn begin_next_scene(
    root: &Path,
    world_id: &str,
    summary_text: &str,
    title: Option<&str>,
) -> DataResult<u64> {
    let next = with_commit(root, world_id, |tx| {
        begin_next_scene_tx(tx, summary_text, title)
    });
    crate::data::bump_capacity_epoch();
    next
}

/// 發布順序（計畫 8.4）：①控制檔寫新幕種子（舊幕結束時的初始化來源、新 epoch）②新幕開頭的摘要事件
/// ③寫 current_scene。失敗就停、回錯；重試時新幕號仍是「目前幕＋1」，①② 覆寫上次沒發布的殘留。
fn begin_next_scene_tx(
    tx: &CommitTx<'_>,
    summary_text: &str,
    title: Option<&str>,
) -> DataResult<u64> {
    let (root, world_id) = (tx.root, tx.world_id);
    message_vars::refuse_during_turn(tx)?;
    let mut state = read_state(root, world_id)?;
    let old_scene = state.current_scene;
    let next_scene = old_scene + 1;
    // 變數模式：新幕開頭的摘要帶一份種子表，第一個 GM 回覆前卡片照樣讀得到值、補得到狀態欄占位
    let seed = message_vars::scene_seed_for_next(tx, old_scene, next_scene)?;
    // 上次沒發布的殘留（摘要寫了、current_scene 沒寫成）：新幕逐字稿重來
    super::super::world_file::commit_world_remove(&transcript_path(root, world_id, next_scene)?)?;
    append_transcript_tx(
        tx,
        next_scene,
        &TranscriptEvent {
            id: None,
            message_vars: seed.as_ref().map(|vars| vars.seed.clone()),
            vars_rev: None,
            vars_epoch: seed.as_ref().map(|vars| vars.epoch.clone()),
            turn_key: None,
            action_id: None,
            raw: None,
            ts: local_timestamp()?,
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: summary_text.to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: Some(EventMarker::SceneSummary),
            opening: false,
        },
    )?;
    if let Some(name) = title.map(str::trim).filter(|name| !name.is_empty()) {
        state
            .scene_titles
            .insert(old_scene.to_string(), name.to_owned());
    }
    let base = scene_label(&state, old_scene).base + 1;
    let version = next_scene_version(&state, old_scene, base);
    state.scene_labels.insert(
        next_scene.to_string(),
        SceneLabel {
            base,
            version,
            parent: Some(old_scene),
            forked: false,
        },
    );
    state.current_scene = next_scene;
    write_state(root, world_id, &state)?;
    let interface_takeover =
        crate::data::is_interface_takeover(root, world_id, state.refactor_mode.as_deref());
    settle_card_visibility(
        root,
        world_id,
        old_scene,
        state.state.table.get("present").map(String::as_str),
        interface_takeover,
    );
    Ok(next_scene)
}

/// 退回前幕：換幕的精確反向操作，純本地檔案處理不必呼叫模型。
/// 前一幕看 scene_labels 的 parent（原線／分岔都適用），不再假設一定是「幕號 -1」。
/// 只認「這一幕剛好一則事件」——begin_next_scene 保證新幕開頭就是那則摘要，
/// 多於一則代表玩家已經在這一幕行動過，退回會悄悄吃掉那些內容，所以直接擋，
/// 且擋下時故意先不動任何檔案／狀態（讀完才判斷），錯誤路徑不留副作用。
pub fn revert_scene(root: &Path, world_id: &str) -> DataResult<u64> {
    let reverted = with_commit(root, world_id, |tx| revert_scene_tx(tx));
    crate::data::bump_capacity_epoch();
    reverted
}

/// 退幕（計畫 8.4）：①寫 current_scene 切回父幕（父幕事件、epoch、種子原封不動）②成功後才移除子幕種子；
/// ② 失敗只留下用不到的殘留，下次換幕到同一號時會被覆寫。
fn revert_scene_tx(tx: &CommitTx<'_>) -> DataResult<u64> {
    let (root, world_id) = (tx.root, tx.world_id);
    let mut state = read_state(root, world_id)?;
    let scene = state.current_scene;
    let Some(previous_scene) = scene_label(&state, scene).parent else {
        return Err(UiMsg::SceneFirstNoPrevious.into_error());
    };
    message_vars::refuse_during_turn(tx)?;
    // 逐字稿的追加都走短提交鎖：這裡判完到刪檔之間不會插進新的一句
    if read_transcript(root, world_id, scene)?.len() != 1 {
        return Err(UiMsg::SceneRewindHasNewContent.into_error());
    }
    state.current_scene = previous_scene;
    state.scene_titles.remove(&previous_scene.to_string());
    // 自己這筆標籤跟著檔案一起消失，不留退回後查不到來源、卻還佔著 key 的殭屍紀錄。
    state.scene_labels.remove(&scene.to_string());
    // current_scene 落回前幕，前幕本來就對齊過了，aligned_scene 不用跟著動。
    state.state = read_transcript(root, world_id, previous_scene)?
        .iter()
        .rev()
        .find_map(|event| event.state.clone())
        .unwrap_or_default();
    // 先發布切回父幕，成功後才清子幕：清不掉只留下用不到的殘留（下次換到同一號時覆寫）
    write_state(root, world_id, &state)?;
    let _ = super::super::world_file::commit_world_remove(&transcript_path(root, world_id, scene)?);
    let _ = message_vars::drop_scene_seed(tx, scene);
    message_vars::refresh_cache(tx);
    Ok(previous_scene)
}

/// 重寫目前這幕唯一那則摘要：摘要不滿意可以直接原地覆寫，不必先退回再重新換幕一次。
pub fn replace_scene_summary(
    root: &Path,
    world_id: &str,
    summary_text: &str,
    title: Option<&str>,
) -> DataResult<()> {
    let replaced = with_commit(root, world_id, |_| {
        replace_scene_summary_locked(root, world_id, summary_text, title)
    });
    crate::data::bump_capacity_epoch();
    replaced
}

fn replace_scene_summary_locked(
    root: &Path,
    world_id: &str,
    summary_text: &str,
    title: Option<&str>,
) -> DataResult<()> {
    let mut state = read_state(root, world_id)?;
    let scene = state.current_scene;
    let label = scene_label(&state, scene);
    let Some(previous_scene) = label.parent else {
        return Err(UiMsg::SummaryFirstScene.into_error());
    };
    // 分岔幕開頭那則是複製來的真實對話，不是摘要。源頭幕剛好只有一則時
    // 「只有一則」這道守門會放行，覆寫下去就把玩家的對話換成摘要了。
    if label.forked {
        return Err(UiMsg::SummaryContinuedScene.into_error());
    }
    let ts = local_timestamp()?;
    // 判斷與覆寫在同一把逐字稿鎖內：判完到寫回之間不會插進新的一句
    super::super::world_file::with_file_lock(
        &transcript_path(root, world_id, scene)?,
        |file| -> DataResult<()> {
            let mut events = match file.read()? {
                Some(bytes) => parse_transcript(&bytes)?,
                None => Vec::new(),
            };
            if events.len() != 1 {
                return Err(UiMsg::SummaryHasNewContent.into_error());
            }
            // 重寫的只有文字，其餘欄位原樣留著——尤其 state 那份快照：
            // 摘要是這一幕唯一一則，快照掉了之後退回這一幕會把狀態欄清成空的。
            let event = &mut events[0];
            event.text = summary_text.to_owned();
            event.marker = Some(EventMarker::SceneSummary);
            event.ts = ts;
            file.write_atomic(format!("{}\n", serde_json::to_string(event)?).as_bytes())
        },
    )?;

    match title.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => {
            state
                .scene_titles
                .insert(previous_scene.to_string(), name.to_owned());
        }
        None => {
            state.scene_titles.remove(&previous_scene.to_string());
        }
    }
    write_state(root, world_id, &state)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::test_support::*;
    use crate::data::*;
    use std::collections::BTreeMap;

    /// 接管桌判定（換幕摘要兩入口與出場收尾共用）：非空骨架＋非 characters 才算
    #[test]
    fn interface_takeover_needs_nonblank_shell_and_non_character_mode() {
        let root = TestRoot::new("interface-takeover");
        let world_id = create_world(root.path(), "接管判定桌").unwrap();
        assert!(!is_interface_takeover(root.path(), &world_id, None));
        write_interface_shell(root.path(), &world_id, " \n").unwrap();
        assert!(!is_interface_takeover(root.path(), &world_id, None));
        write_interface_shell(root.path(), &world_id, "<UI>{{本回合.正文}}</UI>").unwrap();
        assert!(is_interface_takeover(root.path(), &world_id, None));
        assert!(is_interface_takeover(
            root.path(),
            &world_id,
            Some("interface")
        ));
        assert!(!is_interface_takeover(
            root.path(),
            &world_id,
            Some("characters")
        ));
    }

    #[test]
    fn begin_next_scene_appends_summary_and_advances_scene() {
        let root = TestRoot::new("begin-next-scene");
        let world_id = create_world(root.path(), "換場桌").unwrap();
        let event = TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            raw: None,
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "第一場的對話".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        };
        append_transcript(root.path(), &world_id, 0, &event).unwrap();

        let next = begin_next_scene(root.path(), &world_id, "壓縮後的摘要", None).unwrap();
        assert_eq!(next, 1);
        assert_eq!(read_state(root.path(), &world_id).unwrap().current_scene, 1);

        // 摘要落在新場景檔開頭，舊場景不受影響
        assert_eq!(read_transcript(root.path(), &world_id, 0).unwrap().len(), 1);
        let new_scene = read_transcript(root.path(), &world_id, 1).unwrap();
        assert_eq!(new_scene.len(), 1);
        assert_eq!(new_scene[0].speaker_name, "GM");
        assert_eq!(new_scene[0].speaker_id, "");
        assert_eq!(new_scene[0].kind, TranscriptKind::Narration);
        // 只存本文＋代碼，標頭在顯示／匯出／送 AI 時照語系組
        assert_eq!(new_scene[0].text, "壓縮後的摘要");
        assert_eq!(new_scene[0].marker, Some(EventMarker::SceneSummary));
    }

    #[test]
    fn begin_next_scene_stores_title_on_old_scene_when_given() {
        let root = TestRoot::new("begin-next-scene-title");
        let world_id = create_world(root.path(), "取名桌").unwrap();
        let event = TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            raw: None,
            ts: "2026-07-24T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "第一幕的對話".to_owned(),
            state: None,
            truncated: false,
            gm_only: false,
            marker: None,
            opening: false,
        };
        append_transcript(root.path(), &world_id, 0, &event).unwrap();

        begin_next_scene(root.path(), &world_id, "摘要", Some("酒館夜話")).unwrap();
        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.current_scene, 1);
        assert_eq!(
            state.scene_titles.get("0").map(String::as_str),
            Some("酒館夜話")
        );
        assert!(!state.scene_titles.contains_key("1"));

        // 空字串／None 都不進表
        begin_next_scene(root.path(), &world_id, "摘要二", Some("   ")).unwrap();
        begin_next_scene(root.path(), &world_id, "摘要三", None).unwrap();
        let state = read_state(root.path(), &world_id).unwrap();
        assert!(!state.scene_titles.contains_key("1"));
        assert!(!state.scene_titles.contains_key("2"));
    }

    #[test]
    fn revert_scene_returns_to_previous_scene_and_drops_title() {
        let root = TestRoot::new("revert-scene");
        let world_id = create_world(root.path(), "退幕桌").unwrap();
        let snapshot = TableState {
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
                action_id: None,
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "第一幕的對話".to_owned(),
                state: Some(snapshot.clone()),
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "摘要", Some("酒館夜話")).unwrap();
        assert_eq!(read_state(root.path(), &world_id).unwrap().current_scene, 1);

        let previous = revert_scene(root.path(), &world_id).unwrap();
        assert_eq!(previous, 0);

        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.current_scene, 0);
        // 前幕最後一則帶快照事件的 state 要跟著回來，不是砍完就放著預設值
        assert_eq!(state.state, snapshot);
        assert!(!state.scene_titles.contains_key("0"));
        assert!(!root
            .path()
            .join(format!("worlds/{world_id}/transcript/1.jsonl"))
            .exists());
        // 舊幕本身完全沒被動過
        assert_eq!(read_transcript(root.path(), &world_id, 0).unwrap().len(), 1);
    }

    #[test]
    fn revert_scene_rejects_extra_events_without_touching_anything() {
        let root = TestRoot::new("revert-scene-blocked");
        let world_id = create_world(root.path(), "退幕擋桌").unwrap();
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
                action_id: None,
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "序幕".to_owned(),
                state: None,
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();
        // 這一幕除了摘要之外，玩家已經多說了一句——不是「剛好一則」了
        append_transcript(
            root.path(),
            &world_id,
            1,
            &TranscriptEvent {
                id: None,
                message_vars: None,
                vars_rev: None,
                vars_epoch: None,
                turn_key: None,
                action_id: None,
                raw: None,
                ts: "2026-08-06T00:01:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "新的一句".to_owned(),
                state: None,
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
        )
        .unwrap();

        let before_state = read_state(root.path(), &world_id).unwrap();
        let before_events = read_transcript(root.path(), &world_id, 1).unwrap();

        let error = revert_scene(root.path(), &world_id)
            .unwrap_err()
            .to_string();
        assert!(error == UiMsg::SceneRewindHasNewContent.to_string());

        // 擋下時檔案與 state 都沒被動過
        assert_eq!(read_state(root.path(), &world_id).unwrap(), before_state);
        assert_eq!(
            read_transcript(root.path(), &world_id, 1).unwrap(),
            before_events
        );
    }

    #[test]
    fn revert_scene_rejects_at_first_scene() {
        let root = TestRoot::new("revert-scene-first");
        let world_id = create_world(root.path(), "第一幕桌").unwrap();
        let error = revert_scene(root.path(), &world_id)
            .unwrap_err()
            .to_string();
        assert!(error == UiMsg::SceneFirstNoPrevious.to_string());
    }

    #[test]
    fn replace_scene_summary_overwrites_text_and_drops_title_when_none() {
        let root = TestRoot::new("replace-scene-summary");
        let world_id = create_world(root.path(), "重寫摘要桌").unwrap();
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
                action_id: None,
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "序幕".to_owned(),
                state: None,
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "舊摘要", Some("舊標題")).unwrap();
        assert_eq!(
            read_state(root.path(), &world_id)
                .unwrap()
                .scene_titles
                .get("0")
                .map(String::as_str),
            Some("舊標題")
        );

        replace_scene_summary(root.path(), &world_id, "新摘要", None).unwrap();

        let events = read_transcript(root.path(), &world_id, 1).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].text, "新摘要");
        assert_eq!(events[0].marker, Some(EventMarker::SceneSummary));
        assert_eq!(events[0].speaker_name, "GM");
        assert_eq!(events[0].kind, TranscriptKind::Narration);

        // title 傳 None：舊幕名被移除，不留上一次的殘留
        assert!(!read_state(root.path(), &world_id)
            .unwrap()
            .scene_titles
            .contains_key("0"));
    }

    /// 分岔幕開頭那則是複製來的真實對話，不是前情提要。源頭幕剛好只有一則時，
    /// 「這幕只有一則」那道守門會放行——沒有 forked 這一格擋著，重寫就把玩家的對話覆寫掉了。
    #[test]
    fn replace_scene_summary_refuses_a_forked_scene() {
        let root = TestRoot::new("replace-summary-forked");
        let world_id = create_world(root.path(), "分岔守門桌").unwrap();
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
                action_id: None,
                raw: None,
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "玩家的第一句".to_owned(),
                state: None,
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "第一幕摘要", None).unwrap();

        // 幕 0 只有一則，分岔出來的這一幕同樣只有那一則——正是守門會誤放的形狀
        let forked = fork_scene(root.path(), &world_id, 0).unwrap();
        assert_eq!(
            read_transcript(root.path(), &world_id, forked)
                .unwrap()
                .len(),
            1
        );

        assert!(replace_scene_summary(root.path(), &world_id, "不該蓋掉", None).is_err());
        let events = read_transcript(root.path(), &world_id, forked).unwrap();
        assert_eq!(events[0].text, "玩家的第一句");
    }

    /// 重寫摘要只換文字：那則的狀態快照要留著。摘要是這一幕唯一一則，
    /// 快照掉了的話，之後退回這一幕就只能把狀態欄清成空的。
    #[test]
    fn replace_scene_summary_keeps_snapshot_for_later_revert() {
        let root = TestRoot::new("replace-scene-summary-snapshot");
        let world_id = create_world(root.path(), "快照保留桌").unwrap();
        let snapshot = TableState {
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
                action_id: None,
                raw: None,
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: "序幕".to_owned(),
                state: Some(snapshot.clone()),
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
        )
        .unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.state = snapshot.clone();
        write_state(root.path(), &world_id, &state).unwrap();

        begin_next_scene(root.path(), &world_id, "舊摘要", None).unwrap();
        replace_scene_summary(root.path(), &world_id, "新摘要", None).unwrap();
        // 再換一幕：這時第 1 幕那則摘要成了回推狀態的唯一來源
        begin_next_scene(root.path(), &world_id, "第二幕摘要", None).unwrap();

        assert_eq!(revert_scene(root.path(), &world_id).unwrap(), 1);
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, snapshot);
    }

    /// 驗收劇本：原線三幕分岔、續玩換幕、退回吃 parent、再分岔看 version 疊加。
    #[test]
    fn fork_scene_copies_history_and_relabels_through_continue_and_revert() {
        let root = TestRoot::new("fork-scene-scenario");
        let world_id = create_world(root.path(), "分岔桌").unwrap();

        // 原線三幕（內部 0、1、2），人在幕 2
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
                action_id: None,
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: "船長代碼".to_owned(),
                speaker_name: "船長".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "啟航前的最後一夜。".to_owned(),
                state: None,
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "第一幕摘要", None).unwrap();
        begin_next_scene(root.path(), &world_id, "第二幕摘要", None).unwrap();
        assert_eq!(read_state(root.path(), &world_id).unwrap().current_scene, 2);

        let scene0_before = read_transcript(root.path(), &world_id, 0).unwrap();
        let scene1_before = read_transcript(root.path(), &world_id, 1).unwrap();
        let scene2_before = read_transcript(root.path(), &world_id, 2).unwrap();

        // 從幕 0 分岔
        let forked = fork_scene(root.path(), &world_id, 0).unwrap();
        assert_eq!(forked, 3);
        // 分岔重新配發事件 id（計畫 8.4），其餘逐欄相同
        let copied = read_transcript(root.path(), &world_id, 3).unwrap();
        assert!(copied
            .iter()
            .zip(&scene0_before)
            .all(|(copy, original)| copy.id.is_some() && copy.id != original.id));
        let without_ids = |events: Vec<TranscriptEvent>| {
            events
                .into_iter()
                .map(|mut event| {
                    event.id = None;
                    event
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(without_ids(copied), without_ids(scene0_before.clone()));
        // 舊幕一個字都沒被動過
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap(),
            scene0_before
        );
        assert_eq!(
            read_transcript(root.path(), &world_id, 1).unwrap(),
            scene1_before
        );
        assert_eq!(
            read_transcript(root.path(), &world_id, 2).unwrap(),
            scene2_before
        );

        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.current_scene, 3);
        assert_eq!(
            state.scene_labels.get("3").copied(),
            Some(SceneLabel {
                base: 0,
                version: 2,
                parent: Some(2),
                forked: true
            })
        );

        // 在幕 3 續玩一句，再換幕
        append_transcript(
            root.path(),
            &world_id,
            3,
            &TranscriptEvent {
                id: None,
                message_vars: None,
                vars_rev: None,
                vars_epoch: None,
                turn_key: None,
                action_id: None,
                raw: None,
                ts: "2026-08-06T00:01:00Z".to_owned(),
                speaker_id: "船長代碼".to_owned(),
                speaker_name: "船長".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "這次我們往南走。".to_owned(),
                state: None,
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
        )
        .unwrap();
        let advanced =
            begin_next_scene(root.path(), &world_id, "分岔後摘要", Some("南航夜話")).unwrap();
        assert_eq!(advanced, 4);

        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(
            state.scene_labels.get("4").copied(),
            Some(SceneLabel {
                base: 1,
                version: 2,
                parent: Some(3),
                forked: false
            })
        );
        assert_eq!(
            state.scene_titles.get("3").map(String::as_str),
            Some("南航夜話")
        );

        // 退回幕 4：回到 parent（3），不是算術上的 4-1=3 巧合——這裡故意驗證的是「回到分岔前所在幕」
        let reverted = revert_scene(root.path(), &world_id).unwrap();
        assert_eq!(reverted, 3);
        assert!(!root
            .path()
            .join(format!("worlds/{world_id}/transcript/4.jsonl"))
            .exists());
        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.current_scene, 3);
        assert!(!state.scene_titles.contains_key("3"));
        assert!(!state.scene_labels.contains_key("4"));

        // 再從幕 0 分岔一次：幕 0 與幕 3 都是 base 0，這次該排第 3 個版本
        let forked_again = fork_scene(root.path(), &world_id, 0).unwrap();
        assert_eq!(forked_again, 4);
        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(
            state.scene_labels.get("4").copied(),
            Some(SceneLabel {
                base: 0,
                version: 3,
                parent: Some(3),
                forked: true
            })
        );
    }

    #[test]
    fn fork_scene_rejects_current_or_future_scene() {
        let root = TestRoot::new("fork-scene-rejects-current");
        let world_id = create_world(root.path(), "分岔擋桌").unwrap();
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
                action_id: None,
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "序幕".to_owned(),
                state: None,
                truncated: false,
                gm_only: false,
                marker: None,
                opening: false,
            },
        )
        .unwrap();

        // from_scene == current_scene：還沒換幕，不能從自己這幕分岔
        let error = fork_scene(root.path(), &world_id, 0)
            .unwrap_err()
            .to_string();
        assert!(error == UiMsg::SceneForkNotEarlier.to_string());

        // from_scene > current_scene：幕號還沒出現過
        let error = fork_scene(root.path(), &world_id, 5)
            .unwrap_err()
            .to_string();
        assert!(error == UiMsg::SceneForkNotEarlier.to_string());
    }

    #[test]
    fn fork_scene_rejects_a_scene_with_no_events() {
        let root = TestRoot::new("fork-scene-rejects-empty");
        let world_id = create_world(root.path(), "分岔空幕桌").unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.current_scene = 1; // 幕 0 從沒寫過任何事件，模擬空幕
        write_state(root.path(), &world_id, &state).unwrap();

        let error = fork_scene(root.path(), &world_id, 0)
            .unwrap_err()
            .to_string();
        assert!(error == UiMsg::SceneNothingToContinue.to_string());
    }

    #[test]
    fn scene_label_falls_back_to_original_line_for_unlabeled_scene() {
        let root = TestRoot::new("scene-label-fallback");
        let world_id = create_world(root.path(), "原線桌").unwrap();
        let state = read_state(root.path(), &world_id).unwrap();

        assert_eq!(
            scene_label(&state, 5),
            SceneLabel {
                base: 5,
                version: 1,
                parent: Some(4),
                forked: false
            }
        );
        // 幕 0 沒有前幕：fallback 的 parent 也要是 None，跟 revert_scene 的邊界檢查對得起來
        assert_eq!(
            scene_label(&state, 0),
            SceneLabel {
                base: 0,
                version: 1,
                parent: None,
                forked: false
            }
        );
    }
}
