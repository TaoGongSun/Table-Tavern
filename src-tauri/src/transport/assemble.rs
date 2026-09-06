use crate::data::{
    CharacterCard, Mechanism, TableState, TranscriptEvent, TranscriptKind, WorldbookEntry,
};

use super::messages::{message, player_fallback_name, push_merged, ChatMessage};

use super::context::{active_worldbook_entries, gm_system_prompt};

use super::state_view::{gm_dynamic_block, StateScope};

use super::turns::{chars_lane_system, chars_lane_turn, system_event_text};

/// 共線組裝（api-shared-lane 包 B）：claude 以外的四條路共用一份**與「這輪是誰」無關**的前綴。
/// system 走 `chars_lane_system`（扮演引擎前言＋全部公開角色卡），歷史裡所有角色台詞一律
/// `assistant` 並帶「名字：」前綴，本輪指定與私設放尾端一則 `user`。前綴因此逐字穩定，
/// 換角色不再整包重算——實測前三條路（api 底線 64、codex 底線 9,984、grok）都是「換角色全滅、
/// 同角色連續全中」，共線把「全滅」那一半救回來。
///
/// role 序列只跟事件種類有關、與角色無關，`push_merged` 的分組因此也與角色無關；
/// CLI 三條攤平後的 `(system, prompt)` 連帶逐字相同（見 `cli::flatten_messages`）。
///
/// 單角色桌（`cards` 只有一張）把**私設**提回 system：只有一個角色不會洩漏，而 system 的
/// 指令權重高於尾端 user。這是唯一的分支，不另立第二條組裝路徑。提上去的只有 `private_md`
/// 這段穩定內容——限定世界書的 keyword 條目與狀態每輪翻動，進 system 會把前綴打散。
///
/// API 無狀態，上一輪注入的私設不會出現在下一輪的 messages 裡，
/// 不需要 claude lane 那套「回合後從 session 檔抹掉」。
#[allow(clippy::too_many_arguments)]
pub fn assemble_shared_messages(
    card: &CharacterCard,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    events: &[TranscriptEvent],
    worldbook: &[WorldbookEntry],
    state: &TableState,
    mechanism: &Mechanism,
    branch: Option<&[String]>,
    lang: &str,
) -> Vec<ChatMessage> {
    let mut system = chars_lane_system(cards, player, worldbook, lang);
    let turn = chars_lane_turn(
        card,
        player,
        events,
        worldbook,
        state,
        mechanism,
        branch,
        lang,
        cards.len() <= 1,
    );
    if let Some(private) = &turn.hoisted_private {
        system.push('\n');
        system.push_str(private);
    }
    let tail = turn.tail;

    let mut messages = vec![message("system", system)];
    for event in events {
        // 台詞一律 assistant＋名字前綴：對白對誰都是同一則，前綴才穩得住
        let (role, line) = match event.kind {
            TranscriptKind::Dialogue => (
                "assistant",
                format!("{}：{}", event.speaker_name, event.text),
            ),
            TranscriptKind::Player => ("user", format!("{}：{}", event.speaker_name, event.text)),
            TranscriptKind::Narration => ("user", format!("（旁白）{}", event.text)),
            TranscriptKind::System => (
                "user",
                format!("（系統）{}", system_event_text(event, true)),
            ),
        };
        push_merged(&mut messages, role, line);
    }
    // 刻意不走 push_merged：本輪指定維持獨立一則，不黏進歷史
    messages.push(message("user", tail.trim_end().to_owned()));
    messages
}

/// 點名時「輪到玩家」的內部代號；前端以它停下 GM 推進回合。
/// 刻意用不可能當人名的字串：玩家卡或某張 NPC 卡都可能就叫「玩家」。
pub const PLAYER_SENTINEL: &str = "__PLAYER__";

/// 組裝 GM 上下文：world.md（只有 GM 看得到）＋全部角色卡（含私有，NewPlan §7.0）
/// ＋公開 transcript。GM 自己的旁白是 assistant，其餘事件是 user。
/// keyword 條目與「目前狀態」放 transcript 尾端獨立 user 訊息（快取友善），
/// constant 條目與角色卡留在 system（穩定且需要高遵循度）。
#[allow(clippy::too_many_arguments)]
pub fn assemble_gm_messages(
    world_md: &str,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    events: &[TranscriptEvent],
    worldbook: &[WorldbookEntry],
    state: &TableState,
    mechanism: &Mechanism,
    scope: &StateScope,
    lang: &str,
) -> Vec<ChatMessage> {
    let user_name = player
        .map(|player| player.name.as_str())
        .unwrap_or_else(|| player_fallback_name(lang));
    // 快取友善（prompt-cache-optimization A）：keyword 條目與「目前狀態」每輪翻動，
    // 移到 transcript 尾端的一則獨立 user 訊息；constant 條目穩定，留在 system。
    let (constant_entries, keyword_entries): (Vec<_>, Vec<_>) =
        active_worldbook_entries(worldbook, events)
            .into_iter()
            .partition(|entry| entry.constant);
    let system = gm_system_prompt(
        world_md,
        cards,
        player,
        &constant_entries,
        user_name,
        mechanism,
        lang,
    );

    let mut messages = vec![message("system", system)];
    for event in events {
        let (role, line) = match event.kind {
            TranscriptKind::Narration => ("assistant", event.text.clone()),
            TranscriptKind::Dialogue | TranscriptKind::Player => {
                ("user", format!("{}：{}", event.speaker_name, event.text))
            }
            TranscriptKind::System => ("user", format!("（系統）{}", event.text)),
        };
        push_merged(&mut messages, role, line);
    }
    let dynamic = gm_dynamic_block(&keyword_entries, state, user_name, mechanism, scope, lang);
    if !dynamic.is_empty() {
        // 刻意不走 push_merged：動態塊維持獨立一則的語意邊界，不黏進最後一則發言
        messages.push(message("user", dynamic));
    }
    messages
}

#[cfg(test)]
mod tests;
