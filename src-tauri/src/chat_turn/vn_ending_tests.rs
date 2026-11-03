use super::*;
use crate::data::Condition;

fn ending(id: &str, when: Vec<Condition>) -> Ending {
    Ending {
        id: id.to_owned(),
        title: String::new(),
        when,
    }
}

fn table() -> Vec<Ending> {
    vec![
        ending(
            "true_end",
            vec![Condition::Flag {
                path: "flags.met".to_owned(),
                expect: true,
            }],
        ),
        ending(DEFAULT_ENDING, Vec::new()),
    ]
}

fn tree(met: bool) -> BTreeMap<String, StateNode> {
    let mut tree = BTreeMap::new();
    crate::data::set_tree_value(
        &mut tree,
        &["flags".to_owned(), "met".to_owned()],
        &met.to_string(),
    );
    tree
}

fn judge(text: &str, met: bool, chapter: u32, finale: bool) -> Verdict {
    let endings = table();
    let tree = tree(met);
    let mechanism = Mechanism::default();
    Judge {
        endings: &endings,
        chapter,
        max: 3,
        finale,
        tree: &tree,
        mechanism: &mechanism,
    }
    .judge(text)
}

/// 一般回合：條件成立的結局接受、文字原樣；選項由讀取時的優先序剝
#[test]
fn accepts_an_ending_whose_conditions_hold() {
    let text = "終於到了。\n[ending: true_end]\n[choices]\n- 甲\n- 乙";
    let verdict = judge(text, true, 2, false);
    assert_eq!(verdict.ending.as_deref(), Some("true_end"));
    assert!(verdict.notes.is_empty());
    assert_eq!(verdict.rewrite(text), text);
}

/// 條件不成立、不在表內、一般回合寫 default：剝行、回饋、不轉 default
#[test]
fn refuses_unmet_unknown_and_default_endings_in_normal_turns() {
    for (text, note) in [
        ("故事。\n[ending: true_end]", "條件還不成立"),
        ("故事。\n[ending: nope]", "不在結局表內"),
        ("故事。\n[ending: default]", "只在導演指示要求收尾時使用"),
    ] {
        let verdict = judge(text, false, 2, false);
        assert_eq!(verdict.ending, None, "{text}");
        assert!(
            verdict.notes[0].contains(note),
            "{text}: {:?}",
            verdict.notes
        );
        assert_eq!(verdict.rewrite(text), "故事。", "{text}");
        assert_eq!(verdict.stop(), None);
    }
}

/// 強制收尾：合法 id 照收；沒寫或不成立就補 default，必定完結、不回饋
#[test]
fn finale_always_ends() {
    let verdict = judge("最後。\n[ending: true_end]", true, 3, true);
    assert_eq!(verdict.ending.as_deref(), Some("true_end"));
    let verdict = judge("最後。\n[ending: true_end]\n[chapter]", false, 3, true);
    assert_eq!(verdict.ending.as_deref(), Some(DEFAULT_ENDING));
    assert!(verdict.notes.is_empty());
    assert_eq!(
        verdict.rewrite("最後。\n[ending: true_end]\n[chapter]"),
        "最後。\n[ending: default]"
    );
    let verdict = judge("最後。\n[ending: default]", false, 3, true);
    assert_eq!(
        verdict.rewrite("最後。\n[ending: default]"),
        "最後。\n[ending: default]"
    );
}

/// `[chapter]`：未到上限留著；第 M 章剝掉轉收尾並要求停下；有接受的結局時剝掉
#[test]
fn chapter_marker_depends_on_the_chapter_limit() {
    let text = "這章結束。\n[chapter]";
    let verdict = judge(text, false, 2, false);
    assert!(!verdict.finale);
    assert_eq!(verdict.rewrite(text), text);
    let verdict = judge(text, false, 3, false);
    assert!(verdict.finale);
    assert_eq!(verdict.stop(), Some(VnStop::Finale));
    assert_eq!(verdict.rewrite(text), "這章結束。");
    let both = "完。\n[chapter]\n[ending: true_end]";
    assert_eq!(
        judge(both, true, 2, false).rewrite(both),
        "完。\n[ending: true_end]"
    );
}

/// 寫回狀態樹：接受結局寫 `ended` 並清收尾；轉收尾寫 `finale`；回饋句接在既有句後
#[test]
fn apply_writes_ended_finale_and_notes() {
    let mut state = crate::data::TableState {
        notes: vec!["舊回饋".to_owned()],
        ..Default::default()
    };
    judge("[ending: nope]", false, 1, false).apply(&mut state);
    assert_eq!(state.notes.len(), 2);
    judge("[chapter]", false, 3, false).apply(&mut state);
    assert!(crate::data::vn::finale_in(&state.tree));
    judge("[ending: true_end]", true, 3, false).apply(&mut state);
    assert_eq!(crate::data::vn::ended_in(&state.tree), Some("true_end"));
    assert!(!crate::data::vn::finale_in(&state.tree));
}

/// 不驗結局的桌：章末照留、結局剝掉
#[test]
fn unjudged_keeps_chapter_and_strips_endings() {
    let text = "a\n[chapter]\n[ending: x]";
    assert_eq!(Verdict::unjudged().rewrite(text), "a\n[chapter]");
}
