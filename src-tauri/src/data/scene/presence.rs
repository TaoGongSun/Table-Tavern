use std::collections::BTreeSet;
use std::path::Path;

use super::super::character::{list_characters, set_character_auto_hidden};
use super::transcript::read_transcript;

/// present 欄的斷詞規則：頓號／逗號／斜線／分號，trim 後濾空。
pub(crate) fn split_present_names(raw: &str) -> Vec<String> {
    raw.split(['、', '，', ',', '／', '/', '；', ';'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

/// 在場名字跟標題比對：雙向包含，「亞歷山大」對得上「亞歷山大・馮・史特勞斯」。
pub(crate) fn name_matches(name: &str, title: &str) -> bool {
    title.contains(name) || name.contains(title)
}

/// 在場名單對一組候選名字（卡名或人物條目標題，索引＝候選位置）的比對結果。
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct PresenceMatch {
    /// 確定在場：原文精確或原文包含命中，或折疊後唯一命中
    pub matched: BTreeSet<usize>,
    /// 折疊後多個候選同時命中、又分不出哪一個：結算不隱藏它們，但也不替任何一個送全文
    pub ambiguous: BTreeSet<usize>,
}

/// 每個在場名字依序比：原文精確相等 → 原文雙向包含（既有語意，可多個）→ 簡繁折疊後雙向包含。
/// 折疊這一步命中多個時，只認折疊後與名字完全相等的那一個；沒有或不只一個＝歧義。
/// 折疊會誤撞（沈郁／沈鬱），所以原文命中時不再看折疊。
pub(crate) fn match_present(names: &[String], candidates: &[&str]) -> PresenceMatch {
    match_present_with(names, candidates, name_matches)
}

/// `match_present` 的本體，包含比對可換：`matches(在場名字, 候選)`。狀態樹裁切用單向包含
/// （在場項目含卡名），登場與換幕結算用雙向包含；原文優先與折疊歧義的判法兩邊同一套。
///
/// 同一個原文名字（人物分支名與人物條目標題、角色卡名常常同名）只算一個候選：判完再映回每個位置，
/// 否則「铁三叔」出現兩次會讓折疊後的唯一完全相等變成假歧義。不同原文之間的折疊歧義照留。
pub(crate) fn match_present_with(
    names: &[String],
    candidates: &[&str],
    matches: impl Fn(&str, &str) -> bool,
) -> PresenceMatch {
    let mut unique: Vec<&str> = Vec::new();
    let mut slot_of: Vec<usize> = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let slot = unique
            .iter()
            .position(|seen| seen == candidate)
            .unwrap_or_else(|| {
                unique.push(candidate);
                unique.len() - 1
            });
        slot_of.push(slot);
    }
    let found = match_unique(names, &unique, matches);
    let spread = |slots: &BTreeSet<usize>| -> BTreeSet<usize> {
        (0..candidates.len())
            .filter(|&index| slots.contains(&slot_of[index]))
            .collect()
    };
    PresenceMatch {
        matched: spread(&found.matched),
        ambiguous: spread(&found.ambiguous),
    }
}

/// `match_present_with` 的比對本體，候選已去重。
fn match_unique(
    names: &[String],
    candidates: &[&str],
    matches: impl Fn(&str, &str) -> bool,
) -> PresenceMatch {
    let mut result = PresenceMatch::default();
    for name in names {
        let exact: Vec<usize> = (0..candidates.len())
            .filter(|&index| candidates[index] == name.as_str())
            .collect();
        let raw = if exact.is_empty() {
            (0..candidates.len())
                .filter(|&index| matches(name, candidates[index]))
                .collect()
        } else {
            exact
        };
        if !raw.is_empty() {
            result.matched.extend(raw);
            continue;
        }
        let folded_name = super::glyph::fold_glyphs(name);
        let folded: Vec<(usize, String)> = candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| (index, super::glyph::fold_glyphs(candidate).into_owned()))
            .filter(|(_, candidate)| matches(&folded_name, candidate))
            .collect();
        match folded.as_slice() {
            [] => {}
            [(index, _)] => {
                result.matched.insert(*index);
            }
            many => {
                let equal: Vec<usize> = many
                    .iter()
                    .filter(|(_, candidate)| *candidate == folded_name.as_ref())
                    .map(|(index, _)| *index)
                    .collect();
                if let [index] = equal.as_slice() {
                    result.matched.insert(*index);
                } else {
                    result
                        .ambiguous
                        .extend(many.iter().map(|(index, _)| *index));
                }
            }
        }
    }
    let matched = result.matched.clone();
    result.ambiguous.retain(|index| !matched.contains(index));
    result
}

/// 換幕結算角色卡自動隱藏（AI 卡重構包 4b；鐵律：auto_hidden 這個持久欄位只在換幕結算與退回／分岔復原動，
/// 幕中回合的登場偵測只 append 事件不改欄位，見 lib.rs record_card_arrivals）。
///
/// 出現過＝(a) 剛結束那幕的角色卡回歸事件集合 ∪ (b) 換幕當下 present 名單比對命中。
/// 出現過→auto_hidden=false（拉回主區）；沒出現過→auto_hidden=true（收進隱藏區）；
/// archived（手動封存）的卡完全不動，自動判斷不能覆蓋玩家的手動決定。
/// 舞台卡（stage-card-in-speaker-list）同樣完全不動：它提供設定、不是人物，GM 不會把它寫進 present。
///
/// 已知限制：幕開始就在主區、全程活躍，但最後一輪 GM 忘記把它列進 present、
/// 這幕本文也沒有登場事件（因為它本來就沒被隱藏過）的卡，會在這裡被判定「沒出現」
/// 而轉為隱藏——(a)(b) 都掃不到這種情況；真正掃「正文有沒有提到名字」(c) 成本較高
/// （要跑完整幕全部旁白文字），先不做，之後真的常誤判再考慮補。
///
/// 結算失敗一律吞掉：換幕本身已經成功，auto_hidden 記帳不該反過來讓換幕報錯。
///
/// `requires_present_report`（`data::requires_present_report`）：這桌的 GM 回合有沒有被要求報 present。
/// 沒被要求的桌（介面接管桌、沒重構的顯示腳本桌）present 缺席＝未知不是沒人：不結算、旗標原樣保留，
/// 否則原卡主角色會被當成「沒出現」收進隱藏區，GM 上下文就少了那張卡。有被要求的桌照原行為結算。
/// 已知限制：主角色已被藏的顯示腳本桌，`read_card_interfaces` 略過隱藏卡 → 判定掉回一般旁白、照樣結算，
/// 只能玩家手動拉回〔作者裁決 2026-10-27：舊桌不拉回〕。
pub(super) fn settle_card_visibility(
    root: &Path,
    world_id: &str,
    ended_scene: u64,
    present: Option<&str>,
    requires_present_report: bool,
) {
    if present.is_none() && !requires_present_report {
        return;
    }
    let Ok(characters) = list_characters(root, world_id) else {
        return;
    };
    // 舞台卡不參與結算、也不進比對名字池（它不是人物）；比對結果的索引對應過濾後的這份清單
    let characters: Vec<_> = characters.into_iter().filter(|meta| !meta.stage).collect();
    let events = read_transcript(root, world_id, ended_scene).unwrap_or_default();
    let arrived = super::marker::appeared_card_names(&events);
    let present_names = present.map(split_present_names).unwrap_or_default();
    // 整桌人物名字一個候選池（角色卡＋人物條目，與登場比對同一套）〔作者裁決 2026-10-27〕：
    // present 寫的是人物條目「沈郁」時，角色卡「沈鬱」不會因為折疊後同名就被當成出現過。
    // 世界書讀不到就只比卡名（結算本來就盡力而為）。
    let person_titles: Vec<String> = crate::data::read_worldbook(root, world_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| entry.is_person && !entry.disabled)
        .map(|entry| entry.title)
        .collect();
    let names: Vec<&str> = characters
        .iter()
        .map(|meta| meta.name.as_str())
        .chain(person_titles.iter().map(String::as_str))
        .collect();
    // 歧義（折疊後分不出是誰）的卡一律當出現過：寧可留在主區，不誤收進隱藏區
    let present_match = match_present(&present_names, &names);
    for (index, meta) in characters.iter().enumerate() {
        if meta.archived {
            continue;
        }
        let appeared = arrived.iter().any(|name| name_matches(name, &meta.name))
            || present_match.matched.contains(&index)
            || present_match.ambiguous.contains(&index);
        let _ = set_character_auto_hidden(root, world_id, &meta.id, !appeared);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::test_support::*;
    use crate::data::*;

    /// AI 卡重構包 4b：換幕結算角色卡自動隱藏。出現過＝本幕有回歸事件 (a) 或換幕當下
    /// present 名單命中 (b)；兩者都沒有（就算幕開始時本來在主區）結算成隱藏；
    /// archived 的卡完全不受結算影響。
    #[test]
    fn begin_next_scene_settles_card_auto_hidden() {
        let root = TestRoot::new("card-settlement");
        let world_id = create_world(root.path(), "測試桌").unwrap();

        let fox = character_card(&new_id(), "狐狸"); // (a) 本幕有回歸事件
        let bear = character_card(&new_id(), "熊"); // (b) present 命中
        let badger = character_card(&new_id(), "獾"); // 兩者都沒有 → 結算成隱藏
        let ghost = character_card(&new_id(), "亡靈"); // archived → 完全不動
        for card in [&fox, &bear, &badger, &ghost] {
            write_character(root.path(), &world_id, card).unwrap();
        }
        set_character_auto_hidden(root.path(), &world_id, &fox.id, true).unwrap();
        set_character_auto_hidden(root.path(), &world_id, &bear.id, true).unwrap();
        set_character_auto_hidden(root.path(), &world_id, &badger.id, false).unwrap();
        set_character_auto_hidden(root.path(), &world_id, &ghost.id, false).unwrap();
        set_character_archived(root.path(), &world_id, &ghost.id, true).unwrap();

        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                id: None,
                message_vars: None,
                vars_rev: None,
                vars_epoch: None,
                turn_key: None,
                action_id: None,
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::System,
                text: "尾巴很大。".to_owned(),
                raw: None,
                state: None,
                truncated: false,
                gm_only: false,
                marker: Some(super::super::marker::EventMarker::CardArrival {
                    name: "狐狸".to_owned(),
                }),
                opening: false,
            },
        )
        .unwrap();

        let mut state = read_state(root.path(), &world_id).unwrap();
        state
            .state
            .table
            .insert("present".to_owned(), "熊".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();

        begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();

        let metas = list_characters(root.path(), &world_id).unwrap();
        let auto_hidden_of =
            |id: &str| metas.iter().find(|meta| meta.id == id).unwrap().auto_hidden;
        assert!(!auto_hidden_of(&fox.id), "本幕有回歸事件的卡應該結算成主區");
        assert!(!auto_hidden_of(&bear.id), "present 命中的卡應該結算成主區");
        assert!(
            auto_hidden_of(&badger.id),
            "沒出現過的卡（就算原本在主區）應該結算成隱藏"
        );
        assert!(!auto_hidden_of(&ghost.id), "archived 的卡完全不受結算影響");
    }

    /// 舞台卡（stage-card-in-speaker-list）：present 不會寫它，結算也不收進隱藏區；名字池排除它之後，
    /// 排在它後面的人物仍對得上自己的比對結果（舞台卡排第一，索引錯位就會把熊判錯）。
    #[test]
    fn settlement_skips_stage_cards_and_keeps_person_indices() {
        let root = TestRoot::new("card-settlement-stage");
        let world_id = create_world(root.path(), "測試桌").unwrap();
        let stage = character_card(&new_id(), "灰岩要塞");
        let bear = character_card(&new_id(), "熊");
        let badger = character_card(&new_id(), "獾");
        for card in [&stage, &bear, &badger] {
            write_character(root.path(), &world_id, card).unwrap();
        }
        set_character_stage(root.path(), &world_id, &stage.id, true).unwrap();
        set_character_auto_hidden(root.path(), &world_id, &bear.id, true).unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state
            .state
            .table
            .insert("present".to_owned(), "熊".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();

        begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();

        let metas = list_characters(root.path(), &world_id).unwrap();
        let meta_of = |id: &str| metas.iter().find(|meta| meta.id == id).unwrap();
        assert!(!meta_of(&stage.id).auto_hidden, "舞台卡不收進隱藏區");
        assert!(meta_of(&stage.id).stage);
        assert!(
            !meta_of(&bear.id).auto_hidden,
            "present 命中的人物照常拉回主區"
        );
        assert!(meta_of(&badger.id).auto_hidden, "缺席的人物照常隱藏");
    }

    /// present 缺席時只有「沒被要求報 present」的桌（介面接管、沒重構的顯示腳本桌）不結算，旗標原樣保留
    /// （配角照樣藏著，不會被順手拉回）；一般桌、characters 桌與不套卡內腳本的桌照原行為，沒回歸事件的卡
    /// 結算成隱藏。顯示腳本桌 present 有值（含空字串）照常結算。
    #[test]
    fn missing_present_skips_settlement_only_where_present_is_not_requested() {
        // (label, 骨架, 玩法, 主角色帶顯示腳本, 套卡內腳本, present, 主角色結算後隱藏, 配角結算後隱藏)
        #[rustfmt::skip]
        let cases = [
            ("takeover", true, Some("interface"), false, true, None, false, true),
            ("takeover-no-mode", true, None, false, true, None, false, true),
            ("characters", true, Some("characters"), false, true, None, true, true),
            ("plain", false, None, false, true, None, true, true),
            ("interface-no-shell", false, Some("interface"), false, true, None, true, true),
            ("card-format", false, None, true, true, None, false, true),
            ("card-format-regex-off", false, None, true, false, None, true, true),
            ("card-format-empty-present", false, None, true, true, Some(""), true, true),
            ("card-format-present-hit", false, None, true, true, Some("周掌櫃"), true, false),
        ];
        for (label, shell, mode, scripts, regex_allowed, present, host_after, other_after) in cases
        {
            let root = TestRoot::new(&format!("card-settlement-no-present-{label}"));
            let world_id = create_world(root.path(), "桌").unwrap();
            let host_id = if scripts {
                // 啟用中、placement 含 2 的顯示腳本：read_card_interfaces 認得的那種
                let raw = serde_json::json!({"data": {"name": "北境驛站", "extensions": {
                    "regex_scripts": [{
                        "scriptName": "狀態欄",
                        "findRegex": "/<Status_block>[\\s\\S]*?<\\/Status_block>/",
                        "replaceString": "<div>狀態</div>",
                        "placement": [2]
                    }]
                }}})
                .to_string();
                crate::import::import_character(
                    root.path(),
                    &world_id,
                    raw.as_bytes(),
                    "#3366ff",
                    "zh-TW",
                )
                .unwrap()
                .id
            } else {
                let host = character_card(&new_id(), "北境驛站");
                write_character(root.path(), &world_id, &host).unwrap();
                host.id
            };
            let other = character_card(&new_id(), "周掌櫃");
            write_character(root.path(), &world_id, &other).unwrap();
            set_character_auto_hidden(root.path(), &world_id, &host_id, false).unwrap();
            set_character_auto_hidden(root.path(), &world_id, &other.id, true).unwrap();
            if shell {
                crate::data::write_interface_shell(
                    root.path(),
                    &world_id,
                    "<Status_block>\n{{地點}}\n</Status_block>",
                )
                .unwrap();
            }
            let mut state = read_state(root.path(), &world_id).unwrap();
            state.refactor_mode = mode.map(str::to_owned);
            state.regex_allowed = regex_allowed;
            if let Some(present) = present {
                state
                    .state
                    .table
                    .insert("present".to_owned(), present.to_owned());
            }
            write_state(root.path(), &world_id, &state).unwrap();
            if scripts && regex_allowed {
                let interfaces =
                    crate::import::read_card_interfaces(root.path(), &world_id).unwrap();
                assert_eq!(interfaces[0].scripts.len(), 1, "{label}: 應是顯示腳本桌");
            }

            begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();

            let metas = list_characters(root.path(), &world_id).unwrap();
            let auto_hidden_of =
                |id: &str| metas.iter().find(|meta| meta.id == id).unwrap().auto_hidden;
            assert_eq!(auto_hidden_of(&host_id), host_after, "{label}: 主角色");
            assert_eq!(auto_hidden_of(&other.id), other_after, "{label}: 配角");
        }
    }

    fn present(raw: &str) -> Vec<String> {
        split_present_names(raw)
    }

    /// 原文精確或原文包含命中就不看折疊：沈郁／沈鬱 折疊後相同，但寫哪個就是哪個
    #[test]
    fn match_present_prefers_raw_hits_over_glyph_folding() {
        let cards = ["沈郁", "沈鬱", "贺铮"];
        let found = match_present(&present("沈郁、賀錚"), &cards);
        assert_eq!(found.matched.into_iter().collect::<Vec<_>>(), vec![0, 2]);
        assert!(found.ambiguous.is_empty());
        let found = match_present(&present("沈鬱先生"), &cards);
        assert_eq!(found.matched.into_iter().collect::<Vec<_>>(), vec![1]);
    }

    /// 只靠折疊命中多張卡：折疊後與名字完全相等的唯一一張勝出；分不出來＝歧義，不算確定在場
    #[test]
    fn match_present_resolves_or_flags_fold_only_collisions() {
        let found = match_present(&present("鍾"), &["钟", "钟离"]);
        assert_eq!(found.matched.into_iter().collect::<Vec<_>>(), vec![0]);
        let found = match_present(&present("瀋鬱"), &["沈郁", "沈鬱"]);
        assert!(found.matched.is_empty());
        assert_eq!(found.ambiguous.into_iter().collect::<Vec<_>>(), vec![0, 1]);
    }

    /// 同一個原文名字出現在多個位置（分支名＝人物條目標題）只算一個候選：「鐵三叔」折疊後唯一完全相等的
    /// 仍是「铁三叔」（兩個位置都命中），不連帶「铁三」；精確寫「铁三叔」也一樣，不因包含而留「铁三」
    #[test]
    fn duplicate_raw_candidates_count_once() {
        let pool = ["铁三", "铁三叔", "铁三叔"];
        for raw in ["鐵三叔", "铁三叔"] {
            let found = match_present_with(&present(raw), &pool, |item, name| item.contains(name));
            assert_eq!(
                found.matched.into_iter().collect::<Vec<_>>(),
                vec![1, 2],
                "{raw}"
            );
            assert!(found.ambiguous.is_empty(), "{raw}");
        }
        let found = match_present(&present("瀋鬱"), &["沈郁", "沈鬱", "沈郁"]);
        assert_eq!(
            found.ambiguous.into_iter().collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    /// 換幕結算：繁體 present 讓簡體卡名留在主區；折疊歧義的兩張都不隱藏
    #[test]
    fn settlement_keeps_cards_named_in_another_script_and_never_hides_ambiguous_ones() {
        let root = TestRoot::new("card-settlement-glyph");
        let world_id = create_world(root.path(), "桌").unwrap();
        let he = character_card(&new_id(), "贺铮");
        let iron = character_card(&new_id(), "铁三");
        let shen_a = character_card(&new_id(), "沈郁");
        let shen_b = character_card(&new_id(), "沈鬱");
        let absent = character_card(&new_id(), "秦岚");
        for card in [&he, &iron, &shen_a, &shen_b, &absent] {
            write_character(root.path(), &world_id, card).unwrap();
            set_character_auto_hidden(root.path(), &world_id, &card.id, false).unwrap();
        }
        let mut state = read_state(root.path(), &world_id).unwrap();
        state
            .state
            .table
            .insert("present".to_owned(), "林遠、賀錚、鐵三、瀋鬱".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();

        begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();

        let metas = list_characters(root.path(), &world_id).unwrap();
        let hidden = |id: &str| metas.iter().find(|meta| meta.id == id).unwrap().auto_hidden;
        assert!(!hidden(&he.id) && !hidden(&iron.id));
        assert!(!hidden(&shen_a.id) && !hidden(&shen_b.id), "歧義不隱藏");
        assert!(hidden(&absent.id));
    }

    /// 換幕結算與登場共用整桌名字池：present 寫人物條目「沈郁」時，角色卡「沈鬱」照常收進隱藏區；
    /// 寫「瀋鬱」（卡與人物條目折疊後都像）分不出是誰，卡留主區。
    #[test]
    fn settlement_shares_the_name_pool_with_person_entries() {
        for (present, card_hidden_after) in [("沈郁", true), ("瀋鬱", false)] {
            let root = TestRoot::new("card-settlement-person-pool");
            let world_id = create_world(root.path(), "桌").unwrap();
            let card = character_card(&new_id(), "沈鬱");
            write_character(root.path(), &world_id, &card).unwrap();
            set_character_auto_hidden(root.path(), &world_id, &card.id, false).unwrap();
            upsert_worldbook_entry(
                root.path(),
                &world_id,
                WorldbookEntry {
                    is_person: true,
                    constant: true,
                    ..worldbook_entry(1, "沈郁")
                },
            )
            .unwrap();
            let mut state = read_state(root.path(), &world_id).unwrap();
            state
                .state
                .table
                .insert("present".to_owned(), present.to_owned());
            write_state(root.path(), &world_id, &state).unwrap();

            begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();

            let metas = list_characters(root.path(), &world_id).unwrap();
            assert_eq!(metas[0].auto_hidden, card_hidden_after, "present={present}");
        }
    }
}
