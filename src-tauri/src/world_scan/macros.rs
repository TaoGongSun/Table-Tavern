//! 一次生成的巨集環境（方案三之 6、三之 7）：同一個視角、同一份變數表、同一組 outlet。
//! 完整求值（含副作用）只用在本視角擁有的文字——自己那張卡（角色視角）、world.md（GM 視角）、本視角條目池裡
//! 的條目；其他卡、玩家卡、標題用中性模式（變數隔離副本，段落結束就丟）。完整模式的寫入記成操作序列，
//! 實送時由 `landing` 重放落地；量測與試掃只讀，不落地。

use super::Viewer;
use crate::data::card_vars::{self, Layer};
use crate::data::world_info_store::{counts_toward_timing, Perspective};
use crate::data::{CharacterCard, TranscriptEvent, TranscriptKind};
use crate::st_macros::engine::{ChatLine, Limits, RandomSource, SourceText};
use crate::st_macros::moment::SystemClock;
use crate::st_macros::substitute::{
    base_chat_replace, substitute_params, CardText, MacroContext, Mode, PreparedFields,
    SubstituteOptions,
};
use crate::st_macros::variables::{VarOp, VarScope, Variables};
use crate::transport::{self, Side};
use crate::world_info::scan::Substituted;
use std::cell::{OnceCell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// 從磁碟與設定讀來的巨集條件（每個視角、每次生成一份）。
pub struct MacroInputs {
    pub variables: Variables,
    /// 上一輪實送落地時記下的 outlet（掃描前的代換讀它，ST 掃完才換）
    pub prev_outlets: BTreeMap<String, SourceText>,
    /// `{{pick}}` 的 chatId：桌 id
    pub chat_id: String,
    /// `{{model}}`
    pub model: String,
    /// `{{maxPrompt}}` 等
    pub limits: Limits,
    /// 實送（系統亂數）或量測（亂數固定 0）
    pub live: bool,
}

impl MacroInputs {
    /// 讀變數層（chat、global）與上一輪 outlet。層讀不了就當空表（落地時會重讀、讀不了就回錯不送出）。
    pub fn load(
        root: &Path,
        world_id: &str,
        perspective: &Perspective,
        model: String,
        limits: Limits,
        live: bool,
    ) -> Self {
        let scope = |layer: Layer| {
            card_vars::read_layer(root, world_id, layer, None)
                .map_err(|error| error.to_string())
                .and_then(|doc| VarScope::from_json_text(&doc.vars))
                .unwrap_or_else(|error| {
                    log::warn!("巨集：讀不了 {layer:?} 變數層，這次當空表：{error}");
                    VarScope::default()
                })
        };
        Self {
            variables: Variables::new(scope(Layer::Chat), scope(Layer::Global)),
            prev_outlets: previous_outlets(world_id, perspective),
            chat_id: world_id.to_owned(),
            model,
            limits,
            live,
        }
    }

    /// 測試用：空變數、沒有上一輪 outlet、亂數固定 0。
    #[cfg(test)]
    pub fn empty() -> Self {
        Self {
            variables: Variables::default(),
            prev_outlets: BTreeMap::new(),
            chat_id: "table".to_owned(),
            model: String::new(),
            limits: Limits::default(),
            live: false,
        }
    }
}

/// 上一輪 outlet（記憶體，以桌 id＋視角為鍵）：實送落地才更新；app 重啟、換幕、退幕、分岔都清空
/// （ST 換聊天也清空，outlet 不持久化）。
fn outlet_memory() -> &'static Mutex<HashMap<(String, String), BTreeMap<String, SourceText>>> {
    static MEMORY: OnceLock<Mutex<HashMap<(String, String), BTreeMap<String, SourceText>>>> =
        OnceLock::new();
    MEMORY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn previous_outlets(world_id: &str, perspective: &Perspective) -> BTreeMap<String, SourceText> {
    outlet_memory()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&(world_id.to_owned(), perspective.key()))
        .cloned()
        .unwrap_or_default()
}

/// 實送落地成功：這個視角的上一輪 outlet 換成本輪值。
pub fn remember_outlets(
    world_id: &str,
    perspective: &Perspective,
    outlets: &BTreeMap<String, SourceText>,
) {
    outlet_memory()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert((world_id.to_owned(), perspective.key()), outlets.clone());
}

/// 換幕、退幕、分岔：這桌各視角的上一輪 outlet 全部清空。
pub fn forget_outlets(world_id: &str) {
    outlet_memory()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retain(|(world, _), _| world != world_id);
}

struct LiveRandom;

impl RandomSource for LiveRandom {
    fn next_random(&self) -> f64 {
        let mut bytes = [0u8; 8];
        match getrandom::fill(&mut bytes) {
            Ok(()) => (u64::from_le_bytes(bytes) >> 11) as f64 / (1u64 << 53) as f64,
            Err(_) => 0.0,
        }
    }
}

struct ZeroRandom;

impl RandomSource for ZeroRandom {
    fn next_random(&self) -> f64 {
        0.0
    }
}

/// 視角擁有的原文：角色視角是自己那張卡的公開／私有設定，GM 視角是 world.md。
enum Own {
    Character { public: String, private: String },
    Gm { world_md: String },
}

/// 擁有文字的第一輪代換（ST Generate 開頭 `getCharacterCardFields` 那輪，不含卡欄位、有副作用）。
#[derive(Debug, Clone, Default)]
pub struct OwnBase {
    /// 角色：公開設定；GM：world.md
    pub public: Substituted,
    /// 角色：私有設定；GM：空
    pub private: Substituted,
}

/// 卡欄位巨集看到的卡（P7）：描述、個性、角色深度提示＝公開設定＋（看得到時）私有設定，其餘欄位空。
pub fn card_text(card: &CharacterCard, with_private: bool, persona: &SourceText) -> CardText {
    let private = with_private && !card.private_md.trim().is_empty();
    let text = match private {
        true => format!("{}\n{}", card.public_md.trim(), card.private_md.trim()),
        false => card.public_md.trim().to_owned(),
    };
    let field = SourceText {
        text: text.trim().to_owned(),
        private,
    };
    CardText {
        name: card.name.clone(),
        description: field.clone(),
        personality: field.clone(),
        depth_prompt: field,
        persona: persona.clone(),
        ..CardText::default()
    }
}

/// 一次生成的巨集環境。
pub struct MacroSession {
    user: String,
    /// 本視角的卡（GM：世界書路的原卡名，其餘 `{{char}}` 照現行原樣留著）
    own_card: CardText,
    own: Own,
    own_base: OnceCell<OwnBase>,
    own_fields: OnceCell<Option<PreparedFields>>,
    persona: SourceText,
    chat: Vec<ChatLine>,
    input: String,
    variables: RefCell<Variables>,
    chat_id: String,
    model: String,
    limits: Limits,
    clock: SystemClock,
    random: Box<dyn RandomSource>,
    outlets: RefCell<BTreeMap<String, SourceText>>,
    outlet_reads: RefCell<BTreeSet<String>>,
}

/// 事件時間（`YYYY-MM-DD HH:MM[:SS]`，本地時間）→ epoch 毫秒；讀不懂回 None。
fn sent_at(ts: &str) -> Option<f64> {
    use chrono::{Local, NaiveDateTime, TimeZone};
    let parsed = NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M"))
        .ok()?;
    Local
        .from_local_datetime(&parsed)
        .earliest()
        .map(|time| time.timestamp_millis() as f64)
}

/// `{{lastMessage}}` 系列與歷史（舊到新）：本視角送得出的非系統、非摘要事件（與掃描同一排除規則）。
fn chat_lines(events: &[TranscriptEvent], lang: &str, side: Side) -> Vec<ChatLine> {
    events
        .iter()
        .filter(|event| counts_toward_timing(event))
        .filter_map(|event| {
            transport::prompt_text(event, lang, side).map(|text| ChatLine {
                is_user: event.kind == TranscriptKind::Player,
                text,
                sent_at: sent_at(&event.ts),
            })
        })
        .collect()
}

impl MacroSession {
    /// `world_md` 只有 GM 視角用得到；`world_card` 是世界書路的原卡名（GM 的 `{{char}}`）。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        inputs: MacroInputs,
        viewer: &Viewer<'_>,
        world_md: &str,
        world_card: Option<&str>,
        player: Option<&CharacterCard>,
        events: &[TranscriptEvent],
        lang: &str,
    ) -> Self {
        let user = player
            .map(|player| player.name.trim())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| transport::player_fallback_name(lang))
            .to_owned();
        let persona = SourceText::public(
            player
                .map(|player| player.public_md.trim().to_owned())
                .unwrap_or_default(),
        );
        let (own_card, own, side) = match viewer {
            Viewer::Gm => (
                CardText {
                    name: world_card.unwrap_or("{{char}}").to_owned(),
                    persona: persona.clone(),
                    ..CardText::default()
                },
                Own::Gm {
                    world_md: world_md.trim().to_owned(),
                },
                Side::Gm,
            ),
            Viewer::Character(card) => (
                card_text(card, true, &persona),
                Own::Character {
                    public: card.public_md.trim().to_owned(),
                    private: card.private_md.trim().to_owned(),
                },
                Side::Character,
            ),
        };
        let input = events
            .iter()
            .rev()
            .find(|event| counts_toward_timing(event))
            .filter(|event| event.kind == TranscriptKind::Player)
            .map(|event| event.text.clone())
            .unwrap_or_default();
        Self {
            user,
            own_card,
            own,
            own_base: OnceCell::new(),
            own_fields: OnceCell::new(),
            persona,
            chat: chat_lines(events, lang, side),
            input,
            variables: RefCell::new(inputs.variables),
            chat_id: inputs.chat_id,
            model: inputs.model,
            limits: inputs.limits,
            clock: SystemClock,
            random: match inputs.live {
                true => Box::new(LiveRandom),
                false => Box::new(ZeroRandom),
            },
            outlets: RefCell::new(inputs.prev_outlets),
            outlet_reads: RefCell::new(BTreeSet::new()),
        }
    }

    pub fn own_card(&self) -> &CardText {
        &self.own_card
    }

    pub fn persona(&self) -> &SourceText {
        &self.persona
    }

    fn with_context<T>(
        &self,
        card: &CardText,
        prepared: Option<&PreparedFields>,
        work: impl FnOnce(&MacroContext) -> T,
    ) -> T {
        let outlets = self.outlets.borrow();
        self.with_outlets(card, prepared, &outlets, work)
    }

    fn with_outlets<T>(
        &self,
        card: &CardText,
        prepared: Option<&PreparedFields>,
        outlets: &BTreeMap<String, SourceText>,
        work: impl FnOnce(&MacroContext) -> T,
    ) -> T {
        let context = MacroContext {
            card,
            prepared,
            user_name: &self.user,
            chat: &self.chat,
            variables: &self.variables,
            chat_id: &self.chat_id,
            input: &self.input,
            generation_type: "normal",
            model: &self.model,
            limits: self.limits,
            clock: &self.clock,
            random: self.random.as_ref(),
            outlets,
            outlet_reads: Some(&self.outlet_reads),
            is_mobile: false,
        };
        work(&context)
    }

    /// 本視角擁有的文字：完整求值（副作用記進操作序列）。自己那張卡的卡欄位巨集回傳第一輪的結果。
    pub fn own(&self, text: &str) -> Substituted {
        let fields = self.own_fields();
        self.with_context(&self.own_card, fields, |context| {
            substitute_params(text, context, SubstituteOptions::default())
        })
    }

    /// 以指定的卡完整求值（開場白：卡欄位只讀公開設定）。
    pub fn own_as(&self, text: &str, card: &CardText) -> Substituted {
        self.with_context(card, None, |context| {
            substitute_params(text, context, SubstituteOptions::default())
        })
    }

    /// 其他卡、玩家卡：中性求值（變數隔離副本，不記副作用）。`card` 是這段文字的卡。
    pub fn neutral(&self, text: &str, card: &CardText) -> Substituted {
        self.neutral_with(text, card, None)
    }

    /// 本視角的標題等不送出副作用的文字：中性求值，卡欄位巨集同 `own` 回傳第一輪的結果。
    pub fn neutral_own(&self, text: &str) -> Substituted {
        let fields = self.own_fields();
        self.neutral_with(text, &self.own_card, fields)
    }

    fn neutral_with(
        &self,
        text: &str,
        card: &CardText,
        prepared: Option<&PreparedFields>,
    ) -> Substituted {
        self.with_context(card, prepared, |context| {
            substitute_params(
                text,
                context,
                SubstituteOptions {
                    mode: Mode::Neutral,
                    ..SubstituteOptions::default()
                },
            )
        })
    }

    /// 角色視角自己那張卡的卡欄位（描述、個性、角色深度提示＝公開＋私有設定，P7）：擁有文字第一輪的結果，
    /// 卡欄位巨集直接回傳、不再對原文求值，副作用每段每次生成只跑一次。GM 視角沒有自己的卡。
    fn own_fields(&self) -> Option<&PreparedFields> {
        self.own_fields
            .get_or_init(|| {
                let Own::Character { private, .. } = &self.own else {
                    return None;
                };
                let base = self.own_base();
                let with_private = !private.is_empty();
                let text = match with_private {
                    true => format!("{}\n{}", base.public.text.trim(), base.private.text.trim()),
                    false => base.public.text.trim().to_owned(),
                };
                let field = SourceText {
                    text: text.trim().to_owned(),
                    private: with_private || base.public.private || base.private.private,
                };
                Some(PreparedFields {
                    description: field.clone(),
                    personality: field.clone(),
                    depth_prompt: field,
                })
            })
            .as_ref()
    }

    /// 擁有文字的第一輪代換（只算一次；角色先公開後私有）。掃描開頭就叫，副作用的先後才與 ST 一樣排在掃描之前。
    pub fn own_base(&self) -> &OwnBase {
        self.own_base.get_or_init(|| {
            let base = |text: &str| {
                self.with_context(&self.own_card, None, |context| {
                    base_chat_replace(text, context, Mode::Full)
                })
            };
            match &self.own {
                Own::Character { public, private } => {
                    let public = base(public);
                    let private = base(private);
                    OwnBase { public, private }
                }
                Own::Gm { world_md } => OwnBase {
                    public: base(world_md),
                    private: Substituted::default(),
                },
            }
        })
    }

    /// 掃完之後：outlet 換成本輪值，並從這裡開始記哪些 outlet 被代入。
    pub fn set_outlets(&self, outlets: BTreeMap<String, SourceText>) {
        *self.outlets.borrow_mut() = outlets;
        self.outlet_reads.borrow_mut().clear();
    }

    pub fn outlet_reads(&self) -> BTreeSet<String> {
        self.outlet_reads.borrow().clone()
    }

    /// 公開事件（登場、回歸、開場白）用的環境：`{{lastMessage}}` 系列只讀角色側看得到的事件，不帶 outlet。
    pub fn publicized(mut self, events: &[TranscriptEvent], lang: &str) -> Self {
        self.chat = chat_lines(events, lang, Side::Character);
        *self.outlets.borrow_mut() = BTreeMap::new();
        self
    }

    /// 寫死進逐字稿的事件文字：中性代換一段屬於 `card`（`None`＝本視角）的文字。公開事件（`gm_only` 假）
    /// 卡欄位巨集只讀公開設定、沒有 outlet；只給 GM 看的事件才讀得到私設與 outlet。
    pub fn fill(&self, text: &str, card: Option<&CharacterCard>, gm_only: bool) -> String {
        let view = match card {
            Some(card) => card_text(card, gm_only, &self.persona),
            None => self.own_card.clone(),
        };
        if gm_only {
            return self.neutral(text, &view).text;
        }
        let empty = BTreeMap::new();
        self.with_outlets(&view, None, &empty, |context| {
            substitute_params(
                text,
                context,
                SubstituteOptions {
                    mode: Mode::Neutral,
                    ..SubstituteOptions::default()
                },
            )
        })
        .text
    }

    /// 測試用：目前的變數表（完整求值寫進去的都在）。
    #[cfg(test)]
    pub fn variables(&self) -> Variables {
        self.variables.borrow().clone()
    }

    /// 這次生成完整求值寫下的操作序列（依發生順序）。
    pub fn take_ops(&self) -> Vec<VarOp> {
        std::mem::take(&mut self.variables.borrow_mut().ops)
    }
}
