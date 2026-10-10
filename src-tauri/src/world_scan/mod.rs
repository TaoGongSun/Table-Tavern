//! 世界書掃描接線（worldbook-st-trigger-parity 包 5a，方案三之 2、三之 3）：一次生成＝一個視角掃一次。
//! 條目池依可見度過濾、訊息與全域欄位照視角組好，交給 `world_info::scan::check_world_info`；
//! 結果（`WorldScan`）由組裝、導演指示點名格式條目、落地三者共用，不再重擲。
//! 巨集走 `st_macros`（`macros::MacroSession`）：掃描時代換鍵與內文，掃完由 `prepare` 代換組裝要用的文字，
//! 組裝本身不再代換（方案三之 7）。

mod book;
#[cfg(test)]
mod integration_parity_tests;
pub mod landing;
#[cfg(test)]
mod macro_tests;
pub mod macros;
pub mod opening;
#[cfg(test)]
mod opening_tests;
mod placement;
mod prepare;
#[cfg(test)]
mod tests;

pub use book::TableBook;
pub use macros::{MacroInputs, MacroSession};
pub use placement::{arrange, Placed};
pub use prepare::{prepare, PromptTexts};

use crate::data::world_info_store::counts_toward_timing;
use crate::data::{CharacterCard, TranscriptEvent, TranscriptKind};
use crate::scene_budget::{measure_units, Unit};
use crate::st_macros::engine::SourceText;
use crate::st_macros::variables::VarOp;
use crate::transport::{self, Side};
use crate::world_info::js_semantics::js_trim;
use crate::world_info::scan::{
    check_world_info, GlobalScan, ScanField, ScanHooks, ScanInput, Substituted,
};
use crate::world_info::sort::sort_entries;
use crate::world_info::timed::WiTimed;
use std::collections::{BTreeMap, BTreeSet};

/// 掃描的視角：GM 看得到全部條目；角色只看得到 `Public` 與名單含自己的限定條目（P1、三之 2）。
#[derive(Clone, Copy)]
pub enum Viewer<'a> {
    Gm,
    Character(&'a CharacterCard),
}

/// 世界書預算的上限（該路徑上限的 `total − reserve`）與計數單位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    pub unit: Unit,
    pub max: f64,
}

/// 機率與群組抽選的亂數來源：實送用系統亂數；量測一律「機率視為通過、群組抽選用固定種子 0」。
pub enum Randomness {
    Live,
    Measure,
}

impl Randomness {
    fn next(&mut self) -> f64 {
        match self {
            Randomness::Live => {
                let mut bytes = [0u8; 8];
                match getrandom::fill(&mut bytes) {
                    // 53 位元的 [0, 1)，同 Math.random 的精度
                    Ok(()) => (u64::from_le_bytes(bytes) >> 11) as f64 / (1u64 << 53) as f64,
                    Err(_) => 0.0,
                }
            }
            Randomness::Measure => 0.0,
        }
    }
}

/// 一次掃描的輸入。`events` 是本幕原始事件（排除與渲染都在這裡做）；`timed` 是該視角的計時表。
pub struct ScanRequest<'a> {
    pub book: &'a TableBook,
    pub viewer: Viewer<'a>,
    /// 單角色桌（有效在場集合恰好一張，同 Claude 單人線的判定）的卡名：開場白用它帶名字掃描
    pub sole_card: Option<&'a str>,
    pub player: Option<&'a CharacterCard>,
    pub events: &'a [TranscriptEvent],
    pub lang: &'a str,
    pub timed: WiTimed,
    pub budget: Option<Budget>,
    pub random: Randomness,
    /// 這一輪的巨集環境（同一個視角）
    pub session: &'a MacroSession,
}

/// 一個視角這一輪的掃描結果，加上 `prepare` 代換好的組裝文字。
#[derive(Debug, Clone, Default)]
pub struct WorldScan {
    /// 觸發的條目，依 ST 放置前的排序（`compare_order` 穩定排序）；`prepare` 之後標題與內文都已代換
    pub placed: Vec<Placed>,
    /// 掃完之後的計時表（實送時落地）
    pub timed: WiTimed,
    /// 角色視角：共用快照的靜態條目（已在凍結 system，回合尾段不重複）；GM 視角是空的
    pub snapshot: BTreeSet<u64>,
    /// 角色視角：共用快照的條目（`prepare` 以中性脈絡代換）
    pub snapshot_entries: Vec<Placed>,
    /// 本輪 outlet（名稱 → 內容；組成裡有機密條目就是私密）：實送落地後成為下一輪的「上一輪 outlet」
    pub outlets: BTreeMap<String, SourceText>,
    /// 掃完之後的代換真的代入過的 outlet（格式條目判定用）
    pub outlets_used: BTreeSet<String>,
    /// 組裝要用的已代換文字
    pub texts: PromptTexts,
    /// 這一輪完整求值寫下的變數操作序列（實送落地時重放）
    pub var_ops: Vec<VarOp>,
}

struct Hooks<'a> {
    session: &'a MacroSession,
    unit: Option<Unit>,
    random: Randomness,
}

impl ScanHooks for Hooks<'_> {
    fn substitute(&mut self, text: &str) -> Substituted {
        self.session.own(text)
    }

    fn count_tokens(&mut self, text: &str) -> f64 {
        self.unit
            .map_or(0.0, |unit| measure_units(unit, text) as f64)
    }

    fn random(&mut self) -> f64 {
        self.random.next()
    }
}

/// 帶名字掃描的名字（三之 2）：玩家句用玩家名、角色台詞用說話者；旁白在世界書路的桌用原卡名，
/// 開場白在單角色桌用該角色名、世界書路用原卡名；其餘旁白與開場白用 `GM`。
fn scan_name(event: &TranscriptEvent, request: &ScanRequest<'_>) -> String {
    match event.kind {
        TranscriptKind::Player => transport::prompt_speaker(event, request.lang).into_owned(),
        TranscriptKind::Dialogue => event.speaker_name.clone(),
        TranscriptKind::Narration | TranscriptKind::System => {
            if let Some(name) = request.book.world_card_name() {
                return name.to_owned();
            }
            match (event.opening, request.sole_card) {
                (true, Some(name)) => name.to_owned(),
                _ => "GM".to_owned(),
            }
        }
    }
}

/// 掃描用訊息清單（新到舊）：先排除 `System` 類與換幕摘要（與則數同一規則），再照視角渲染；
/// 渲染後不送的那則留空字串，則數與各視角一致。
fn scan_messages(request: &ScanRequest<'_>, include_names: bool) -> Vec<String> {
    let side = match request.viewer {
        Viewer::Gm => Side::Gm,
        Viewer::Character(_) => Side::Character,
    };
    let mut lines: Vec<String> = request
        .events
        .iter()
        .filter(|event| counts_toward_timing(event))
        .map(
            |event| match transport::prompt_text(event, request.lang, side) {
                None => String::new(),
                Some(text) if include_names => format!("{}: {text}", scan_name(event, request)),
                Some(text) => text,
            },
        )
        .collect();
    lines.reverse();
    lines
}

fn joined(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 全域掃描欄位（P7）：角色視角的描述、個性、角色深度提示比對該卡 `public_md`＋`private_md`（公開部分只有
/// `public_md`），人設比對玩家卡 `public_md`；GM 視角角色欄給空；劇本與作者備註沒有對應欄位。
/// 文字是擁有者第一輪代換的結果（ST 先 `getCharacterCardFields` 再掃），同一份文字不重複執行副作用。
/// 代換讀到私密來源（私密的上一輪 outlet、私設）的那段不算公開部分：只靠它觸發的條目照私密觸發分流。
fn global_scan(request: &ScanRequest<'_>) -> GlobalScan {
    let session = request.session;
    let field = |full: String, public: &str, private: bool| ScanField {
        public: match private {
            true => String::new(),
            false => public.to_owned(),
        },
        full,
    };
    let persona = match request.player {
        Some(player) => {
            let card = macros::card_text(player, false, session.persona());
            let persona = session.neutral(player.public_md.trim(), &card);
            field(persona.text.clone(), &persona.text, persona.private)
        }
        None => ScanField::default(),
    };
    let base = session.own_base();
    let character = match request.viewer {
        Viewer::Gm => ScanField::default(),
        Viewer::Character(_) => field(
            joined(&[&base.public.text, &base.private.text]),
            base.public.text.trim(),
            base.public.private,
        ),
    };
    GlobalScan {
        persona_description: persona,
        character_description: character.clone(),
        character_personality: character.clone(),
        character_depth_prompt: character,
        scenario: ScanField::default(),
        creator_notes: ScanField::default(),
    }
}

/// 掃一個視角。
pub fn scan(request: ScanRequest<'_>) -> WorldScan {
    // ST 先代換卡欄位（含副作用）才掃：擁有文字的第一輪代換排在最前面
    request.session.own_base();
    let settings = request.book.settings(&request.viewer);
    let mut pool = request.book.pool(&request.viewer);
    sort_entries(&mut pool);
    let snapshot: BTreeSet<u64> = match request.viewer {
        Viewer::Gm => BTreeSet::new(),
        Viewer::Character(_) => request.book.snapshot_uids(),
    };
    let pinned: BTreeSet<String> = snapshot.iter().map(u64::to_string).collect();
    let chat = scan_messages(&request, settings.include_names);
    let global = global_scan(&request);
    let mut hooks = Hooks {
        session: request.session,
        unit: request.budget.map(|budget| budget.unit),
        random: request.random,
    };
    let result = check_world_info(
        &pool,
        ScanInput {
            chat: &chat,
            max_context: request.budget.map_or(f64::INFINITY, |budget| budget.max),
            global_scan: &global,
            // 桌面版沒有重新生成（方案二「生成類型」）
            trigger: "normal",
            timed: request.timed,
            settings,
            pinned: &pinned,
        },
        &mut hooks,
    );
    let placed: Vec<Placed> = result
        .placed
        .iter()
        .filter_map(|(id, content)| {
            let entry = pool.iter().find(|entry| entry.id == *id)?;
            let view = request.book.view(id)?;
            Some(Placed::new(
                view,
                entry,
                content.clone(),
                result.private_ids.contains(id) || result.private_content.contains(id),
            ))
        })
        .collect();
    let outlets = result
        .outlets
        .iter()
        .map(|(name, contents)| {
            let private = placed.iter().any(|entry: &Placed| {
                entry.outlet_name == *name
                    && entry.position == crate::world_info::entry::position::OUTLET
                    && entry.confidential()
            });
            (
                name.clone(),
                SourceText {
                    text: contents.join("\n"),
                    private,
                },
            )
        })
        .collect();
    WorldScan {
        placed,
        timed: result.timed,
        snapshot,
        outlets,
        ..WorldScan::default()
    }
}

/// JS `String.prototype.trim` 後的文字（注入段落用）。
pub(crate) fn trimmed(text: &str) -> &str {
    js_trim(text)
}
