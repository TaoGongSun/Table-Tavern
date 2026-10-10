//! 世界書掃描接線（worldbook-st-trigger-parity 包 5a，方案三之 2、三之 3）：一次生成＝一個視角掃一次。
//! 條目池依可見度過濾、訊息與全域欄位照視角組好，交給 `world_info::scan::check_world_info`；
//! 結果（`WorldScan`）由組裝、導演指示點名格式條目、落地三者共用，不再重擲。
//! 巨集在包 5b 換成新引擎；這裡的代換先沿用舊的 `{{user}}`／`{{char}}`。

mod book;
pub mod landing;
mod placement;
#[cfg(test)]
mod tests;

pub use book::TableBook;
pub use placement::{arrange, Placed};

use crate::data::world_info_store::counts_toward_timing;
use crate::data::{CharacterCard, TranscriptEvent, TranscriptKind};
use crate::scene_budget::{measure_units, Unit};
use crate::transport::{self, Side};
use crate::world_info::js_semantics::js_trim;
use crate::world_info::scan::{
    check_world_info, GlobalScan, ScanField, ScanHooks, ScanInput, Substituted,
};
use crate::world_info::sort::sort_entries;
use crate::world_info::timed::WiTimed;
use std::collections::BTreeSet;

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
}

/// 一個視角這一輪的掃描結果。
#[derive(Debug, Clone, Default)]
pub struct WorldScan {
    /// 觸發的條目，依 ST 放置前的排序（`compare_order` 穩定排序）
    pub placed: Vec<Placed>,
    /// 掃完之後的計時表（實送時落地）
    pub timed: WiTimed,
    /// 角色視角：共用快照的靜態條目（已在凍結 system，回合尾段不重複）；GM 視角是空的
    pub snapshot: BTreeSet<u64>,
}

struct Hooks {
    user: String,
    char_name: Option<String>,
    unit: Option<Unit>,
    random: Randomness,
}

impl ScanHooks for Hooks {
    fn substitute(&mut self, text: &str) -> Substituted {
        Substituted {
            text: transport::replace_st_macros(text, &self.user, self.char_name.as_deref()),
            private: false,
        }
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
fn global_scan(request: &ScanRequest<'_>, user: &str, char_name: Option<&str>) -> GlobalScan {
    let fill = |text: &str| transport::replace_st_macros(text, user, char_name);
    let persona = request
        .player
        .map(|player| fill(player.public_md.trim()))
        .unwrap_or_default();
    let character = match request.viewer {
        Viewer::Gm => ScanField::default(),
        Viewer::Character(card) => ScanField {
            full: fill(&joined(&[&card.public_md, &card.private_md])),
            public: fill(card.public_md.trim()),
        },
    };
    GlobalScan {
        persona_description: ScanField::public(persona),
        character_description: character.clone(),
        character_personality: character.clone(),
        character_depth_prompt: character,
        scenario: ScanField::default(),
        creator_notes: ScanField::default(),
    }
}

/// 掃一個視角。
pub fn scan(request: ScanRequest<'_>) -> WorldScan {
    let user = request
        .player
        .map(|player| player.name.trim())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| transport::player_fallback_name(request.lang))
        .to_owned();
    let char_name = match request.viewer {
        Viewer::Gm => None,
        Viewer::Character(card) => Some(card.name.clone()),
    };
    let settings = request.book.settings(&request.viewer);
    let mut pool = request.book.pool(&request.viewer);
    sort_entries(&mut pool);
    let snapshot: BTreeSet<u64> = match request.viewer {
        Viewer::Gm => BTreeSet::new(),
        Viewer::Character(_) => request.book.snapshot_uids(),
    };
    let pinned: BTreeSet<String> = snapshot.iter().map(u64::to_string).collect();
    let chat = scan_messages(&request, settings.include_names);
    let global = global_scan(&request, &user, char_name.as_deref());
    let mut hooks = Hooks {
        user,
        char_name,
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
    let placed = result
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
    WorldScan {
        placed,
        timed: result.timed,
        snapshot,
    }
}

/// JS `String.prototype.trim` 後的文字（注入段落用）。
pub(crate) fn trimmed(text: &str) -> &str {
    js_trim(text)
}
