//! 掃完之後、組裝之前的代換（方案三之 3、三之 7）：outlet 換成本輪值，組裝要用的文字一次代換好——
//! 組裝函式自己不再代換，同一段文字的副作用每次生成只執行一次，凍結 system 與指紋也不會因重算而不同。
//! 先後照網頁版 `prompt.ts`：前／後兩組條目的內文再代換一次（ST 對 `wi.before`／`wi.after`）→ 擁有文字第二輪
//! （ST 對 `getCharacterCardFields` 結果再 `substituteParams`）→ 注入段落（由淺到深、同深度 system → user →
//! assistant）；其他卡、玩家卡、標題中性。不送出的條目（outlet、內文空、深度不合法）不跑第二輪：outlet 的內容
//! 只在 `{{outlet::名稱}}` 被代入時進提示（ST 也只代換實際送出的分組）。

use super::macros::{card_text, MacroSession};
use super::{arrange, Placed, TableBook, Viewer, WorldScan};
use crate::data::CharacterCard;
use crate::st_macros::engine::is_static;
use crate::st_macros::substitute::CardText;
use std::collections::BTreeMap;

/// 一張卡在這一輪提示裡的文字（已代換）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CardTexts {
    pub public: String,
    /// 只有看得到的視角才有（GM 全部、角色只有自己的）
    pub private: String,
    /// 公開設定含靜態巨集以外的巨集：不進角色共線的共用快照，改在每位角色的回合尾（會抹掉的段落）以
    /// 該角色視角代換（三之 3）。
    pub dynamic: bool,
}

/// 組裝要用的已代換文字。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PromptTexts {
    /// GM 視角：world.md
    pub world_md: String,
    /// 卡 id → 文字
    pub cards: BTreeMap<String, CardTexts>,
    /// 玩家卡公開設定（中性）；動態時同卡片公開設定的規則
    pub player: CardTexts,
}

impl PromptTexts {
    /// 某張卡的文字；`prepare` 沒收到這張卡（組裝與準備拿到不同的卡清單，不該發生）就當空的。
    pub fn card(&self, card: &CharacterCard) -> CardTexts {
        self.cards.get(&card.id).cloned().unwrap_or_else(|| {
            log::warn!("巨集：組裝用到沒準備過的卡 {}", card.id);
            CardTexts::default()
        })
    }
}

/// 中性代換一段卡片文字；拿掉 `\r`（同擁有文字第一輪的 baseChatReplace），自己的卡與別人看到的同一張卡逐字相同，
/// 共用快照才不會因視角而不同。
fn neutral_card(session: &MacroSession, text: &str, card: &CardText) -> String {
    session.neutral(text, card).text.replace('\r', "")
}

/// 公開設定只含靜態巨集與 `{{char}}`（該卡自己的名字）＝不隨視角或輪次改變。
fn dynamic_public(text: &str) -> bool {
    !is_static(text, true)
}

/// 掃完之後代換組裝文字。`cards` 是組裝會用到的全部角色卡（同一份清單交給組裝函式）。
pub fn prepare(
    scan: &mut WorldScan,
    session: &MacroSession,
    book: &TableBook,
    viewer: &Viewer<'_>,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
) {
    session.set_outlets(scan.outlets.clone());
    let base = session.own_base().clone();
    let mut texts = PromptTexts::default();
    let persona = session.persona().clone();
    let (early, late) = second_round(&scan.placed);
    for &index in &early {
        second(session, &mut scan.placed[index]);
    }
    match viewer {
        Viewer::Gm => {
            texts.world_md = session.own(&base.public.text).text;
            for card in cards {
                let card_view = card_text(card, true, &persona);
                texts.cards.insert(
                    card.id.clone(),
                    CardTexts {
                        public: neutral_card(session, card.public_md.trim(), &card_view),
                        private: neutral_card(session, card.private_md.trim(), &card_view),
                        dynamic: dynamic_public(card.public_md.trim()),
                    },
                );
            }
        }
        Viewer::Character(me) => {
            texts.cards.insert(
                me.id.clone(),
                CardTexts {
                    public: session.own(&base.public.text).text,
                    private: session.own(&base.private.text).text,
                    dynamic: dynamic_public(me.public_md.trim()),
                },
            );
            for card in cards.iter().filter(|card| card.id != me.id) {
                // 別人的卡：看不到私設，卡欄位巨集也只讀得到公開設定
                let card_view = card_text(card, false, &persona);
                texts.cards.insert(
                    card.id.clone(),
                    CardTexts {
                        public: neutral_card(session, card.public_md.trim(), &card_view),
                        private: String::new(),
                        dynamic: dynamic_public(card.public_md.trim()),
                    },
                );
            }
        }
    }
    if let Some(player) = player {
        let card_view = card_text(player, false, &persona);
        texts.player = CardTexts {
            public: neutral_card(session, player.public_md.trim(), &card_view),
            private: String::new(),
            dynamic: dynamic_public(player.public_md.trim()),
        };
    }
    for &index in &late {
        second(session, &mut scan.placed[index]);
    }
    for entry in &mut scan.placed {
        let title = session.neutral_own(&entry.title);
        entry.title = title.text;
        entry.private_trigger |= title.private;
    }
    let own_card = session.own_card().clone();
    if let Viewer::Character(_) = viewer {
        scan.snapshot_entries = book
            .snapshot()
            .into_iter()
            .map(|entry| neutral_entry(session, &own_card, entry))
            .collect();
    }
    scan.outlets_used = session.outlet_reads();
    scan.texts = texts;
    scan.var_ops = session.take_ops();
}

/// 觸發條目內文的第二輪（完整求值）。
fn second(session: &MacroSession, entry: &mut Placed) {
    let content = session.own(&entry.content);
    entry.content = content.text;
    entry.private_trigger |= content.private;
}

/// 第二輪的先後（`scan.placed` 的索引）：前（擁有文字之前）＝前組、後組；後（擁有文字之後）＝注入段落，
/// 照網頁版 `injectExtensionPrompts` 代換的順序（攤平的閱讀順序反過來）。不在這兩份裡的條目不送出。
fn second_round(placed: &[Placed]) -> (Vec<usize>, Vec<usize>) {
    let refs: Vec<&Placed> = placed.iter().collect();
    let arranged = arrange(&refs);
    let index = |entry: &Placed| {
        placed
            .iter()
            .position(|candidate| std::ptr::eq(candidate, entry))
            .expect("arrange 只回傳傳入的條目")
    };
    let early = arranged
        .front
        .iter()
        .chain(arranged.back.iter())
        .map(|entry| index(entry))
        .collect();
    let late = arranged
        .injected
        .iter()
        .rev()
        .flat_map(|injection| injection.members.iter().map(|entry| index(entry)))
        .collect();
    (early, late)
}

/// 共用快照的條目（靜態，所有角色代換結果相同）。
fn neutral_entry(session: &MacroSession, card: &CardText, mut entry: Placed) -> Placed {
    entry.title = session.neutral(&entry.title, card).text;
    entry.content = session.neutral(&entry.content, card).text;
    entry
}
