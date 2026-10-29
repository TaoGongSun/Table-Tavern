//! GM 回合提交後的登場紀錄：世界書人物首次在場、自動隱藏角色卡回歸。
use crate::{chat_assembly, data, transport};

/// 這一桌自動隱藏、且未手動封存的角色卡（回合登場檢測用，AI 卡重構包 4b）；
/// 與 chat_assembly::active_cards 互補，present 名單命中就是「回歸」。
/// 自動隱藏中的卡（回歸候選）＋桌上其餘卡的名字（只拿來判折疊歧義）。
pub(super) fn load_hidden_cards(
    root: &std::path::Path,
    world_id: &str,
    strip: bool,
) -> Result<(Vec<data::CharacterCard>, Vec<String>), String> {
    let (hidden, others): (Vec<_>, Vec<_>) = data::list_characters(root, world_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|meta| !meta.stage)
        .partition(|meta| meta.cast_role() == data::CastRole::Hidden);
    let hidden = hidden
        .into_iter()
        .map(|meta| {
            data::read_character(root, world_id, &meta.id)
                .map(|card| chat_assembly::prompt_card(card, strip))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<_, _>>()?;
    Ok((hidden, others.into_iter().map(|meta| meta.name).collect()))
}

/// 登場紀錄：GM 回合提交後的附屬追加帶回合憑證（不交接，排在正文前）；沒有憑證就是一般新事件。
/// `action`：觸發這回合的玩家動作，蓋在事件上供換幕容量預測切段。
fn append_arrival(
    root: &std::path::Path,
    world_id: &str,
    scene: u64,
    event: &data::TranscriptEvent,
    turn: Option<&data::message_vars::TurnTicket>,
    action: Option<&str>,
) -> data::DataResult<u64> {
    let event = data::TranscriptEvent {
        action_id: action.map(str::to_owned),
        ..event.clone()
    };
    match turn {
        Some(ticket) => data::append_within_turn(root, world_id, scene, &event, ticket),
        None => data::append_transcript(root, world_id, scene, &event),
    }
}

/// 世界書人物條目首次在場（AI 卡重構包 4a）：present 名單（缺席就退回本文比對）比對得上、
/// 本幕還沒登場過的 is_person 條目，逐一把全文 append 成一則系統事件；同一人本幕只記一次，
/// 離場不拔。寫檔失敗一律吞掉，登場記錄不該反過來中斷旁白。回傳這輪實際記上的標題清單
/// （成功寫檔才算），供 gm_narrate 回傳給前端本地移區。
/// visibility 非 Public 的條目帶 gm_only=true（包 4b）：這種條目原本就限定 GM 或特定角色
/// 看得到，全文登場事件不能透過 chars 續聊線的共用歷史洩漏給所有角色。
#[allow(clippy::too_many_arguments)]
pub(super) fn record_person_arrivals(
    root: &std::path::Path,
    world_id: &str,
    scene: u64,
    worldbook: &[data::WorldbookEntry],
    card_names: &[String],
    events: &[data::TranscriptEvent],
    present: Option<&str>,
    reply_body: &str,
    fill: transport::NeutralFill<'_>,
    turn: Option<&data::message_vars::TurnTicket>,
    action: Option<&str>,
) -> Vec<String> {
    let already = data::appeared_person_titles(events);
    let arrivals =
        transport::detect_new_arrivals(worldbook, card_names, present, reply_body, &already);
    if arrivals.is_empty() {
        return Vec::new();
    }
    let ts = data::local_timestamp().unwrap_or_default();
    let mut titles = Vec::new();
    for entry in arrivals {
        let (marker, text) = transport::person_arrival(entry, fill);
        let event = data::TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            ts: ts.clone(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: data::TranscriptKind::System,
            text,
            raw: None,
            state: None,
            truncated: false,
            gm_only: !matches!(entry.visibility, data::Visibility::Public),
            marker: Some(marker),
            opening: false,
        };
        if append_arrival(root, world_id, scene, &event, turn, action).is_ok() {
            titles.push(entry.title.clone());
        }
    }
    titles
}

/// 角色卡自動回歸（AI 卡重構包 4b）：present 名單（缺席就退回本文比對）比對得上、
/// 本幕還沒回歸過的 auto_hidden 卡，逐一 append 公開回歸事件；有私設時另 append
/// 一則 gm_only 私設事件（只有 GM 看得到，card-arrival-private-leak）。同一張卡本幕只記一次。鏡射 record_person_arrivals，鍵從世界書 title 換成卡片 name；
/// **不改 auto_hidden 欄位本身**（鐵律：持久欄位只在換幕結算與退回／分岔復原動，見 data::begin_next_scene）。
/// 私設先寫：私設寫失敗就整張跳過、下一輪重試；公開事件寫失敗頂多多一則重複私設，
/// 不會出現「已回歸卻沒有私設」被去重擋住補不回來。
/// 回傳這輪實際記上的卡 id 清單（成功寫檔才算）。
#[allow(clippy::too_many_arguments)]
pub(super) fn record_card_arrivals(
    root: &std::path::Path,
    world_id: &str,
    scene: u64,
    hidden_cards: &[data::CharacterCard],
    other_names: &[String],
    events: &[data::TranscriptEvent],
    present: Option<&str>,
    reply_body: &str,
    fill: transport::NeutralFill<'_>,
    turn: Option<&data::message_vars::TurnTicket>,
    action: Option<&str>,
) -> Vec<String> {
    let already = data::appeared_card_names(events);
    let arrivals = transport::detect_new_card_arrivals(
        hidden_cards,
        other_names,
        present,
        reply_body,
        &already,
    );
    if arrivals.is_empty() {
        return Vec::new();
    }
    let ts = data::local_timestamp().unwrap_or_default();
    let mut ids = Vec::new();
    let system_event =
        |(marker, text): (data::EventMarker, String), gm_only: bool| data::TranscriptEvent {
            id: None,
            message_vars: None,
            vars_rev: None,
            vars_epoch: None,
            turn_key: None,
            action_id: None,
            ts: ts.clone(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: data::TranscriptKind::System,
            text,
            raw: None,
            state: None,
            truncated: false,
            gm_only,
            marker: Some(marker),
            opening: false,
        };
    for card in arrivals {
        if let Some(private) = transport::card_private(card, fill) {
            if append_arrival(
                root,
                world_id,
                scene,
                &system_event(private, true),
                turn,
                action,
            )
            .is_err()
            {
                continue;
            }
        }
        let event = system_event(transport::card_arrival(card, fill), false);
        if append_arrival(root, world_id, scene, &event, turn, action).is_ok() {
            ids.push(card.id.clone());
        }
    }
    ids
}

#[cfg(test)]
#[path = "arrivals_tests.rs"]
mod tests;
