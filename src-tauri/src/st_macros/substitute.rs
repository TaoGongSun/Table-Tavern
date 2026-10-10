//! 一次巨集代換（網頁版 `substitute.ts`，ST script.js `substituteParams`、MacroEnvBuilder）：環境由這裡組。
//! 卡欄位照 `getCharacterCardFieldsLazy`：用到才算、先過一輪不含卡欄位的代換（baseChatReplace）。
//! 兩種模式（方案三之 6）：完整＝在呼叫端的變數上執行並記操作序列；中性＝在隔離副本上執行，段落結束就丟掉。

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::BTreeMap;

use super::engine::{
    evaluate, CardField, CharacterSource, ChatLine, Limits, MacroEnv, Names, RandomSource,
    SourceText,
};
use super::js_value::{string_hash, trim};
use super::moment::Clock;
use super::variables::Variables;
use crate::world_info::scan::Substituted;

/// 提示組裝用得到的卡欄位原文（卡片沒有的欄位是空字串）；`private`＝讀自私密來源。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CardText {
    pub name: String,
    pub description: SourceText,
    pub personality: SourceText,
    pub scenario: SourceText,
    pub first_mes: SourceText,
    pub mes_example: SourceText,
    pub creator_notes: SourceText,
    pub system_prompt: SourceText,
    pub post_history_instructions: SourceText,
    pub alternate_greetings: Vec<SourceText>,
    pub character_version: String,
    pub depth_prompt: SourceText,
    /// `{{persona}}`：玩家卡的公開設定（ST 的人設描述）
    pub persona: SourceText,
}

/// 已代換好的卡欄位（本視角自己的卡：擁有文字第一輪的結果）。給了就直接回傳、不再對原文求值，
/// 卡欄位巨集不會讓同一段擁有文字的副作用多跑一次（方案三之 7）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreparedFields {
    pub description: SourceText,
    pub personality: SourceText,
    pub depth_prompt: SourceText,
}

/// 一段對話裡固定的代換條件（網頁版 `MacroContext`）。
pub struct MacroContext<'a> {
    pub card: &'a CardText,
    /// `card` 的描述、個性、角色深度提示已代換好的值（本視角自己的卡）；`None`＝照原文求值
    pub prepared: Option<&'a PreparedFields>,
    pub user_name: &'a str,
    /// 舊到新
    pub chat: &'a [ChatLine],
    pub variables: &'a RefCell<Variables>,
    pub chat_id: &'a str,
    pub input: &'a str,
    pub generation_type: &'a str,
    pub model: &'a str,
    pub limits: Limits,
    pub clock: &'a dyn Clock,
    pub random: &'a dyn RandomSource,
    /// 這次世界書掃出的 outlet（`{{outlet::名稱}}`）
    pub outlets: &'a BTreeMap<String, SourceText>,
    /// 有給就記下這次代換讀了哪些 outlet（格式條目判定「outlet 真的被代入」用）
    pub outlet_reads: Option<&'a RefCell<std::collections::BTreeSet<String>>>,
    pub is_mobile: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// 本視角擁有的文字：變數寫入落在呼叫端的表上，並記進 `Variables::ops`
    #[default]
    Full,
    /// 其他卡、顯示用：在變數的隔離副本上執行（同段 setvar→getvar 照樣看得到），結束就丟掉
    Neutral,
}

pub struct SubstituteOptions<'a> {
    pub mode: Mode,
    /// `{{original}}` 的原文；`private`＝讀自私密來源
    pub original: Option<&'a SourceText>,
    /// false＝卡欄位巨集（`{{description}}` 等）不代入，ST baseChatReplace 用
    pub replace_character_card: bool,
}

impl Default for SubstituteOptions<'_> {
    fn default() -> Self {
        Self {
            mode: Mode::Full,
            original: None,
            replace_character_card: true,
        }
    }
}

/// `substituteParams`：回傳代換結果與「讀到私密來源」。
pub fn substitute_params(
    content: &str,
    context: &MacroContext,
    options: SubstituteOptions,
) -> Substituted {
    match options.mode {
        Mode::Full => substitute_with(content, context, context.variables, options),
        Mode::Neutral => {
            let isolated = RefCell::new(context.variables.borrow().isolated());
            substitute_with(content, context, &isolated, options)
        }
    }
}

/// ST `baseChatReplace`：非空字串才代換（不含卡欄位），並拿掉 `\r`。
pub fn base_chat_replace(value: &str, context: &MacroContext, mode: Mode) -> Substituted {
    let options = SubstituteOptions {
        mode,
        replace_character_card: false,
        ..SubstituteOptions::default()
    };
    base_replace_with(value, context, None, options)
}

fn base_replace_with(
    value: &str,
    context: &MacroContext,
    variables: Option<&RefCell<Variables>>,
    options: SubstituteOptions,
) -> Substituted {
    if value.is_empty() {
        return Substituted {
            text: String::new(),
            private: false,
        };
    }
    let result = match variables {
        Some(variables) => substitute_with(value, context, variables, options),
        None => substitute_params(value, context, options),
    };
    Substituted {
        text: result.text.replace('\r', ""),
        private: result.private,
    }
}

/// 卡欄位：第一次用到時以同一份變數做 baseChatReplace，之後沿用。
struct LazyFields<'c, 'a> {
    context: &'c MacroContext<'a>,
    variables: &'c RefCell<Variables>,
    cells: [OnceCell<SourceText>; 11],
    greetings: OnceCell<Vec<SourceText>>,
}

impl LazyFields<'_, '_> {
    fn replace(&self, source: &SourceText) -> SourceText {
        let options = SubstituteOptions {
            replace_character_card: false,
            ..SubstituteOptions::default()
        };
        let result = base_replace_with(
            trim(&source.text),
            self.context,
            Some(self.variables),
            options,
        );
        SourceText {
            text: result.text,
            private: source.private || result.private,
        }
    }
}

impl CharacterSource for LazyFields<'_, '_> {
    fn field(&self, field: CardField) -> SourceText {
        let card = self.context.card;
        let prepared = self.context.prepared;
        self.cells[field as usize]
            .get_or_init(|| match (field, prepared) {
                (CardField::Description, Some(fields)) => fields.description.clone(),
                (CardField::Personality, Some(fields)) => fields.personality.clone(),
                (CardField::CharDepthPrompt, Some(fields)) => fields.depth_prompt.clone(),
                _ => match field {
                    CardField::CharPrompt => self.replace(&card.system_prompt),
                    CardField::CharInstruction => self.replace(&card.post_history_instructions),
                    CardField::Description => self.replace(&card.description),
                    CardField::Personality => self.replace(&card.personality),
                    CardField::Scenario => self.replace(&card.scenario),
                    CardField::Persona => self.replace(&card.persona),
                    CardField::MesExamplesRaw => self.replace(&card.mes_example),
                    CardField::CharDepthPrompt => self.replace(&card.depth_prompt),
                    CardField::CreatorNotes => self.replace(&card.creator_notes),
                    CardField::FirstMessage => self.replace(&card.first_mes),
                    CardField::Version => SourceText::public(card.character_version.clone()),
                },
            })
            .clone()
    }

    fn alternate_greetings(&self) -> &[SourceText] {
        self.greetings.get_or_init(|| {
            self.context
                .card
                .alternate_greetings
                .iter()
                .map(|greeting| self.replace(greeting))
                .collect()
        })
    }
}

fn substitute_with(
    content: &str,
    context: &MacroContext,
    variables: &RefCell<Variables>,
    options: SubstituteOptions,
) -> Substituted {
    if content.is_empty() {
        return Substituted {
            text: String::new(),
            private: false,
        };
    }
    let fields = LazyFields {
        context,
        variables,
        cells: Default::default(),
        greetings: OnceCell::new(),
    };
    let card_name = context.card.name.clone();
    let env = MacroEnv {
        content_hash: string_hash(content, 0),
        names: Names {
            user: context.user_name.to_owned(),
            char: card_name.clone(),
            group: card_name.clone(),
            group_not_muted: card_name,
            not_char: context.user_name.to_owned(),
        },
        character: options
            .replace_character_card
            .then_some(&fields as &dyn CharacterSource),
        model: context.model,
        original: options
            .original
            .map(|original| (original, Cell::new(false))),
        chat: context.chat,
        variables,
        input: context.input,
        generation_type: context.generation_type,
        limits: context.limits,
        clock: context.clock,
        chat_id: context.chat_id,
        random: context.random,
        outlets: context.outlets,
        outlet_reads: context.outlet_reads,
        is_mobile: context.is_mobile,
        private: Cell::new(false),
    };
    let text = evaluate(content, &env);
    Substituted {
        text,
        private: env.private.get(),
    }
}
