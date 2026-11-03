use super::*;

fn instruction(lang: &str, cue: ChapterCue) -> String {
    let direction = VnDirection {
        chapter: Some(cue),
        ..VnDirection::default()
    };
    vn_narrate_instruction(lang, &[], None, false, &direction).content
}

fn cue(chapter: u32, finale: bool) -> ChapterCue {
    ChapterCue {
        chapter,
        max: 4,
        end_condition: Some("抵達燈塔".to_owned()),
        finale,
        holding: vec!["true_end".to_owned()],
    }
}

/// 回合尾章提示四種分支（包 5 §2.4）：一般章、收束（M−1）、最後一章、強制收尾；zh／en 各一
#[test]
fn chapter_cue_covers_each_stage_in_both_scaffolds() {
    let normal = instruction("zh-TW", cue(1, false));
    assert!(
        normal.contains("第 1／4 章")
            && normal.contains("抵達燈塔")
            && !normal.contains("收束模式")
    );
    assert!(instruction("zh-TW", cue(3, false)).contains("收束模式"));
    let last = instruction("zh-TW", cue(4, false));
    assert!(last.contains("最後一章") && last.contains("true_end") && !last.contains("結束條件"));
    let finale = instruction("zh-TW", cue(4, true));
    assert!(finale.contains("強制收尾") && finale.contains("[ending: default]"));
    let en = instruction("en", cue(3, false));
    assert!(en.contains("Chapter 3/4") && en.contains("Convergence mode"));
    assert!(instruction("en", cue(2, true)).contains("(Final beat)"));
    // 沒聖經：一般章不提示，只有最後一章與強制收尾
    let bare = ChapterCue {
        end_condition: None,
        holding: Vec::new(),
        ..cue(1, false)
    };
    assert!(!instruction("zh-TW", bare.clone()).contains("[chapter]"));
    assert!(
        instruction("zh-TW", ChapterCue { chapter: 4, ..bare }).contains("目前條件成立的結局：無")
    );
}

/// GM 規範教 `[chapter]`／`[ending]`；角色線禁寫清單也列上
#[test]
fn format_rules_teach_chapter_and_ending_markers() {
    for lang in ["zh-TW", "en"] {
        let rules = vn_format_rules(lang, false, false);
        assert!(
            rules.contains("[chapter]") && rules.contains("[ending:"),
            "{lang}"
        );
        let character = vn_character_rules(lang);
        assert!(
            character.contains("[chapter]") && character.contains("[ending]"),
            "{lang}"
        );
    }
}
