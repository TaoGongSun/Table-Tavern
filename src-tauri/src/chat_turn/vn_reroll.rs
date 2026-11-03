//! VN「換一組」（vn-mode 包 1 §5）：只重生成最後一則 GM 旁白的 `[choices]` 區塊。
//! 獨立一次性呼叫：只重用上下文組裝（世界書唯讀掃描、不落地），不經 `gm_lane_reply`／`lanes::run_turn`，
//! 不提交狀態、不記回合、不動 GM session；只改寫那則事件的 text／raw，事件 id 不變。
use super::vn_script::{parse_event, replace_choices, VnCell, VnRoster, VnSource};
use crate::scene_budget::{summarize::SummaryCaller, PathLimits};
use crate::world_scan::{MacroInputs, Randomness};
use crate::{chat_assembly, data, transport};

fn last_choices(cells: &[VnCell]) -> Option<Vec<String>> {
    cells.iter().rev().find_map(|cell| match cell {
        VnCell::Choices { options } => Some(options.clone()),
        _ => None,
    })
}

/// 換一組要送的請求與改寫目標（同步組好：巨集輸入不能跨 await）。
struct Request {
    messages: Vec<transport::ChatMessage>,
    roster: VnRoster,
    scene: u64,
    event_id: String,
    text: String,
    raw: Option<String>,
}

/// 前置：VN 桌、本幕最後一則故事事件是帶 id 的 GM 旁白且解析出選項；否則回錯、什麼都不寫。
fn request(
    root: &std::path::Path,
    world_id: &str,
    lang: &str,
    path: &PathLimits,
) -> Result<Request, String> {
    let materials = chat_assembly::gm_materials(root, world_id)?;
    if !chat_assembly::vn_table(&materials.state) {
        return Err("VN_REROLL_UNAVAILABLE: not a VN table".to_owned());
    }
    let roster = VnRoster::new(&materials.metas, materials.player.as_ref());
    let target = materials
        .events
        .iter()
        .rev()
        .find(|event| event.kind != data::TranscriptKind::System)
        .filter(|event| event.kind == data::TranscriptKind::Narration)
        .ok_or("VN_REROLL_UNAVAILABLE: last story event is not GM narration")?;
    let event_id = target
        .id
        .clone()
        .ok_or("VN_REROLL_UNAVAILABLE: narration has no id")?;
    let current = last_choices(&parse_event(&target.text, &VnSource::Gm, &roster))
        .ok_or("VN_REROLL_UNAVAILABLE: no choices to replace")?;
    let scene = materials.state.current_scene;
    // 唯讀掃描：計時表只讀不結算、不落地（同換幕量測的讀法）；巨集照實送讀變數，但不落地
    let perspective = data::world_info_store::Perspective::Gm;
    let timed =
        data::world_info_store::read_timed(root, world_id, scene, &perspective).unwrap_or_default();
    let macros = MacroInputs::load(
        root,
        world_id,
        &perspective,
        path.model.clone(),
        path.limits,
        true,
    );
    let scan = chat_assembly::gm_scan(
        &materials,
        &materials.events,
        timed,
        path.budget,
        macros,
        Randomness::Live,
        lang,
    );
    let (scope, _) = chat_assembly::gm_scope(&materials);
    let instruction = transport::vn_reroll_instruction(lang, &current);
    Ok(Request {
        messages: chat_assembly::gm_messages(&materials, &scan, &scope, instruction, lang),
        roster,
        scene,
        event_id,
        text: target.text.clone(),
        raw: target.raw.clone(),
    })
}

/// 成功回新選項；模型沒給出 2 個以上選項＝原組保留並回錯。只改寫那則事件的 text／raw。
pub(crate) async fn reroll_choices<C: SummaryCaller>(
    root: &std::path::Path,
    world_id: &str,
    lang: &str,
    path: &PathLimits,
    caller: &mut C,
) -> Result<Vec<String>, String> {
    // 在途互斥：與回合、另一次換組都不並行（登記在回合同一張在途表），忙碌直接回錯
    let turn_id = format!("vn-reroll-{}", data::new_id());
    let (_guard, _cancel) = crate::inflight::register_turn(world_id, &turn_id);
    if crate::inflight::other_turn_in_flight(world_id, &turn_id) {
        return Err("VN_REROLL_BUSY: a turn is in progress".to_owned());
    }
    let Request {
        messages,
        roster,
        scene,
        event_id,
        text,
        raw,
    } = request(root, world_id, lang, path)?;
    let reply = caller.call(messages).await?;
    let options = last_choices(&parse_event(&reply, &VnSource::Gm, &roster))
        .ok_or("VN_REROLL_FAILED: the reply has no usable choices")?;
    let replaced =
        replace_choices(&text, &options).ok_or("VN_REROLL_UNAVAILABLE: no choices to replace")?;
    let raw = raw.map(|raw| replace_choices(&raw, &options).unwrap_or(raw));
    // 提交閘門內重驗：模型回來前玩家已繼續或這則被改過，就不改歷史
    let written = data::replace_transcript_text(
        root,
        world_id,
        scene,
        &event_id,
        &text,
        &replaced,
        raw.as_deref(),
    );
    match written {
        Ok(true) => Ok(options),
        Ok(false) => Err("VN_REROLL_STALE: the narration changed or is no longer last".to_owned()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
#[path = "vn_reroll_tests.rs"]
mod tests;
