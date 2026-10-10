//! 聊天回合的實送組裝（GM 旁白／推進、角色接話）。`commands::chat` 送出與 `scene_budget` 量容量
//! 共用這裡的每一個入口——量的就是會送的，不另寫一套近似。
use crate::world_scan::{self, Budget, Randomness, ScanRequest, TableBook, Viewer, WorldScan};
use crate::{data, import, mechanism, transport};

/// GM 上下文素材＝world.md＋世界書＋全部角色卡（含私有）＋公開 transcript（NewPlan §7.0）。
/// 單發組裝與 claude lane 續聊共用同一份素材。
#[derive(Clone)]
pub(crate) struct GmMaterials {
    pub world_md: String,
    /// 掃描用的整本世界書（觸發看掃描結果，見 `gm_scan`）
    pub book: TableBook,
    pub state: data::WorldState,
    pub events: Vec<data::TranscriptEvent>,
    pub cards: Vec<data::CharacterCard>,
    pub player: Option<data::CharacterCard>,
    /// 這桌全部角色卡的目錄（含封存、隱藏；世界書掃描判「單角色桌」用）
    pub metas: Vec<data::CharacterMeta>,
}

/// 這一桌在場的角色卡（未封存、未自動隱藏）。
pub(crate) fn active_cards(
    root: &std::path::Path,
    world_id: &str,
) -> Result<Vec<data::CharacterCard>, String> {
    data::list_characters(root, world_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|meta| !meta.archived && !meta.auto_hidden)
        .map(|meta| {
            data::read_character(root, world_id, &meta.id).map_err(|error| error.to_string())
        })
        .collect()
}

/// claude 角色線的有效在場集合是否只有開口者本人（plans/claude-resume-tail-cache.md 三-B-1）。
/// 集合＝未封存的卡中「非 auto_hidden」或「本幕已回歸」（回歸只記事件、不改旗標）＋開口者本人。
/// 回歸以名字比對，同名多算只會偏向多角色——寧可多抹，不可漏抹。人數只影響成本，
/// 隔離另由 lanes 的 unerased_owner 保證。
pub(crate) fn sole_present_character(
    root: &std::path::Path,
    world_id: &str,
    speaker_id: &str,
    events: &[data::TranscriptEvent],
) -> Result<bool, String> {
    let metas = data::list_characters(root, world_id).map_err(|error| error.to_string())?;
    Ok(sole_present(&metas, speaker_id, events))
}

pub(crate) fn sole_present(
    metas: &[data::CharacterMeta],
    speaker_id: &str,
    events: &[data::TranscriptEvent],
) -> bool {
    effective_present(metas, events)
        .iter()
        .all(|meta| meta.id == speaker_id)
}

/// 有效在場集合：未封存的卡中「非 auto_hidden」或「本幕已回歸」（回歸只記事件、不改旗標）。
fn effective_present<'a>(
    metas: &'a [data::CharacterMeta],
    events: &[data::TranscriptEvent],
) -> Vec<&'a data::CharacterMeta> {
    let arrived = data::appeared_card_names(events);
    metas
        .iter()
        .filter(|meta| !meta.archived && (!meta.auto_hidden || arrived.contains(&meta.name)))
        .collect()
}

/// 世界書帶名字掃描的「單角色桌」（開場白用卡名）：與 Claude 單人線同一套有效在場集合，恰好一張就回它的名字。
pub(crate) fn sole_card_name(
    metas: &[data::CharacterMeta],
    events: &[data::TranscriptEvent],
) -> Option<String> {
    match effective_present(metas, events).as_slice() {
        [only] => Some(only.name.clone()),
        _ => None,
    }
}

pub(crate) fn gm_materials(root: &std::path::Path, world_id: &str) -> Result<GmMaterials, String> {
    let mut state = data::read_state(root, world_id).map_err(|error| error.to_string())?;
    // 數字欄更新策略算這一次：提示詞與這一輪的提交（turn.commit）都用這一份
    state.mechanism.numeric_update = mechanism::resolve_numeric_update(root, world_id, &state);
    let events = data::read_transcript(root, world_id, state.current_scene)
        .map_err(|error| error.to_string())?;
    Ok(GmMaterials {
        world_md: data::read_world_md(root, world_id).map_err(|error| error.to_string())?,
        book: TableBook::load(root, world_id)?,
        state,
        events,
        cards: active_cards(root, world_id)?,
        metas: data::list_characters(root, world_id).map_err(|error| error.to_string())?,
        player: data::read_player_card(root, world_id).map_err(|error| error.to_string())?,
    })
}

/// GM 視角掃一次世界書（方案三之 2）。`events` 是要掃的本幕事件（量測的固定部分傳空的）；
/// `timed` 是 GM 的計時表；實送用 `Randomness::Live`、量測用 `Randomness::Measure`。
pub(crate) fn gm_scan(
    materials: &GmMaterials,
    events: &[data::TranscriptEvent],
    timed: crate::world_info::timed::WiTimed,
    budget: Option<Budget>,
    random: Randomness,
    lang: &str,
) -> WorldScan {
    world_scan::scan(ScanRequest {
        book: &materials.book,
        viewer: Viewer::Gm,
        sole_card: sole_card_name(&materials.metas, events).as_deref(),
        player: materials.player.as_ref(),
        events,
        lang,
        timed,
        budget,
        random,
    })
}

/// 角色 X 視角掃一次世界書：條目池＝`Public`＋名單含 X 的限定條目，計時用 X 自己那份（P1）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn character_scan(
    book: &TableBook,
    card: &data::CharacterCard,
    sole_card: Option<&str>,
    player: Option<&data::CharacterCard>,
    events: &[data::TranscriptEvent],
    timed: crate::world_info::timed::WiTimed,
    budget: Option<Budget>,
    random: Randomness,
    lang: &str,
) -> WorldScan {
    world_scan::scan(ScanRequest {
        book,
        viewer: Viewer::Character(card),
        sole_card,
        player,
        events,
        lang,
        timed,
        budget,
        random,
    })
}

/// GM 回合的狀態可見範圍：換幕後第一輪送整棵樹對齊，之後每輪只送在場分支＋變動標記（狀態欄二期包 5）。
/// 回傳 (scope, 這輪是不是對齊輪)。
pub(crate) fn gm_scope(materials: &GmMaterials) -> (transport::StateScope, bool) {
    let align = materials.state.mechanism.incremental
        && materials.state.aligned_scene != Some(materials.state.current_scene);
    let scope = transport::state_scope(
        &materials.state.state,
        &materials.state.mechanism,
        &materials.cards,
        materials.player.as_ref(),
        &materials.state.branch_bindings,
        align,
    );
    (scope, align)
}

/// 這一輪 GM 回合尾的導演指示與收尾句（四態見 transport::GmTurnFormat）：
/// - 卡片自帶介面、還沒被 App 接管：讓路給卡片自己規定的輸出格式——格式條目全文真的在本輪
///   提示裡才點名它（CardFormat），否則用不指向缺席格式的中性版（CardFormatAbsent）；
/// - 介面已由 App 接管（桌上有重構產的介面骨架）：正文＋只寫變動的 `<UpdateVariable>`，不要 ```state
///   圍欄——再叫它照卡片格式就是要它每回合重印整份狀態區塊，要圍欄又和 system 的增量協定互斥
///   （refactor-statusbar-skeleton 實測都踩過）；
/// - 其餘：一般旁白＋```state 圍欄。
pub(crate) fn gm_turn_instruction(
    root: &std::path::Path,
    world_id: &str,
    materials: &GmMaterials,
    scan: &WorldScan,
    roster: &[String],
    player_name: Option<&str>,
    lang: &str,
) -> (transport::ChatMessage, &'static str) {
    let card_scripts: Vec<import::InterfaceScript> = import::read_card_interfaces(root, world_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|interface| interface.unsupported.is_none())
        .flat_map(|interface| interface.scripts)
        .collect();
    let has_interface_shell = data::read_interface_shell(root, world_id)
        .ok()
        .flatten()
        .is_some_and(|shell| !shell.trim().is_empty());
    match transport::gm_turn_format(
        !card_scripts.is_empty(),
        has_interface_shell,
        materials.state.refactor_mode.as_deref(),
    ) {
        transport::GmTurnFormat::CardFormat | transport::GmTurnFormat::CardFormatAbsent => {
            // 只點名全文真的進得了本輪提示的格式條目；找不到就用中性版（指示與收尾句同一判定）
            let prompt_entries = transport::gm_prompt_full_entries(scan, &materials.events);
            let entry_title = import::card_format_entry(&card_scripts, &prompt_entries);
            transport::card_format_turn(lang, entry_title.as_deref())
        }
        transport::GmTurnFormat::InterfaceTakeover => {
            let instruction_message = transport::takeover_instruction(lang, roster, player_name);
            let closing = transport::gm_closing(
                transport::GmTurnFormat::InterfaceTakeover,
                !roster.is_empty(),
                lang,
            );
            (instruction_message, closing)
        }
        transport::GmTurnFormat::Narration => {
            let instruction_message = transport::narrate_instruction(lang, roster, player_name);
            let closing =
                transport::gm_closing(transport::GmTurnFormat::Narration, !roster.is_empty(), lang);
            (instruction_message, closing)
        }
    }
}

/// GM 回合的指示與收尾（含名冊與玩家名，旁白與推進共用）。
pub(crate) fn gm_instruction(
    root: &std::path::Path,
    world_id: &str,
    materials: &GmMaterials,
    scan: &WorldScan,
    lang: &str,
) -> (transport::ChatMessage, &'static str) {
    let roster: Vec<String> = materials
        .cards
        .iter()
        .map(|card| card.name.clone())
        .collect();
    let player_name = materials.player.as_ref().map(|card| card.name.as_str());
    gm_turn_instruction(root, world_id, materials, scan, &roster, player_name, lang)
}

/// GM lane 的一輪：凍結 system（GM 指示＋world.md＋穩定觸發的世界書＋全卡）＋回合尾段
/// （其餘觸發條目＋狀態＋導演指示）。回傳 (凍結 system, 回合尾段)。
pub(crate) fn gm_lane_parts(
    materials: &GmMaterials,
    scan: &WorldScan,
    scope: &transport::StateScope,
    instruction: &str,
    lang: &str,
) -> (String, String) {
    let frozen = transport::gm_lane_system(
        &materials.world_md,
        &materials.cards,
        materials.player.as_ref(),
        scan,
        &materials.state.mechanism,
        lang,
    );
    let turn = transport::gm_lane_turn(
        scan,
        materials.player.as_ref(),
        &materials.state.state,
        &materials.state.mechanism,
        scope,
        instruction,
        lang,
    );
    (frozen, turn.tail)
}

/// GM 無狀態路徑（API／codex）的訊息：共線組裝＋本輪指示。
pub(crate) fn gm_messages(
    materials: &GmMaterials,
    scan: &WorldScan,
    scope: &transport::StateScope,
    instruction_message: transport::ChatMessage,
    lang: &str,
) -> Vec<transport::ChatMessage> {
    let mut messages = transport::assemble_gm_messages(
        &materials.world_md,
        &materials.cards,
        materials.player.as_ref(),
        &materials.events,
        scan,
        &materials.state.state,
        &materials.state.mechanism,
        scope,
        lang,
    );
    messages.push(instruction_message);
    messages
}

/// 角色 lane 的一輪：凍結 system（不抹尾段的線——Agy 一角一線、單人 Claude——把私設與本輪世界書提進來）
/// ＋回合尾段（含共線的機密段）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn character_lane_parts(
    card: &data::CharacterCard,
    cards: &[data::CharacterCard],
    player: Option<&data::CharacterCard>,
    scan: &WorldScan,
    snapshot: &[world_scan::Placed],
    state: &data::WorldState,
    branch: Option<&[String]>,
    lang: &str,
    hoist: bool,
) -> (String, transport::LaneTurn) {
    let mut frozen = transport::chars_lane_system(cards, player, snapshot, lang);
    let turn = transport::chars_lane_turn(
        card,
        player,
        scan,
        &state.state,
        &state.mechanism,
        branch,
        lang,
        match hoist {
            true => transport::Hoist::All,
            false => transport::Hoist::None,
        },
    );
    if let Some(private) = &turn.hoisted_private {
        frozen.push('\n');
        frozen.push_str(private);
    }
    // 不抹尾段的線：system 裡的世界書（共用快照＋本輪）一變就要重開，指紋涵蓋全部
    let mut turn = turn;
    if hoist {
        turn.hoisted_worldbook = Some(transport::system_worldbook(
            snapshot,
            player,
            turn.hoisted_worldbook.as_deref(),
            lang,
        ));
    }
    (frozen, turn)
}

/// 測試用：GM 視角照量測的方式掃一次（讀計時表不結算、機率視為通過、沒有上限）。
#[cfg(test)]
pub(crate) fn test_gm_scan(
    root: &std::path::Path,
    world_id: &str,
    materials: &GmMaterials,
    lang: &str,
) -> WorldScan {
    let timed = data::world_info_store::read_timed(
        root,
        world_id,
        materials.state.current_scene,
        &data::world_info_store::Perspective::Gm,
    )
    .unwrap_or_default();
    gm_scan(
        materials,
        &materials.events,
        timed,
        None,
        Randomness::Measure,
        lang,
    )
}

/// 測試用：角色視角照量測的方式掃一次。
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn test_character_scan(
    root: &std::path::Path,
    world_id: &str,
    card: &data::CharacterCard,
    player: Option<&data::CharacterCard>,
    events: &[data::TranscriptEvent],
    lang: &str,
) -> (WorldScan, Vec<world_scan::Placed>) {
    let book = TableBook::load(root, world_id).unwrap();
    let scene = data::read_state(root, world_id).unwrap().current_scene;
    let timed = data::world_info_store::read_timed(
        root,
        world_id,
        scene,
        &data::world_info_store::Perspective::Character(card.id.clone()),
    )
    .unwrap_or_default();
    let metas = data::list_characters(root, world_id).unwrap();
    let scan = character_scan(
        &book,
        card,
        sole_card_name(&metas, events).as_deref(),
        player,
        events,
        timed,
        None,
        Randomness::Measure,
        lang,
    );
    (scan, book.snapshot())
}
