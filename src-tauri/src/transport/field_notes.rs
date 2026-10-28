//! 退場桌開場補齊的欄位說明（statusbar-only-fields 第 11 版，進凍結快照）：逐格說明追蹤什麼，再加一句同回合更新提示。
//! 只讀套用時存下的 `mechanism.opening`（註記與旗標），不讀狀態樹、present 或本回合文字，system 才不會隨狀態變。
//! 不帶原卡整段模板，避免模型照模板重印整個狀態欄。

use crate::data::Mechanism;

use super::messages::scaffold_en;

fn pointer(path: &str) -> String {
    path.split('.')
        .map(|segment| format!("/{}", segment.replace('~', "~0").replace('/', "~1")))
        .collect()
}

pub(super) fn section(mechanism: &Mechanism, lang: &str) -> String {
    let en = scaffold_en(lang);
    let mut lines = vec![if en {
        "## What each field tracks\n\nThis only explains what each field tracks; it is not an output format. Never reprint the whole status panel in the story or in the update block.".to_owned()
    } else {
        "## 各格追蹤什麼\n\n下面說明每一格追蹤什麼，不是輸出格式；不要在正文或更新區塊重印整個狀態欄。".to_owned()
    }];
    for (path, note) in &mechanism.opening.field_notes {
        lines.push(if en {
            format!("- `{}`: tracks {note}", pointer(path))
        } else {
            format!("- `{}`：追蹤{note}", pointer(path))
        });
    }
    lines.push(if en {
        "Whenever the story describes a field's state changing (time passing, weather turning, supplies coming in or going out, stamina spent, a change of clothes or an injury), write that field into the update block in the same turn. When a lot of time passes (e.g. overnight), check every field for changes.".to_owned()
    } else {
        "正文描述某一格的狀態發生變化（時間推進、天氣轉變、物資進出、體力消耗、換裝或受傷），同一回合就在更新區塊寫進那一格；時間大幅推進（例如過夜）時，逐格檢查是否有變化。".to_owned()
    });
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::super::context::gm_system_prompt;
    use crate::data::{Mechanism, NumericUpdate};

    fn system(mechanism: &Mechanism) -> String {
        gm_system_prompt(
            &crate::world_scan::PromptTexts::default(),
            &[],
            None,
            &[],
            &[],
            mechanism,
            "zh-TW",
            false,
            &Default::default(),
        )
    }

    fn tracked() -> Mechanism {
        let mut mechanism = Mechanism {
            incremental: true,
            ..Mechanism::default()
        };
        mechanism.opening.tracked = true;
        mechanism.opening.field_notes.insert(
            "状态栏.人物列表.*.体力".to_owned(),
            "0-100 的数值与简述".to_owned(),
        );
        mechanism
    }

    /// 開場補齊桌：附逐格說明與同回合提示；本桌範例是空的也照附；沒有註記只剩開頭與結尾。
    #[test]
    fn opening_tracked_tables_get_notes_and_same_turn_reminder() {
        let text = system(&tracked());
        assert!(text.contains("## 各格追蹤什麼"), "{text}");
        assert!(
            text.contains("- `/状态栏/人物列表/*/体力`：追蹤0-100 的数值与简述"),
            "{text}"
        );
        assert!(text.contains("同一回合就在更新區塊寫進那一格"), "{text}");
        let mut empty = tracked();
        empty.opening.field_notes.clear();
        let text = system(&empty);
        assert!(
            text.contains("## 各格追蹤什麼") && text.contains("同一回合就在更新區塊寫進那一格")
        );
        assert!(!text.contains("- `/"), "{text}");
    }

    /// 非退場桌、介面接管桌、MVU（照上游）桌：開場資料不影響 GM 提示，逐字不變。
    #[test]
    fn other_tables_are_untouched() {
        let plain = Mechanism {
            incremental: true,
            ..Mechanism::default()
        };
        assert!(!system(&plain).contains("各格追蹤什麼"));
        let mut interface = tracked();
        interface.guide = "地點每回合必報".to_owned();
        let mut interface_plain = interface.clone();
        interface_plain.opening = Default::default();
        assert_eq!(system(&interface), system(&interface_plain));
        let mut upstream = tracked();
        upstream.numeric_update = NumericUpdate::Upstream;
        let mut upstream_plain = upstream.clone();
        upstream_plain.opening = Default::default();
        assert_eq!(system(&upstream), system(&upstream_plain));
    }
}
