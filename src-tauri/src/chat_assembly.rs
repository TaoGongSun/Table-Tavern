//! 聊天回合的實送組裝（GM 旁白／推進、角色接話）。`chat_turn` 送出與 `scene_budget` 量容量
//! 共用這裡的每一個入口——量的就是會送的，不另寫一套近似。
use crate::world_scan::{
    self, Budget, MacroInputs, MacroSession, Randomness, ScanRequest, TableBook, Viewer, WorldScan,
};
use crate::{data, import, mechanism, transport};

pub(crate) mod greeting;
#[cfg(test)]
mod panel_tests;
#[cfg(test)]
mod stage_tests;

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
    /// 介面接管桌（讀殼失敗算否）：導演指示與這一輪套用狀態用同一個判定
    pub takeover: bool,
    /// 送進提示詞的卡剝掉開場白段（`greeting::policy`；回歸事件的卡也用它）
    pub strip_greetings: bool,
    /// GM 送出層的開場白政策（`greeting::policy`；換幕範例已套 promptOnly 正則）
    pub opening: transport::GmOpening,
    /// 卡片 promptOnly 正則（送模型的歷史副本套用；`prompt_regex`）
    pub prompt_regex: transport::PromptRegex,
    /// 多人房間的真人玩家（multiplayer-room 包 3）；一般桌是空的
    pub party: Party,
    /// VN 桌各角色可用的立繪變體（角色名, label）；非 VN 桌為空
    pub vn_variants: Vec<(String, Vec<String>)>,
    /// VN 桌的聖經凍結副本與補種初值（`vn_bible::turn_inputs`）；沒有聖經為空
    pub vn_bible: Option<data::vn::Frozen>,
    pub vn_seeds: Vec<(Vec<String>, String)>,
    /// VN 卡建的桌才有：卡原件（卡定主角、旁白人稱、出場限制）
    pub vn_card: Option<data::vn::CardOrigin>,
}

/// 多人房間的真人玩家（multiplayer-room 包 3，來源 `WorldState::player_card_ids`／`absent_card_ids`）：
/// 在場的照固定順序帶 (卡 id, 卡名)，離席不點名的只帶 id。一般桌兩者皆空，點名與提示照舊。
#[derive(Clone, Default)]
pub(crate) struct Party {
    pub present: Vec<(String, String)>,
    pub absent: Vec<String>,
    /// 當次派工裡把回合交給 GM 的玩家角色名（room-speak-request §2）；`hand_over` 填，預設空
    pub handed: Vec<String>,
}

impl Party {
    pub(crate) fn load(root: &std::path::Path, world_id: &str, state: &data::WorldState) -> Party {
        Party {
            present: state
                .player_card_ids
                .iter()
                .filter_map(|id| {
                    let card = data::read_character(root, world_id, id).ok()?;
                    Some((id.clone(), card.name))
                })
                .collect(),
            // 進房前的單人玩家卡沒被認領時也不點名（退出房間會還原成玩家卡）
            absent: state
                .absent_card_ids
                .iter()
                .chain(&state.solo_player_card_id)
                .filter(|id| !state.player_card_ids.contains(id))
                .cloned()
                .collect(),
            handed: Vec::new(),
        }
    }

    /// 只認 `action_id` 等於當次派工的交給 GM 標記：host 回合的派工沒有行動、id 也不同，名單必定是空的；
    /// retry 與重連續跑是同一個派工，regenerate 會以新派工重落同一批標記
    pub(crate) fn hand_over(&mut self, events: &[data::TranscriptEvent], action_id: Option<&str>) {
        let Some(id) = action_id else {
            return self.handed.clear();
        };
        self.handed = events
            .iter()
            .filter(|event| event.action_id.as_deref() == Some(id))
            .filter_map(|event| match &event.marker {
                Some(data::EventMarker::PlayerHandedToGm { name }) => Some(name.clone()),
                _ => None,
            })
            .collect();
    }

    pub(crate) fn names(&self) -> Vec<String> {
        self.present.iter().map(|(_, name)| name.clone()).collect()
    }

    /// 不進 NPC 點名名單的卡：在場真人玩家與離席不點名的
    fn holds(&self, id: &str) -> bool {
        self.present.iter().any(|(player, _)| player == id) || self.absent.iter().any(|a| a == id)
    }
}

/// 這一桌送進提示詞的卡（發言角色＋舞台卡；不含封存、自動隱藏），照 `prompt_card` 整理成送進提示詞的樣子。
/// 舞台卡的設定照送（GM 與角色線放「## 舞台」段）；誰能發言另由 `speakers` 分流。
pub(crate) fn active_cards(
    root: &std::path::Path,
    world_id: &str,
) -> Result<Vec<data::CharacterCard>, String> {
    active_cards_with(
        root,
        world_id,
        greeting::strip_card_greetings(root, world_id),
    )
}

/// `active_cards`，剝不剝開場白段由呼叫端算好（一輪只算一次，往下傳）。
pub(crate) fn active_cards_with(
    root: &std::path::Path,
    world_id: &str,
    strip: bool,
) -> Result<Vec<data::CharacterCard>, String> {
    data::list_characters(root, world_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|meta| {
            matches!(
                meta.cast_role(),
                data::CastRole::Speaker | data::CastRole::Stage
            )
        })
        .map(|meta| {
            data::read_character(root, world_id, &meta.id)
                .map(|card| prompt_card(card, strip))
                .map_err(|error| error.to_string())
        })
        .collect()
}

/// 集中分流（stage-card-in-speaker-list）：送進提示詞的卡裡，能發言的那些（點名名單、點名解析、
/// 用量記的人數都從這裡取）；舞台卡只提供設定。以 meta 判斷的入口用 `data::CharacterMeta::cast_role`。
pub(crate) fn speakers(
    cards: &[data::CharacterCard],
) -> impl Iterator<Item = &data::CharacterCard> {
    cards.iter().filter(|card| !card.stage)
}

/// 點名名單：發言角色的名字（GM 導演指示的「下一位」候選）。
pub(crate) fn speaker_roster(cards: &[data::CharacterCard]) -> Vec<String> {
    speakers(cards).map(|card| card.name.clone()).collect()
}

/// 送進提示詞（GM、角色線、回歸事件）的卡：`strip` 時拿掉開場白與備用開場白段（政策見 `greeting::policy`）。
/// 卡檔不動。
pub(crate) fn prompt_card(mut card: data::CharacterCard, strip: bool) -> data::CharacterCard {
    if strip {
        let (public, private) = crate::import::strip_greetings(&card.public_md, &card.private_md);
        card.public_md = public;
        card.private_md = private;
    }
    card
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

/// 有效在場集合：發言角色，加上自動隱藏中「本幕已回歸」的（回歸只記事件、不改旗標）。
/// 舞台卡不算在場：「1 角色＋1 舞台」算單角色桌〔模型判斷·未裁決〕。
fn effective_present<'a>(
    metas: &'a [data::CharacterMeta],
    events: &[data::TranscriptEvent],
) -> Vec<&'a data::CharacterMeta> {
    let arrived = data::appeared_card_names(events);
    metas
        .iter()
        .filter(|meta| match meta.cast_role() {
            data::CastRole::Speaker => true,
            data::CastRole::Hidden => arrived.contains(&meta.name),
            data::CastRole::Stage | data::CastRole::Archived => false,
        })
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
    let party = Party::load(root, world_id, &state);
    // 數字欄更新策略算這一次：提示詞與這一輪的提交（turn.commit）都用這一份
    state.mechanism.numeric_update = mechanism::resolve_numeric_update(root, world_id, &state);
    let events = data::read_transcript(root, world_id, state.current_scene)
        .map_err(|error| error.to_string())?;
    let takeover = data::is_interface_takeover(root, world_id, state.refactor_mode.as_deref());
    let mut greeting = greeting::policy(root, world_id, &state, &events, takeover);
    let regex = prompt_regex(root, world_id);
    let cards = active_cards_with(root, world_id, greeting.strip_cards)?;
    let vn_inputs = crate::vn_bible::turn_inputs(root, world_id, &state);
    let vn_variants = match vn_table(&state) {
        true => crate::vn_sprites::variant_roster(root, world_id, &cards, vn_inputs.card.as_ref()),
        false => Vec::new(),
    };
    greeting.gm.example = greeting
        .gm
        .example
        .map(|example| regex.apply_oldest_ai(example));
    Ok(GmMaterials {
        world_md: data::read_world_md(root, world_id).map_err(|error| error.to_string())?,
        book: TableBook::load(root, world_id)?,
        state,
        events,
        cards,
        vn_variants,
        vn_bible: vn_inputs.frozen,
        vn_seeds: vn_inputs.seeds,
        vn_card: vn_inputs.card,
        strip_greetings: greeting.strip_cards,
        opening: greeting.gm,
        prompt_regex: regex,
        metas: data::list_characters(root, world_id).map_err(|error| error.to_string())?,
        player: data::read_player_card(root, world_id).map_err(|error| error.to_string())?,
        takeover,
        party,
    })
}

/// 這桌在場卡（可用、D24 允許 regex）的 promptOnly 正則；讀不到卡當沒有。
pub(crate) fn prompt_regex(root: &std::path::Path, world_id: &str) -> transport::PromptRegex {
    let interfaces = crate::import::read_card_interfaces(root, world_id).unwrap_or_default();
    transport::PromptRegex::from_scripts(
        interfaces
            .iter()
            .filter(|interface| interface.unsupported.is_none())
            .flat_map(|interface| &interface.prompt_scripts),
    )
}

/// 介面接管桌只走 GM 一條敘事線（作者裁決 2026-10-27）：角色回合一律拒收。只讀不寫，
/// 呼叫端放在容量記帳、代落上一輪等任何副作用之前，被拒就不寫樓、不打模型、不扣容量。
pub(crate) fn reject_character_turn_on_takeover(
    root: &std::path::Path,
    world_id: &str,
) -> Result<(), String> {
    let state = data::read_state(root, world_id).map_err(|error| error.to_string())?;
    match data::is_interface_takeover(root, world_id, state.refactor_mode.as_deref()) {
        true => Err(crate::ui_msg::UiMsg::InterfaceTableGmOnly.into()),
        false => Ok(()),
    }
}

/// 舞台卡只提供設定、不發言（stage-card-in-speaker-list）：它的角色回合一律拒收。只讀不寫，呼叫端
/// 與 `reject_character_turn_on_takeover` 放在一起，被拒就不寫樓、不打模型、不扣容量。
pub(crate) fn reject_stage_character_turn(
    root: &std::path::Path,
    world_id: &str,
    character_id: &str,
) -> Result<(), String> {
    let card =
        data::read_character(root, world_id, character_id).map_err(|error| error.to_string())?;
    match card.stage {
        true => Err(crate::ui_msg::UiMsg::StageCardNoTurn.into()),
        false => Ok(()),
    }
}

/// GM 點名對回角色 id：LLM 只認名字，同名取第一個；玩家哨兵原樣回傳。介面接管桌不點名，
/// 模型照習慣寫了點名行（已由 `extract_next_speaker` 剝掉）也一律 None，前端不接力。
pub(crate) fn next_speaker(
    takeover: bool,
    cards: &[data::CharacterCard],
    next_raw: Option<String>,
    player_name: Option<&str>,
) -> Option<String> {
    if takeover {
        return None;
    }
    let roster = speaker_roster(cards);
    let picked = transport::pick_speaker(&next_raw?, &roster, player_name)?;
    if picked == transport::PLAYER_SENTINEL {
        return Some(picked);
    }
    speakers(cards)
        .find(|card| card.name == picked)
        .map(|card| card.id.clone())
}

/// GM 點名對回（旁白回合用）：(前端的 next＝角色 id 或玩家哨兵, 多人桌點到的那位真人玩家卡 id)。
/// 一般桌就是 `next_speaker`、第二格恆 None。多人桌候選＝NPC（不含真人玩家與離席卡）＋在場玩家名，
/// 點到玩家名或哨兵都回哨兵（前端照舊停下），寫了名字的另帶卡 id 給房間決定下一回合。
pub(crate) fn gm_next_speaker(
    materials: &GmMaterials,
    next_raw: Option<String>,
) -> (Option<String>, Option<String>) {
    let party = &materials.party;
    if party.present.is_empty() && party.absent.is_empty() {
        let player = materials.player.as_ref().map(|card| card.name.as_str());
        let next = next_speaker(materials.takeover, &materials.cards, next_raw, player);
        return (next, None);
    }
    let roster = call_roster(materials);
    let picked = match (materials.takeover, next_raw) {
        (false, Some(raw)) => transport::pick_speaker(&raw, &roster, None),
        _ => None,
    };
    let Some(picked) = picked else {
        return (None, None);
    };
    if let Some(card) = npcs(materials).find(|card| card.name == picked) {
        return (Some(card.id.clone()), None);
    }
    let player = party.present.iter().find(|(_, name)| *name == picked);
    (
        Some(transport::PLAYER_SENTINEL.to_owned()),
        player.map(|(id, _)| id.clone()),
    )
}

/// 能被點名的 NPC：發言角色扣掉多人房間的真人玩家與離席卡（一般桌就是 `speakers`）。
fn npcs(materials: &GmMaterials) -> impl Iterator<Item = &data::CharacterCard> {
    speakers(&materials.cards).filter(|card| !materials.party.holds(&card.id))
}

/// GM 導演指示的「下一位」名單：NPC 在前、在場真人玩家在後（一般桌＝`speaker_roster`）。
fn call_roster(materials: &GmMaterials) -> Vec<String> {
    npcs(materials)
        .map(|card| card.name.clone())
        .chain(materials.party.names())
        .collect()
}

/// GM 視角掃一次世界書（方案三之 2）並代換組裝文字（三之 7）。`events` 是要掃的本幕事件（量測的固定部分傳
/// 空的）；`timed` 是 GM 的計時表；實送用 `Randomness::Live`、量測用 `Randomness::Measure`。
pub(crate) fn gm_scan(
    materials: &GmMaterials,
    events: &[data::TranscriptEvent],
    timed: crate::world_info::timed::WiTimed,
    budget: Option<Budget>,
    macros: MacroInputs,
    random: Randomness,
    lang: &str,
) -> WorldScan {
    let viewer = Viewer::Gm;
    let session = MacroSession::new(
        macros,
        &viewer,
        &materials.world_md,
        materials.book.world_card_name(),
        materials.player.as_ref(),
        events,
        lang,
    );
    let mut scan = world_scan::scan(ScanRequest {
        book: &materials.book,
        viewer,
        sole_card: sole_card_name(&materials.metas, events).as_deref(),
        player: materials.player.as_ref(),
        events,
        lang,
        timed,
        budget,
        random,
        session: &session,
    });
    world_scan::prepare(
        &mut scan,
        &session,
        &materials.book,
        &viewer,
        &materials.cards,
        materials.player.as_ref(),
    );
    scan.texts.party = materials
        .party
        .present
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    scan
}

/// 角色 X 視角掃一次世界書並代換組裝文字：條目池＝`Public`＋名單含 X 的限定條目，計時用 X 自己那份（P1）。
/// `cards` 是組裝會列的全部角色卡（共線 system 的那份）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn character_scan(
    book: &TableBook,
    card: &data::CharacterCard,
    cards: &[data::CharacterCard],
    sole_card: Option<&str>,
    player: Option<&data::CharacterCard>,
    events: &[data::TranscriptEvent],
    timed: crate::world_info::timed::WiTimed,
    budget: Option<Budget>,
    macros: MacroInputs,
    random: Randomness,
    lang: &str,
) -> WorldScan {
    let viewer = Viewer::Character(card);
    let session = MacroSession::new(macros, &viewer, "", None, player, events, lang);
    let mut scan = world_scan::scan(ScanRequest {
        book,
        viewer,
        sole_card,
        player,
        events,
        lang,
        timed,
        budget,
        random,
        session: &session,
    });
    world_scan::prepare(&mut scan, &session, book, &viewer, cards, player);
    scan
}

/// GM 視角的中性巨集環境（登場事件等回合提交時寫死的文字用）：讀最新的變數，不記副作用。
pub(crate) fn gm_neutral_session(
    root: &std::path::Path,
    world_id: &str,
    materials: &GmMaterials,
    path: &crate::scene_budget::PathLimits,
    lang: &str,
) -> MacroSession {
    MacroSession::new(
        MacroInputs::load(
            root,
            world_id,
            &data::world_info_store::Perspective::Gm,
            path.model.clone(),
            path.limits,
            true,
        ),
        &Viewer::Gm,
        &materials.world_md,
        materials.book.world_card_name(),
        materials.player.as_ref(),
        &materials.events,
        lang,
    )
}

/// 整桌人物名字：全部角色卡（含隱藏、封存）＋未停用的人物條目，在場比對判折疊歧義用。
fn table_names(materials: &GmMaterials) -> Vec<String> {
    materials
        .metas
        .iter()
        .map(|meta| meta.name.clone())
        .chain(materials.book.person_titles())
        .collect()
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
        &table_names(materials),
    );
    (scope, align)
}

/// 這一輪 GM 回合尾的導演指示與收尾句（四態見 transport::GmTurnFormat）：
/// - 卡片自帶介面（`import::reply_format_scripts` 非空：有會作用在回覆上的顯示腳本，開場白美化不算）、還沒被 App 接管：讓路給卡片自己規定的輸出格式——格式條目全文真的在本輪
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
    let card_scripts: Vec<import::InterfaceScript> = import::reply_format_scripts(root, world_id);
    let format = transport::gm_turn_format(
        vn_table(&materials.state),
        !card_scripts.is_empty(),
        materials.takeover,
        materials.state.refactor_mode.as_deref(),
    );
    let (mut instruction, closing) = match format {
        transport::GmTurnFormat::CardFormat | transport::GmTurnFormat::CardFormatAbsent => {
            // 只點名全文真的進得了本輪提示的格式條目；找不到就用中性版（指示與收尾句同一判定）
            let prompt_entries = transport::gm_prompt_full_entries(scan, &materials.events);
            let entry_title = import::card_format_entry(&card_scripts, &prompt_entries);
            transport::card_format_turn(lang, entry_title.as_deref())
        }
        // 介面接管桌只走 GM 一條敘事線：不點名
        transport::GmTurnFormat::InterfaceTakeover => (
            transport::takeover_instruction(lang),
            transport::gm_closing(transport::GmTurnFormat::InterfaceTakeover, false, lang),
        ),
        transport::GmTurnFormat::VnScript => {
            let updates = materials.state.mechanism.incremental;
            (
                transport::vn_narrate_instruction(
                    lang,
                    roster,
                    player_name,
                    updates,
                    &vn_direction(materials),
                ),
                transport::vn_closing(!roster.is_empty(), updates, lang),
            )
        }
        transport::GmTurnFormat::Narration => {
            // 增量桌：圍欄只管三欄，其餘欄位的變動另附更新區塊
            let updates = materials.state.mechanism.incremental;
            let instruction_message = transport::narrate_instruction(
                lang,
                roster,
                player_name,
                updates,
                materials.state.statusbar_retired,
            );
            let closing = if updates {
                transport::narration_closing_with_updates(!roster.is_empty(), lang)
            } else {
                transport::gm_closing(transport::GmTurnFormat::Narration, !roster.is_empty(), lang)
            };
            (instruction_message, closing)
        }
    };
    // 多人房間：在場真人玩家與本回合行動（一般桌是空字串）
    let call = format == transport::GmTurnFormat::Narration && !roster.is_empty();
    instruction.content.push_str(&transport::party_instruction(
        lang,
        &materials.party.names(),
        call,
        &materials.party.handed,
    ));
    (instruction, closing)
}

/// GM 回合的指示與收尾（含名冊與玩家名，旁白與推進共用）。
pub(crate) fn gm_instruction(
    root: &std::path::Path,
    world_id: &str,
    materials: &GmMaterials,
    scan: &WorldScan,
    lang: &str,
) -> (transport::ChatMessage, &'static str) {
    let roster = call_roster(materials);
    // 多人桌的玩家名已在名單裡、以名字點名；「玩家（某某）」只留給一般桌
    let player_name = match materials.party.present.is_empty() {
        true => materials.player.as_ref().map(|card| card.name.as_str()),
        false => None,
    };
    gm_turn_instruction(root, world_id, materials, scan, &roster, player_name, lang)
}

/// GM lane 的一輪：凍結 system（GM 指示＋world.md＋穩定觸發的世界書＋全卡）＋回合尾段
/// （其餘觸發條目＋狀態＋導演指示）。回傳 (凍結 system, 回合尾段與其中的尾段世界書)。
pub(crate) fn gm_lane_parts(
    materials: &GmMaterials,
    scan: &WorldScan,
    scope: &transport::StateScope,
    instruction: &str,
    lang: &str,
) -> (String, transport::LaneTurn) {
    let mut frozen = transport::gm_lane_system_with(
        &materials.cards,
        materials.player.as_ref(),
        scan,
        &materials.state.mechanism,
        lang,
        gm_voice(materials),
        &materials.opening,
    );
    if vn_table(&materials.state) {
        frozen.push_str(&vn_system(materials, lang));
    }
    let turn = transport::gm_lane_turn(
        scan,
        materials.player.as_ref(),
        &materials.state.state,
        &materials.state.mechanism,
        scope,
        instruction,
        lang,
    );
    (frozen, turn)
}

/// GM 無狀態路徑（API／codex）的訊息：共線組裝＋本輪指示。
pub(crate) fn gm_messages(
    materials: &GmMaterials,
    scan: &WorldScan,
    scope: &transport::StateScope,
    instruction_message: transport::ChatMessage,
    lang: &str,
) -> Vec<transport::ChatMessage> {
    let mut messages = transport::assemble_gm_messages_with(
        &materials.cards,
        materials.player.as_ref(),
        &materials.events,
        scan,
        &materials.state.state,
        &materials.state.mechanism,
        scope,
        lang,
        gm_voice(materials),
        &materials.opening,
        &materials.prompt_regex,
        vn_table(&materials.state),
    );
    if vn_table(&materials.state) {
        messages[0].content.push_str(&vn_system(materials, lang));
    }
    messages.push(instruction_message);
    messages
}

/// VN 桌 GM system 的附加段：劇本格式、立繪變體、聖經（常駐段＋當章包）。
fn vn_system(materials: &GmMaterials, lang: &str) -> String {
    let card = materials.vn_card.as_ref();
    let expressions = crate::vn_sprites::expression_roster(&materials.cards, card);
    let places = card.map_or(&[][..], |card| card.locations.as_slice());
    let mut text = transport::vn_format_rules(lang, !expressions.is_empty(), !places.is_empty());
    text.push_str(&transport::vn_card_note(lang, &expressions, places));
    text.push_str(&transport::vn_variant_note(lang, &materials.vn_variants));
    if let Some(frozen) = &materials.vn_bible {
        text.push_str(&crate::vn_bible::injection(
            lang,
            frozen,
            &materials.cards,
            materials.player.as_ref(),
        ));
    }
    if let Some(card) = &materials.vn_card {
        text.push_str(&transport::vn_bible_prompts::cast_limits(
            lang,
            card,
            &materials.cards,
        ));
    }
    text
}

/// 卡定主角：VN 卡標了固定主角、主角（玩家卡槽位）還在，才換掉總則、玩家段與導演指示（pkg9 §2.5）。
fn vn_protagonist(materials: &GmMaterials) -> Option<&data::CharacterCard> {
    let fixed = vn_table(&materials.state)
        && materials
            .vn_card
            .as_ref()
            .is_some_and(|card| card.protagonist_fixed);
    materials.player.as_ref().filter(|_| fixed)
}

/// GM system 的敘事分工：介面接管桌優先，其次 VN 卡定主角。
pub(crate) fn gm_voice(materials: &GmMaterials) -> transport::GmVoice {
    match (materials.takeover, vn_protagonist(materials)) {
        (true, _) => transport::GmVoice::Sole,
        (false, Some(_)) => transport::GmVoice::Protagonist,
        (false, None) => transport::GmVoice::Shared,
    }
}

/// VN 導演指示的可變部分：凍結聖經的 `branching`、卡給的旁白人稱、卡定主角名。
fn vn_direction(materials: &GmMaterials) -> transport::VnDirection {
    transport::VnDirection {
        branching: materials
            .vn_bible
            .as_ref()
            .map(|frozen| frozen.bible.branching),
        pov: materials.vn_card.as_ref().and_then(|card| card.pov),
        protagonist: vn_protagonist(materials).map(|card| card.name.clone()),
        chapter: Some(vn_chapter_cue(materials)),
    }
}

/// 回合尾的章與結局提示（包 5 §2.4）：章號與收尾狀態讀本回合開始時的樹，結局條件同一份樹求值。
fn vn_chapter_cue(materials: &GmMaterials) -> transport::ChapterCue {
    let tree = &materials.state.state.tree;
    let chapter = data::vn::chapter_in(tree);
    let bible = materials.vn_bible.as_ref().map(|frozen| &frozen.bible);
    transport::ChapterCue {
        chapter,
        max: bible.map_or(data::vn::CHAPTER_FALLBACK, |bible| bible.chapter_max),
        end_condition: bible.map(|bible| {
            (chapter as usize)
                .checked_sub(1)
                .and_then(|index| bible.chapters.get(index))
                .map(|pack| match pack.end_condition.trim().is_empty() {
                    true => pack.goal.trim().to_owned(),
                    false => pack.end_condition.trim().to_owned(),
                })
                .unwrap_or_default()
        }),
        finale: data::vn::finale_in(tree),
        holding: bible
            .map(|bible| {
                bible
                    .endings
                    .iter()
                    .filter(|ending| ending.id != data::vn::DEFAULT_ENDING)
                    .filter(|ending| {
                        ending.when.iter().all(|condition| {
                            crate::mechanism::condition_holds(
                                tree,
                                &materials.state.mechanism,
                                condition,
                            )
                        })
                    })
                    .map(|ending| ending.id.clone())
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// VN 桌（vn-mode）：GM 指示、system 附加段與送模型的歷史前綴都看這個。
pub(crate) fn vn_table(state: &data::WorldState) -> bool {
    state.table_kind == data::TableKind::Vn
}

/// 角色 lane 的一輪：凍結 system（不抹尾段的線——Agy 一角一線、單人 Claude——把私設與本輪世界書提進來）
/// ＋回合尾段（含共線的機密段）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn character_lane_parts(
    card: &data::CharacterCard,
    cards: &[data::CharacterCard],
    player: Option<&data::CharacterCard>,
    scan: &WorldScan,
    state: &data::WorldState,
    branch: Option<&[String]>,
    lang: &str,
    hoist: bool,
) -> (String, transport::LaneTurn) {
    let mut frozen = transport::chars_lane_system(cards, player, scan, lang);
    if vn_table(state) {
        frozen.push_str(&transport::vn_character_rules(lang));
    }
    let turn = transport::chars_lane_turn_with(
        card,
        cards,
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
        vn_table(state),
    );
    if let Some(private) = &turn.hoisted_private {
        frozen.push('\n');
        frozen.push_str(private);
    }
    // 不抹尾段的線：system 裡的世界書部分（共用快照＋本輪世界書與動態公開設定）算指紋，供帳本記 worldbook-changed；
    // 整份 system（含私設）另由 lanes::plan_turn 比對，一變就重開
    let mut turn = turn;
    if hoist {
        turn.hoisted_worldbook = Some(transport::system_worldbook(
            scan,
            turn.hoisted_worldbook.as_deref(),
            lang,
        ));
    }
    (frozen, turn)
}

/// 測試用：GM 視角照量測的方式掃一次（讀計時表不結算、機率視為通過、沒有上限、空變數）。
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
        MacroInputs::empty(),
        Randomness::Measure,
        lang,
    )
}

/// 測試用：角色視角照量測的方式掃一次（組裝的卡清單＝這桌在場的卡）。
#[cfg(test)]
pub(crate) fn test_character_scan(
    root: &std::path::Path,
    world_id: &str,
    card: &data::CharacterCard,
    player: Option<&data::CharacterCard>,
    events: &[data::TranscriptEvent],
    lang: &str,
) -> WorldScan {
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
    let cards = active_cards(root, world_id).unwrap();
    character_scan(
        &book,
        card,
        &cards,
        sole_card_name(&metas, events).as_deref(),
        player,
        events,
        timed,
        None,
        MacroInputs::empty(),
        Randomness::Measure,
        lang,
    )
}

#[cfg(test)]
mod takeover_gate_tests {
    use super::{next_speaker, reject_character_turn_on_takeover};
    use crate::{data, transport};

    /// 一般桌照常點名（角色對回 id、玩家哨兵原樣）；介面接管桌一律不點名
    #[test]
    fn next_speaker_is_none_on_takeover_tables() {
        let fox = data::CharacterCard {
            id: "fox-id".to_owned(),
            name: "狐狸".to_owned(),
            color: "#336699".to_owned(),
            avatar: "🦊".to_owned(),
            tier: data::Tier::Balanced,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: String::new(),
            private_md: String::new(),
            stage: false,
        };
        let cards = [fox];
        let named = || Some("狐狸".to_owned());
        assert_eq!(
            next_speaker(false, &cards, named(), None).as_deref(),
            Some("fox-id")
        );
        assert_eq!(
            next_speaker(
                false,
                &cards,
                Some(transport::PLAYER_SENTINEL.to_owned()),
                None
            )
            .as_deref(),
            Some(transport::PLAYER_SENTINEL)
        );
        assert_eq!(next_speaker(true, &cards, named(), None), None);
        assert_eq!(next_speaker(false, &cards, None, None), None);
    }

    fn world_files(dir: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files.extend(world_files(&path));
            } else {
                files.push((path.clone(), std::fs::read(&path).unwrap()));
            }
        }
        files.sort();
        files
    }

    /// 接管桌拒收角色回合且不動任何檔；一般桌與角色優先桌照常放行
    #[test]
    fn character_turn_rejected_on_takeover_without_side_effects() {
        let root = crate::smart_free::test_root("takeover-gate");
        let world_id = data::create_world(&root, "接管桌").unwrap();
        assert!(reject_character_turn_on_takeover(&root, &world_id).is_ok());
        data::write_interface_shell(&root, &world_id, "<UI>{{本回合.正文}}</UI>").unwrap();
        let before = world_files(&root);
        let error = reject_character_turn_on_takeover(&root, &world_id).unwrap_err();
        assert!(error.contains("interface_table_gm_only"), "{error}");
        assert_eq!(world_files(&root), before);
        let mut state = data::read_state(&root, &world_id).unwrap();
        state.refactor_mode = Some("characters".to_owned());
        data::write_state(&root, &world_id, &state).unwrap();
        assert!(reject_character_turn_on_takeover(&root, &world_id).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }
}
