//! VN 劇本聖經（plans/vn-mode-pkg4.md §2.2–§2.4）：生成與重試、套用到狀態樹與規則、角色秘密重骰、
//! GM 注入的凍結副本、劇本分頁存檔、格邊旗標提示。模型呼叫一律在短提交鎖外，成功後才進鎖提交。
use crate::data::vn::{self, Bible, Frozen};
use crate::data::{self, StateNode, TableState};
use crate::scene_budget::summarize::SummaryCaller;
use crate::transport::vn_bible_prompts as prompts;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

/// 已有遊玩紀錄：聖經只在未遊玩的桌生成或重骰（前端換成「請開新桌」提示）。
pub const PLAYED: &str = "VN_BIBLE_PLAYED";
/// 解析重試一次仍失敗。
pub const FAILED: &str = "VN_BIBLE_FAILED";
pub const NO_BIBLE: &str = "VN_BIBLE_MISSING";
/// VN 卡鎖住章數：劇本分頁存檔不收章數變更。
pub const CHAPTER_LOCKED: &str = "VN_CHAPTER_LOCKED";

fn text<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

/// 生成與重骰的素材：world.md、送進提示詞的卡（含私設）、GM-only 世界書條目、玩家卡（卡定主角桌的主角）。
struct Materials {
    world_md: String,
    cards: Vec<data::CharacterCard>,
    gm_entries: Vec<(String, String)>,
    player: Option<data::CharacterCard>,
}

impl Materials {
    fn load(root: &Path, world_id: &str) -> Result<Self, String> {
        let gm_entries = data::read_worldbook(root, world_id)
            .map_err(text)?
            .into_iter()
            .filter(|entry| entry.visibility == data::Visibility::Gm && !entry.disabled)
            .map(|entry| (entry.title, entry.content))
            .collect();
        Ok(Self {
            world_md: data::read_world_md(root, world_id).map_err(text)?,
            cards: crate::chat_assembly::active_cards(root, world_id)?,
            gm_entries,
            player: data::read_player_card(root, world_id).map_err(text)?,
        })
    }

    fn source(&self) -> prompts::BibleSource<'_> {
        prompts::BibleSource {
            world_md: &self.world_md,
            cast: self
                .cards
                .iter()
                .map(|card| prompts::CastEntry {
                    name: &card.name,
                    public_md: &card.public_md,
                    private_md: &card.private_md,
                })
                .collect(),
            gm_entries: self
                .gm_entries
                .iter()
                .map(|(title, content)| (title.as_str(), content.as_str()))
                .collect(),
        }
    }

    /// 正規化對秘密持有者用的名單：登場角色＋玩家卡（卡定主角也可以持有秘密）。
    fn cast(&self) -> Vec<(String, String)> {
        self.cards
            .iter()
            .chain(self.player.as_ref())
            .map(|card| (card.id.clone(), card.name.clone()))
            .collect()
    }
}

/// 呼叫並解析；解析不出（含回覆截斷）自動重試一次，再失敗回 `FAILED`。
async fn call_parsed<T>(
    caller: &mut impl SummaryCaller,
    messages: Vec<crate::transport::ChatMessage>,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<T, String> {
    for _ in 0..2 {
        match caller.call(messages.clone()).await {
            Ok(raw) => {
                if let Some(parsed) = parse(&raw) {
                    return Ok(parsed);
                }
            }
            Err(error) if error.starts_with("AI_INCOMPLETE_RESPONSE") => {}
            Err(error) => return Err(error),
        }
    }
    Err(FAILED.to_owned())
}

fn refuse_if_played(root: &Path, world_id: &str) -> Result<(), String> {
    let state = data::read_state(root, world_id).map_err(text)?;
    match crate::refactor::played(root, world_id, &state).map_err(text)? {
        true => Err(PLAYED.to_owned()),
        false => Ok(()),
    }
}

/// 整份生成（精靈 ②、匯入建桌、劇本分頁重生成共用）：先確保 vn.json 在（帶 ① 選的劇透鎖，生成中與失敗後
/// 都有鎖可套），鎖外呼叫模型，成功才套用；失敗時原聖經、私設、狀態一律不動。
pub(crate) async fn generate(
    root: &Path,
    world_id: &str,
    lang: &str,
    spoiler_lock: Option<bool>,
    caller: &mut impl SummaryCaller,
) -> Result<(), String> {
    data::state_commit::with_commit(root, world_id, |tx| vn::create_if_missing(tx, spoiler_lock))
        .map_err(text)?;
    // VN 卡建的桌（卡上有聖經）：重骰改成恢復卡內原版（§2.4），不呼叫模型
    let card = vn::read_vn(root, world_id).map_err(text)?.card;
    if card.is_some_and(|card| card.original_bible.is_some()) {
        return restore_original(root, world_id);
    }
    refuse_if_played(root, world_id)?;
    let materials = Materials::load(root, world_id)?;
    let messages = prompts::bible_messages(&materials.source(), lang);
    let bible = call_parsed(caller, messages, prompts::parse_bible_reply).await?;
    install(root, world_id, &materials.cast(), None, |_| Ok(bible))
}

/// 恢復卡內原版：走同一支套用函式（鎖內重驗未遊玩，聖經、狀態初值、機制規則、快照一起同步）。
pub(crate) fn restore_original(root: &Path, world_id: &str) -> Result<(), String> {
    let original = vn::read_vn(root, world_id)
        .map_err(text)?
        .card
        .and_then(|card| card.original_bible)
        .ok_or(NO_BIBLE)?;
    let cast = Materials::load(root, world_id)?.cast();
    install(root, world_id, &cast, None, |_| Ok(original))
}

/// 角色秘密重骰：只換這個角色的私設與 holder 是他的秘密，聖經其餘不動（鎖內以最新的聖經合併）。
pub(crate) async fn reroll_secrets(
    root: &Path,
    world_id: &str,
    character_id: &str,
    lang: &str,
    caller: &mut impl SummaryCaller,
) -> Result<(), String> {
    refuse_if_played(root, world_id)?;
    let bible = vn::read_vn(root, world_id)
        .map_err(text)?
        .bible
        .ok_or(NO_BIBLE)?;
    let materials = Materials::load(root, world_id)?;
    let card = data::read_character(root, world_id, character_id).map_err(text)?;
    let messages = prompts::secret_messages(&materials.source(), &card.name, &bible, lang);
    let (private, secrets) = call_parsed(caller, messages, prompts::parse_secret_reply).await?;
    let private = vn::cap_chars(&private, vn::PRIVATE_CAP);
    install(
        root,
        world_id,
        &materials.cast(),
        Some((character_id, private)),
        |latest| {
            let mut bible = latest.ok_or(NO_BIBLE)?;
            let mine = |holder: &Option<String>| holder.as_deref() == Some(character_id);
            bible.secrets.retain(|secret| !mine(&secret.holder));
            bible.secrets.extend(secrets.into_iter().map(|mut secret| {
                secret.holder = Some(character_id.to_owned());
                secret
            }));
            Ok(bible)
        },
    )
}

/// 套用聖經（整份生成、整份重骰、角色秘密重骰共用）：同一把短提交鎖內依序做——重驗未遊玩 → state.json
/// （規則、`flags`／`revealed` 子樹、incremental）→ 私設 → vn.json（bible、frozen）。state.json 先寫：中途失敗
/// 只會剩「有規則沒聖經」，下次套用整批換掉；反過來會剩聖經在、規則缺，補種只補樹不補規則，合法值檢查就失效。
/// `build` 拿鎖內重讀的最新聖經產生要套用的那份（之後統一正規化）。VN 卡匯入建桌在新桌獨占內也走這支。
pub(crate) fn install(
    root: &Path,
    world_id: &str,
    cast: &[(String, String)],
    private: Option<(&str, String)>,
    build: impl FnOnce(Option<Bible>) -> Result<Bible, String>,
) -> Result<(), String> {
    data::state_commit::with_commit(root, world_id, |tx| {
        let state = data::read_state(root, world_id).map_err(text)?;
        if crate::refactor::played(root, world_id, &state).map_err(text)? {
            return Err(PLAYED.to_owned());
        }
        let old = vn::read_vn(root, world_id).map_err(text)?.bible;
        let stale: Vec<Vec<String>> = old
            .iter()
            .flat_map(Bible::seeds)
            .map(|(path, _)| path)
            .collect();
        let bible = vn::normalize(build(old)?, cast);
        let seeds = bible.seeds();
        // 舊表的葉子刪掉、新表的寫成初值（未遊玩，等於重置）；章號已有就不動
        let reseed = |tree: &mut BTreeMap<String, StateNode>| {
            for path in stale.iter().filter(|path| path.len() > 1) {
                data::set_tree_value(tree, path, "");
            }
            for (path, initial) in &seeds {
                if path.len() > 1 || data::node_at(tree, path).is_none() {
                    data::set_tree_value(tree, path, initial);
                }
            }
            true
        };
        let variable = data::message_vars::edit_effective_tree(tx, true, false, reseed)
            .map_err(text)?
            .is_some();
        let state = data::update_state(root, world_id, |state| {
            let mechanism = &mut state.mechanism;
            mechanism
                .rules
                .retain(|key, _| !vn::LOCKED_ROOTS.contains(&key.split('.').next().unwrap_or("")));
            mechanism.rules.extend(bible.rules());
            mechanism.incremental = true;
            mechanism.locked_roots = vn::LOCKED_ROOTS.map(str::to_owned).to_vec();
            if !variable {
                reseed(&mut state.state.tree);
            }
            Ok(Some(state.clone()))
        })
        .map_err(text)?
        .expect("update_state 回 Some");
        if !variable {
            data::set_last_transcript_state(root, world_id, state.current_scene, &state.state)
                .map_err(text)?;
        }
        if let Some((character_id, private)) = private {
            let mut card = data::read_character(root, world_id, character_id).map_err(text)?;
            card.private_md = private;
            data::write_character(root, world_id, &card).map_err(text)?;
        }
        let projected = data::read_state(root, world_id).map_err(text)?;
        vn::edit_vn(tx, |file| {
            file.frozen = Some(Frozen {
                scene: state.current_scene,
                chapter: chapter_of(&projected.state.tree),
                bible: bible.clone(),
            });
            file.bible = Some(bible);
        })
        .map_err(text)
    })
}

fn chapter_of(tree: &BTreeMap<String, StateNode>) -> u32 {
    match tree.get("chapter") {
        Some(StateNode::Leaf(value)) => value.trim().parse().unwrap_or(1),
        _ => 1,
    }
}

/// GM 回合的 VN 輸入：注入用的凍結聖經、補種用的初值表（照目前的聖經，跟規則同一份）、VN 卡原件。
#[derive(Default)]
pub(crate) struct TurnInputs {
    pub frozen: Option<Frozen>,
    pub seeds: Vec<(Vec<String>, String)>,
    pub card: Option<vn::CardOrigin>,
}

/// 凍結副本沒有或幕號不符就把當前聖經與章號抄進去寫回（鎖內重讀，與劇本分頁存檔互斥）——幕中改聖經
/// 不動 system，下一幕生效。讀寫失敗當沒有聖經，不擋回合。
pub(crate) fn turn_inputs(root: &Path, world_id: &str, state: &data::WorldState) -> TurnInputs {
    if state.table_kind != data::TableKind::Vn {
        return TurnInputs::default();
    }
    let result = data::state_commit::with_commit(root, world_id, |tx| {
        let file = vn::read_vn(root, world_id)?;
        let Some(bible) = file.bible else {
            return Ok(TurnInputs {
                card: file.card,
                ..TurnInputs::default()
            });
        };
        let seeds = bible.seeds();
        let frozen = match file.frozen {
            Some(frozen) if frozen.scene == state.current_scene => frozen,
            _ => {
                let frozen = Frozen {
                    scene: state.current_scene,
                    chapter: chapter_of(&state.state.tree),
                    bible,
                };
                vn::edit_vn(tx, |file| file.frozen = Some(frozen.clone()))?;
                frozen
            }
        };
        data::DataResult::Ok(TurnInputs {
            frozen: Some(frozen),
            seeds,
            card: file.card,
        })
    });
    result.unwrap_or_else(|error| {
        log::warn!("VN 聖經讀寫失敗，這一輪不注入：{error}");
        TurnInputs::default()
    })
}

/// GM system 附加段：常駐段＋當章包（章號取凍結時的章）。
pub(crate) fn injection(
    lang: &str,
    frozen: &Frozen,
    cards: &[data::CharacterCard],
    player: Option<&data::CharacterCard>,
) -> String {
    let names = |id: &str| {
        cards
            .iter()
            .chain(player)
            .find(|card| card.id == id)
            .map_or_else(|| id.to_owned(), |card| card.name.clone())
    };
    let mut text = prompts::bible_resident(lang, &frozen.bible, &names);
    text.push_str(&prompts::bible_chapter(lang, &frozen.bible, frozen.chapter));
    text
}

/// 劇本分頁存檔：只收文字類欄位（主線、文風、章節文字、秘密文字與揭露時機、旗標說明、結局標題）與章數上限；
/// 結構（章數、旗標、結局、id 與條件）以檔上的為準。鎖內重讀合併，改動下一幕生效（凍結）。
pub(crate) fn save(root: &Path, world_id: &str, edited: Bible) -> Result<(), String> {
    let cast = Materials::load(root, world_id)?.cast();
    data::state_commit::with_commit(root, world_id, |tx| {
        let mut bible = vn::read_vn(root, world_id)
            .map_err(text)?
            .bible
            .ok_or(NO_BIBLE)?;
        // 章數鎖以鎖內讀到的檔為準；請求帶的 chapter_lock 一律不收
        if bible.chapter_lock && edited.chapter_max != bible.chapter_max {
            return Err(CHAPTER_LOCKED.to_owned());
        }
        let old_max = bible.chapter_max;
        bible.logline = edited.logline;
        bible.style = edited.style;
        bible.chapter_max = edited.chapter_max;
        for (chapter, new) in bible.chapters.iter_mut().zip(edited.chapters) {
            *chapter = new;
        }
        for secret in &mut bible.secrets {
            if let Some(new) = edited.secrets.iter().find(|new| new.id == secret.id) {
                secret.title = new.title.clone();
                secret.text = new.text.clone();
                secret.reveal = new.reveal.clone();
                secret.hint = new.hint.clone();
            }
        }
        for flag in &mut bible.flags {
            if let Some(new) = edited.flags.iter().find(|new| new.id == flag.id) {
                flag.label = new.label.clone();
                flag.trigger = new.trigger.clone();
            }
        }
        for ending in &mut bible.endings {
            if let Some(new) = edited.endings.iter().find(|new| new.id == ending.id) {
                ending.title = new.title.clone();
            }
        }
        let mut bible = vn::normalize(bible, &cast);
        // 線性作品的章門檻是正規化自動補的：跟著改過的章數上限走
        if bible.branching == vn::Branching::None {
            vn::move_chapter_gate(&mut bible, old_max);
        }
        vn::edit_vn(tx, |file| file.bible = Some(bible)).map_err(text)
    })
}

pub(crate) fn set_spoiler_lock(root: &Path, world_id: &str, locked: bool) -> Result<(), String> {
    data::state_commit::with_commit(root, world_id, |tx| {
        vn::edit_vn(tx, |file| file.spoiler_lock = locked)
    })
    .map_err(text)
}

/// 格邊旗標提示（§2.7）：這則事件快照的變動裡有 `flags.*` 或 `revealed.*` 就列出。
/// 揭露只列設成 true 的（所以鎖住時也不會露出未揭露秘密的標題）。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum FlagHint {
    Flag {
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mark: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },
    Reveal {
        title: String,
    },
}

pub(crate) fn flag_hints(bible: &Bible, state: &TableState) -> Vec<FlagHint> {
    let leaf = |path: &str| {
        let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
        match data::node_at(&state.tree, &segments) {
            Some(StateNode::Leaf(value)) => Some(value.clone()),
            _ => None,
        }
    };
    state
        .changes
        .iter()
        .filter_map(|(path, mark)| {
            if let Some(id) = path.strip_prefix("flags.") {
                let flag = bible.flags.iter().find(|flag| flag.id == id)?;
                let label = match flag.label.is_empty() {
                    true => flag.id.clone(),
                    false => flag.label.clone(),
                };
                let delta = mark.starts_with(['+', '-']);
                return Some(FlagHint::Flag {
                    label,
                    mark: delta.then(|| mark.clone()),
                    value: (!delta).then(|| leaf(path)).flatten(),
                });
            }
            let id = path.strip_prefix("revealed.")?;
            let secret = bible.secrets.iter().find(|secret| secret.id == id)?;
            (leaf(path).as_deref() == Some("true")).then(|| FlagHint::Reveal {
                title: secret.title.clone(),
            })
        })
        .collect()
}

/// 已揭露清單：目前樹上 `revealed.<id>` 為 true 的秘密，附揭露時所在的章（沿這條幕線往回找那次變動的快照）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Revealed {
    pub id: String,
    pub chapter: Option<u32>,
}

pub(crate) fn revealed(
    root: &Path,
    world_id: &str,
    state: &data::WorldState,
) -> Result<Vec<Revealed>, String> {
    let Some(StateNode::Branch(open)) = state.state.tree.get("revealed") else {
        return Ok(Vec::new());
    };
    let ids: Vec<&String> = open
        .iter()
        .filter(|(_, value)| **value == StateNode::Leaf("true".to_owned()))
        .map(|(id, _)| id)
        .collect();
    let mut chapters: BTreeMap<String, u32> = BTreeMap::new();
    let mut scene = Some(state.current_scene);
    while let Some(current) = scene.filter(|_| chapters.len() < ids.len()) {
        for event in data::read_transcript(root, world_id, current).map_err(text)? {
            let Some(snapshot) = event.state else {
                continue;
            };
            for id in &ids {
                if snapshot.changes.contains_key(&format!("revealed.{id}")) {
                    chapters.insert((*id).clone(), chapter_of(&snapshot.tree));
                }
            }
        }
        scene = data::scene_label(state, current).parent;
    }
    Ok(ids
        .into_iter()
        .map(|id| Revealed {
            chapter: chapters.get(id).copied(),
            id: id.clone(),
        })
        .collect())
}

#[cfg(test)]
#[path = "vn_bible_tests.rs"]
mod tests;
