use std::collections::BTreeSet;
use std::path::Path;

use super::super::character::{list_characters, set_character_auto_hidden};
use super::transcript::{TranscriptEvent, TranscriptKind, read_transcript};



// ---------------------------------------------------------------------
// AI 卡重構包 4b：角色卡自動上下場共用的登場掃描原語。人物（transport::PERSON_ARRIVAL_PREFIX）
// 與角色卡（CARD_ARRIVAL_PREFIX）登場比對邏輯相同、鍵不同，這裡放兩邊都用得到、且
// 換幕結算（本檔 begin_next_scene）必須直接呼叫、不能反過來依賴 transport 的最小共用集合。
// ---------------------------------------------------------------------

/// 角色卡回歸事件的固定前綴，接著是〈name〉那一行——跟世界書人物的登場前綴
/// （transport::PERSON_ARRIVAL_PREFIX）分開，掃 transcript 或前端呈現時才分得出兩種來源。
pub const CARD_ARRIVAL_PREFIX: &str = "（角色回歸）";

/// 從一則事件文字剝出前綴後的〈title〉；prefix 不符或沒有〈〉包住就回 None。
pub(crate) fn bracket_title(text: &str, prefix: &str) -> Option<String> {
    let rest = text.strip_prefix(prefix)?.strip_prefix('〈')?;
    let end = rest.find('〉')?;
    Some(rest[..end].to_owned())
}

/// 本幕已登場（依指定前綴）集合：掃 System 事件取出〈title〉。
pub(crate) fn appeared_titles(events: &[TranscriptEvent], prefix: &str) -> BTreeSet<String> {
    events
        .iter()
        .filter(|event| event.kind == TranscriptKind::System)
        .filter_map(|event| bracket_title(&event.text, prefix))
        .collect()
}

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
pub(super) fn settle_card_visibility(root: &Path, world_id: &str, ended_scene: u64, present: Option<&str>) {
    let Ok(characters) = list_characters(root, world_id) else {
        return;
    };
    let events = read_transcript(root, world_id, ended_scene).unwrap_or_default();
    let arrived = appeared_titles(&events, CARD_ARRIVAL_PREFIX);
    let present_names = present.map(split_present_names);
    for meta in characters {
        if meta.archived {
            continue;
        }
        let appeared = arrived.iter().any(|name| name_matches(name, &meta.name))
            || present_names
                .as_ref()
                .is_some_and(|names| names.iter().any(|name| name_matches(name, &meta.name)));
        let _ = set_character_auto_hidden(root, world_id, &meta.id, !appeared);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::*;
    use crate::data::test_support::*;

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
                text: "（角色回歸）〈狐狸〉\n尾巴很大。".to_owned(),
                raw: None,
                state: None,
                gm_only: false,
            },
        )
        .unwrap();

        let mut state = read_state(root.path(), &world_id).unwrap();
        state
            .state
            .table
            .insert("present".to_owned(), "狐狸、熊".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();

        begin_next_scene(root.path(), &world_id, "摘要", "zh-TW", None).unwrap();

        let metas = list_characters(root.path(), &world_id).unwrap();
        let auto_hidden_of = |id: &str| metas.iter().find(|meta| meta.id == id).unwrap().auto_hidden;
        assert!(!auto_hidden_of(&fox.id), "本幕有回歸事件的卡應該結算成主區");
        assert!(!auto_hidden_of(&bear.id), "present 命中的卡應該結算成主區");
        assert!(
            auto_hidden_of(&badger.id),
            "沒出現過的卡（就算原本在主區）應該結算成隱藏"
        );
        assert!(!auto_hidden_of(&ghost.id), "archived 的卡完全不受結算影響");
    }

}
