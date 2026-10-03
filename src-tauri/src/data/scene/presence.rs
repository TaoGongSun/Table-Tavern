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

/// 換幕結算角色卡自動隱藏（AI 卡重構包 4b；鐵律：auto_hidden 這個持久欄位只在換幕動，
/// 幕中回合的登場偵測只 append 事件不改欄位，見 lib.rs record_card_arrivals）。
///
/// 出現過＝(a) 剛結束那幕的角色卡回歸事件集合 ∪ (b) 換幕當下 present 名單比對命中。
/// 出現過→auto_hidden=false（拉回主區）；沒出現過→auto_hidden=true（收進隱藏區）；
/// archived（手動封存）的卡完全不動，自動判斷不能覆蓋玩家的手動決定。
///
/// 已知限制：幕開始就在主區、全程活躍，但最後一輪 GM 忘記把它列進 present、
/// 這幕本文也沒有登場事件（因為它本來就沒被隱藏過）的卡，會在這裡被判定「沒出現」
/// 而轉為隱藏——(a)(b) 都掃不到這種情況；真正掃「正文有沒有提到名字」(c) 成本較高
/// （要跑完整幕全部旁白文字），先不做，之後真的常誤判再考慮補。
///
/// 結算失敗一律吞掉：換幕本身已經成功，auto_hidden 記帳不該反過來讓換幕報錯。
///
/// 介面由 App 接管的桌（有重構骨架、不是 characters 桌，與 transport::gm_turn_format 同一依據）
/// 不寫 state 圍欄、沒有在場名單：present 缺席時不結算，否則原卡主角色會被當成「沒出現」收進隱藏區，
/// GM 上下文就少了那張卡。其他桌（含 characters 與沒重構的桌）照原行為結算。
pub(super) fn settle_card_visibility(
    root: &Path,
    world_id: &str,
    ended_scene: u64,
    present: Option<&str>,
    interface_takeover: bool,
) {
    if present.is_none() && interface_takeover {
        return;
    }
    let Ok(characters) = list_characters(root, world_id) else {
        return;
    };
    let events = read_transcript(root, world_id, ended_scene).unwrap_or_default();
    let arrived = super::marker::appeared_card_names(&events);
    let present_names = present.map(split_present_names).unwrap_or_default();
    for meta in characters {
        if meta.archived {
            continue;
        }
        let appeared = arrived.iter().any(|name| name_matches(name, &meta.name))
            || present_names
                .iter()
                .any(|name| name_matches(name, &meta.name));
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

    /// present 缺席時：介面接管桌（有骨架、不是 characters 桌）不結算；一般桌與 characters 桌照原行為，
    /// 沒有回歸事件的卡結算成隱藏。
    #[test]
    fn missing_present_skips_settlement_only_on_takeover_tables() {
        for (label, shell, mode, host_hidden_after) in [
            ("takeover", true, Some("interface"), false),
            ("takeover-no-mode", true, None, false),
            ("characters", true, Some("characters"), true),
            ("plain", false, None, true),
            ("interface-no-shell", false, Some("interface"), true),
        ] {
            let root = TestRoot::new(&format!("card-settlement-no-present-{label}"));
            let world_id = create_world(root.path(), "桌").unwrap();
            let host = character_card(&new_id(), "北境驛站");
            let hidden = character_card(&new_id(), "周掌櫃");
            for card in [&host, &hidden] {
                write_character(root.path(), &world_id, card).unwrap();
            }
            set_character_auto_hidden(root.path(), &world_id, &host.id, false).unwrap();
            set_character_auto_hidden(root.path(), &world_id, &hidden.id, true).unwrap();
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
            write_state(root.path(), &world_id, &state).unwrap();
            assert!(state.state.table.get("present").is_none());

            begin_next_scene(root.path(), &world_id, "摘要", None).unwrap();

            let metas = list_characters(root.path(), &world_id).unwrap();
            let auto_hidden_of =
                |id: &str| metas.iter().find(|meta| meta.id == id).unwrap().auto_hidden;
            assert_eq!(auto_hidden_of(&host.id), host_hidden_after, "{label}");
            assert!(auto_hidden_of(&hidden.id), "{label}");
        }
    }
}
