//! 量實際組裝（計畫 §3.5）：每條路徑都呼叫與實送相同的組裝函式，只是不送出。
//! 固定輸入 F＝同一組裝把事件清空後的量，H＝整包 − F，F 只算一次。
use super::estimate::{budget_tokens, measure, Unit};
use crate::chat_assembly::{self, GmMaterials};
use crate::cli;
use crate::data::{self, CharacterCard, TranscriptEvent};
use crate::lanes::{self, Lane, LaneProvider};
use crate::transport::{self, ChatMessage, Side};
use crate::world_scan::{Budget, Randomness, WorldScan};

/// 一次請求在某單位下的原始量（尚未乘校正）：整包與事件清空後的固定部分。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Raw {
    pub full: u64,
    pub fixed: u64,
}

/// 請求的形狀：CLI 攤平成 (system, prompt)；API 是訊息陣列。
pub enum Request {
    Cli { system: String, prompt: String },
    Api(Vec<ChatMessage>),
}

/// 一則 API 訊息的包裝開銷（role 與分隔），保守取 4 token
const API_MESSAGE_OVERHEAD: u64 = 4;

pub fn size(request: &Request, unit: Unit) -> u64 {
    match (request, unit) {
        // agy 截尾看的就是 stdin 那份（與實送同一個函式組出來）
        (Request::Cli { system, prompt }, Unit::Bytes) => {
            cli::agy_body(system, prompt).len() as u64
        }
        (Request::Cli { system, prompt }, Unit::Tokens) => {
            budget_tokens(system) + budget_tokens(prompt)
        }
        (Request::Api(messages), Unit::Tokens) => messages
            .iter()
            .map(|message| budget_tokens(&message.content) + API_MESSAGE_OVERHEAD)
            .sum(),
        (Request::Api(messages), Unit::Bytes) => messages
            .iter()
            .map(|message| message.content.len() as u64)
            .sum(),
    }
}

fn is_cli(transport: &str) -> bool {
    matches!(transport, "claude" | "codex" | "agy" | "grok")
}

/// 換幕摘要那次的請求（與 `summarize::summarize_scene` 整幕那次同一組裝與攤平）。
pub fn summary_request(
    events: &[TranscriptEvent],
    lang: &str,
    transport: &str,
    takeover: bool,
) -> Request {
    summary_request_of(
        transport::summary_messages(events, lang, takeover),
        lang,
        transport,
    )
}

/// 任何一次換幕摘要類呼叫（整幕、分段、合併、縮短）實際送出的形狀：CLI 以 GM 身分攤平並接摘要收尾句。
pub fn summary_request_of(messages: Vec<ChatMessage>, lang: &str, transport: &str) -> Request {
    if is_cli(transport) {
        let (system, prompt) =
            cli::flatten_messages("GM", transport::summary_closing(lang), &messages, lang);
        Request::Cli { system, prompt }
    } else {
        Request::Api(messages)
    }
}

pub fn summary_raw(
    events: &[TranscriptEvent],
    lang: &str,
    transport: &str,
    takeover: bool,
    unit: Unit,
) -> Raw {
    Raw {
        full: size(&summary_request(events, lang, transport, takeover), unit),
        fixed: size(&summary_request(&[], lang, transport, takeover), unit),
    }
}

/// 換幕摘要裡每則事件佔的量（角色側那一行＋分隔），供預測下一輪回覆量。
/// 不出現在摘要裡的事件（私設、GM 限定）回 0。
pub fn event_summary_size(event: &TranscriptEvent, lang: &str, unit: Unit) -> u64 {
    transport::lane_event_line(event, lang, Side::Character)
        .map(|line| measure(unit, &line) + measure(unit, "\n\n"))
        .unwrap_or(0)
}

/// 一桌的素材：與 GM 回合同一份讀法（`chat_assembly::gm_materials`），另把上一回合待落的正文算進本幕。
pub type Materials = GmMaterials;

pub fn load(root: &std::path::Path, world_id: &str) -> Result<Materials, String> {
    let mut materials = chat_assembly::gm_materials(root, world_id)?;
    // 上一回合待落的正文：任何新事件前都會先代落，算進本幕（容量關卡排在代落之前也不漏量）
    let scene = materials.state.current_scene;
    materials.events.extend(
        data::state_commit::with_commit(root, world_id, data::message_vars::unlanded_events)
            .into_iter()
            .filter(|(owner, _)| *owner == scene)
            .map(|(_, event)| event),
    );
    Ok(materials)
}

/// 一條聊天路徑：誰（GM 或某角色）、用哪個檔位、校正種類、lane 時的線身分。
pub struct ChatPath {
    pub kind: &'static str,
    pub tier: data::Tier,
    pub request_full: Request,
    pub request_fixed: Request,
    /// lane 後端：(線種, scope)；查上一輪實報的總輸入用
    pub lane: Option<(Lane, Option<String>)>,
}

/// 無狀態路徑的最終輸入：codex 經 CLI 攤平（與 dispatch 同一個 flatten），API 就是訊息本身。
fn stateless_request(
    messages: Vec<ChatMessage>,
    label: &str,
    closing: &str,
    lang: &str,
    transport: &str,
) -> Request {
    if is_cli(transport) {
        let (system, prompt) = cli::flatten_messages(label, closing, &messages, lang);
        Request::Cli { system, prompt }
    } else {
        Request::Api(messages)
    }
}

/// 量測讀的計時表：唯讀（`read_timed` 不結算、看到未結的 pending 只在記憶體裡以落地前的表算）；
/// 讀不了就當空表——量測只是估算，不該因計時檔壞掉擋住送出（實送前的結算會明確回錯）。
/// 固定部分（事件清空）也用空表：sticky 是本幕歷史的結果，不能算進固定部分。
fn measure_timed(
    root: &std::path::Path,
    world_id: &str,
    m: &Materials,
    perspective: &data::world_info_store::Perspective,
) -> crate::world_info::timed::WiTimed {
    if m.events.is_empty() {
        return Default::default();
    }
    data::world_info_store::read_timed(root, world_id, m.state.current_scene, perspective)
        .unwrap_or_default()
}

/// GM 那條（與 `commands::chat::gm_narrate` 同一組裝）：lane 後端量「重開全量」那份
/// （任何對不上都會降級重開），無狀態路徑量共線組裝＋同一個指示四態。
/// 世界書在組裝裡依「傳入的事件」掃（固定部分清空事件後重組，不帶本輪關鍵字條目），量測一律唯讀。
fn gm_request(
    root: &std::path::Path,
    world_id: &str,
    m: &Materials,
    provider: Option<LaneProvider>,
    lang: &str,
    transport_kind: &str,
    budget: Option<Budget>,
) -> Request {
    let timed = measure_timed(root, world_id, m, &data::world_info_store::Perspective::Gm);
    let scan = chat_assembly::gm_scan(m, &m.events, timed, budget, Randomness::Measure, lang);
    let (scope, _) = chat_assembly::gm_scope(m);
    let (instruction, closing) = chat_assembly::gm_instruction(root, world_id, m, &scan, lang);
    match provider {
        Some(_) => {
            let joined = format!("{}\n{closing}", instruction.content);
            let (system, tail) = chat_assembly::gm_lane_parts(m, &scan, &scope, &joined, lang);
            let prompt = lanes::build_prompt(&m.events, 0, &tail, true, Lane::Gm, lang);
            Request::Cli { system, prompt }
        }
        None => stateless_request(
            chat_assembly::gm_messages(m, &scan, &scope, instruction, lang),
            "GM",
            closing,
            lang,
            transport_kind,
        ),
    }
}

/// 角色那條（與 `commands::chat::chat_with_character` 同一組裝）。
/// `shape`：lane 後端時與實送同一組開關（`lanes::chars_lane_shape`），None＝無狀態路徑。
#[allow(clippy::too_many_arguments)]
fn character_request(
    root: &std::path::Path,
    world_id: &str,
    m: &Materials,
    card: &CharacterCard,
    shape: Option<lanes::CharsLaneShape>,
    lang: &str,
    transport_kind: &str,
    budget: Option<Budget>,
) -> Request {
    let branch = transport::resolve_branch(
        &m.state.state.tree,
        &m.state.branch_bindings,
        &card.id,
        &card.name,
    );
    let timed = measure_timed(
        root,
        world_id,
        m,
        &data::world_info_store::Perspective::Character(card.id.clone()),
    );
    let scan: WorldScan = chat_assembly::character_scan(
        &m.book,
        card,
        chat_assembly::sole_card_name(&m.metas, &m.events).as_deref(),
        m.player.as_ref(),
        &m.events,
        timed,
        budget,
        Randomness::Measure,
        lang,
    );
    let snapshot = m.book.snapshot();
    match shape {
        Some(shape) => {
            let (system, turn) = chat_assembly::character_lane_parts(
                card,
                &m.cards,
                m.player.as_ref(),
                &scan,
                &snapshot,
                &m.state,
                branch.as_deref(),
                lang,
                shape.hoist_private,
            );
            // 機密段（沒提進 system 的私設、限定條目、狀態）本來就在 tail 裡，不另外再接
            let prompt = lanes::build_prompt(&m.events, 0, &turn.tail, true, Lane::Chars, lang);
            Request::Cli { system, prompt }
        }
        // 共線組裝已自足：label 與 closing 傳空字串（同 chat_with_character）
        None => stateless_request(
            transport::assemble_shared_messages(
                card,
                &m.cards,
                m.player.as_ref(),
                &m.events,
                &scan,
                &snapshot,
                &m.state.state,
                &m.state.mechanism,
                branch.as_deref(),
                lang,
            ),
            "",
            "",
            lang,
            transport_kind,
        ),
    }
}

/// 這桌所有實際會用到的聊天路徑：GM＋每個在場角色。固定部分＝同一組裝把本幕事件清空。
/// `budget_for`：某檔位的世界書掃描預算（與實送同一個上限）。
#[allow(clippy::too_many_arguments)]
pub fn chat_paths(
    root: &std::path::Path,
    world_id: &str,
    m: &Materials,
    gm_tier: data::Tier,
    provider: Option<LaneProvider>,
    lang: &str,
    transport_kind: &str,
    budget_for: &dyn Fn(data::Tier) -> Option<Budget>,
) -> Vec<ChatPath> {
    let empty = Materials {
        events: Vec::new(),
        ..m.clone()
    };
    let gm_budget = budget_for(gm_tier);
    let mut paths = vec![ChatPath {
        kind: "gm",
        tier: gm_tier,
        request_full: gm_request(root, world_id, m, provider, lang, transport_kind, gm_budget),
        request_fixed: gm_request(
            root,
            world_id,
            &empty,
            provider,
            lang,
            transport_kind,
            gm_budget,
        ),
        lane: provider.map(|_| (Lane::Gm, None)),
    }];
    for card in &m.cards {
        // 與 chat_with_character 同一判定；讀卡失敗就當多角色（寧可多算機密段）
        let shape = provider.map(|provider| {
            let sole = provider == LaneProvider::Claude
                && chat_assembly::sole_present_character(root, world_id, &card.id, &m.events)
                    .unwrap_or(false);
            lanes::chars_lane_shape(provider, sole)
        });
        let scope = shape
            .filter(|shape| shape.scope_by_card)
            .map(|_| card.id.clone());
        let budget = budget_for(card.tier);
        paths.push(ChatPath {
            kind: "chars",
            tier: card.tier,
            request_full: character_request(
                root,
                world_id,
                m,
                card,
                shape,
                lang,
                transport_kind,
                budget,
            ),
            request_fixed: character_request(
                root,
                world_id,
                &empty,
                card,
                shape,
                lang,
                transport_kind,
                budget,
            ),
            lane: shape.map(|_| (Lane::Chars, scope)),
        });
    }
    paths
}
