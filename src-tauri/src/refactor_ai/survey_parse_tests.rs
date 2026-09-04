use super::*;

#[test]
fn recommend_parses_two_lines_and_rejects_garbage() {
    let ok = parse_recommend("RECOMMEND: interface\nEVIDENCE: 這張卡有完整遊戲介面。").unwrap();
    assert_eq!(ok.recommend, "interface");
    assert_eq!(ok.evidence, "這張卡有完整遊戲介面。");
    // 大小寫與前置雜訊容忍；characters 值
    let loose = parse_recommend("recommend: Characters 多角色\nevidence: 卡內有 8 位帶完整設定的人物").unwrap();
    assert_eq!(loose.recommend, "characters");
    // 缺 RECOMMEND 或值不合法＝None（前端走預設介面優先，不偽造證據）
    assert!(parse_recommend("EVIDENCE: 只有證據沒有建議").is_none());
    assert!(parse_recommend("RECOMMEND: both\nEVIDENCE: 亂答").is_none());
    // EVIDENCE 缺席仍成立（證據空字串，前端不顯判官句）
    assert_eq!(parse_recommend("RECOMMEND: interface").unwrap().evidence, "");
}

/// MODE 回聲解析：合法值收、亂值留空（呼叫端核對不過整份拒收）。
#[test]
fn parse_survey_reads_mode_echo() {
    let echoed = parse_survey("## MODE: interface\n\n## PERSONS\n\n## ENTRIES\n- uid=3 action: carry\n");
    assert_eq!(echoed.mode, "interface");
    assert_eq!(parse_survey("## MODE: Characters\n\n## PERSONS\n").mode, "characters");
    // 值誤寫成獨立一行：characters 不撞標記、容錯收下；interface 會被吃成 INTERFACE 標記行，
    // 收不到＝呼叫端拒收重跑（提示詞已要求單行逐字照寫）
    assert_eq!(parse_survey("## MODE\ncharacters\n\n## PERSONS\n").mode, "characters");
    assert_eq!(parse_survey("## MODE: both\n\n## PERSONS\n").mode, "");
    assert_eq!(parse_survey("## PERSONS\n").mode, "");
}

#[test]
fn parse_survey_extracts_all_six_blocks() {
    let raw = "## PERSONS\n\
               - name: 亞瑟 uids: 101 player: yes\n\
               - name: 霍玄 uids: 12,45 mode: clean spans: 12#s1,45#s2 private: 45#s3\n\
               \n\
               ## INTERFACE\n\
               - uid=201 playable: no\n\
               - uid=202 playable: yes\n\
               \n\
               ## ENTRIES\n\
               - uid=3 action: carry\n\
               - uid=4 action: carry reason: 歷史年表非機制\n\
               - uid=9 action: absorb\n\
               - uid=5 action: drop rule: 2\n\
               - uid=7 action: split\n\
               \n\
               ## SPLITS\n\
               - span: 7#s1 route: statusbar\n\
               - span: 7#s2 route: gm\n\
               - span: 7#s3 route: drop rule: 1\n\
               - span: 23#s2 route: person name: 霍玄\n\
               - span: 23#s4 route: entry title: 王府概況\n\
               - span: 16#s2 route: group id: g1\n\
               - span: 16#s6 route: unabsorbed note: 擲骰檢定\n\
               \n\
               ## GROUPS\n\
               - id: g1 title: 格式與行為 kind: mechanism spans: 16#s2,16#s5,18#s1\n\
               \n\
               ## FIELDS\n\
               - 好感度\n\
               - 淪陷天數\n";
    let outcome = parse_survey(raw);

    assert_eq!(outcome.persons.len(), 2);
    assert_eq!(outcome.persons[0].name, "亞瑟");
    assert!(outcome.persons[0].is_player);
    assert_eq!(outcome.persons[0].mode, "");
    assert!(outcome.persons[0].spans.is_empty());
    assert_eq!(outcome.persons[1].name, "霍玄");
    assert_eq!(outcome.persons[1].uids, vec!["12", "45"]);
    assert_eq!(outcome.persons[1].mode, "clean");
    assert_eq!(outcome.persons[1].spans, vec!["12#s1", "45#s2"]);
    assert_eq!(outcome.persons[1].private_spans, vec!["45#s3"]);

    assert_eq!(outcome.interface_uids, vec!["201", "202"]);
    assert_eq!(outcome.playable_interface_uids, vec!["202"]);

    assert_eq!(outcome.verdicts.len(), 5);
    assert_eq!(outcome.verdicts[0].action, "carry");
    assert_eq!(outcome.verdicts[0].reason, "");
    assert_eq!(outcome.verdicts[1].action, "carry");
    assert_eq!(outcome.verdicts[1].reason, "歷史年表非機制");
    assert_eq!(outcome.verdicts[2].action, "absorb");
    assert_eq!(outcome.verdicts[3].action, "drop");
    assert_eq!(outcome.verdicts[3].rule, Some(2));
    assert_eq!(outcome.verdicts[4].action, "split");

    assert_eq!(outcome.splits.len(), 7);
    assert_eq!(outcome.splits[0].route, "statusbar");
    assert_eq!(outcome.splits[1].route, "gm");
    assert_eq!(outcome.splits[2].route, "drop");
    assert_eq!(outcome.splits[2].rule, Some(1));
    assert_eq!(outcome.splits[3].route, "person");
    assert_eq!(outcome.splits[3].name, "霍玄");
    assert_eq!(outcome.splits[4].route, "entry");
    assert_eq!(outcome.splits[4].title, "王府概況");
    assert_eq!(outcome.splits[5].route, "group");
    assert_eq!(outcome.splits[5].group, "g1");
    assert_eq!(outcome.splits[6].route, "unabsorbed");
    assert_eq!(outcome.splits[6].note, "擲骰檢定");

    assert_eq!(outcome.groups.len(), 1);
    assert_eq!(outcome.groups[0].id, "g1");
    assert_eq!(outcome.groups[0].title, "格式與行為");
    assert_eq!(outcome.groups[0].kind, "mechanism");
    assert_eq!(outcome.groups[0].spans, vec!["16#s2", "16#s5", "18#s1"]);

    assert_eq!(outcome.fields, vec!["好感度", "淪陷天數"]);
    assert_eq!(outcome.raw, raw);
}

/// MODE 正規化：interface 模式判官違規吐 PERSONS＝整區清掉（內容由涵蓋稽核照搬），
/// characters 模式人物照留。
#[test]
fn normalize_survey_clears_persons_only_in_interface_mode() {
    let mut violating = parse_survey(
        "## MODE: interface\n\n## PERSONS\n- name: 亞瑟 uids: 101\n\n## ENTRIES\n- uid=9 action: carry\n",
    );
    assert_eq!(violating.persons.len(), 1);
    normalize_survey_for_mode(&mut violating);
    assert!(violating.persons.is_empty());
    assert_eq!(violating.verdicts.len(), 1);

    let mut kept = parse_survey("## MODE: characters\n\n## PERSONS\n- name: 亞瑟 uids: 101\n");
    normalize_survey_for_mode(&mut kept);
    assert_eq!(kept.persons.len(), 1);
}

#[test]
fn parse_survey_persons_old_format_line_parses_without_new_fields() {
    let raw = "## PERSONS\n- name: 亞瑟 uids: 101 player: yes\n";
    let outcome = parse_survey(raw);
    let person = &outcome.persons[0];
    assert_eq!(person.name, "亞瑟");
    assert_eq!(person.uids, vec!["101"]);
    assert!(person.is_player);
    assert_eq!(person.mode, "");
    assert!(person.spans.is_empty());
    assert!(person.private_spans.is_empty());
}

#[test]
fn parse_survey_interface_without_playable_flag_defaults_to_no() {
    let raw = "## INTERFACE\n- uid=201\n";
    let outcome = parse_survey(raw);
    assert_eq!(outcome.interface_uids, vec!["201"]);
    assert!(outcome.playable_interface_uids.is_empty());
}

#[test]
fn parse_survey_includes_single_source_person() {
    let raw = "## PERSONS\n- name: 酒館老闆 uids: 55\n\n## INTERFACE\n";
    let outcome = parse_survey(raw);
    assert_eq!(outcome.persons.len(), 1);
    assert_eq!(outcome.persons[0].uids, vec!["55"]);
}

#[test]
fn parse_survey_ignores_chitchat_before_and_after_markers() {
    let raw = "好的，以下是我的盤點結果：\n\n\
               ## PERSONS\n\
               - name: 小明 uids: 1\n\n\
               ## ENTRIES\n\
               - uid=9 action: carry\n\n\
               以上就是全部分類，如有需要再讓我知道！";
    let outcome = parse_survey(raw);
    assert_eq!(outcome.persons.len(), 1);
    assert_eq!(outcome.verdicts.len(), 1);
}

#[test]
fn parse_survey_skips_malformed_person_lines() {
    let raw = "## PERSONS\n- 這行沒有照格式寫\n- name: 缺 uids 的人\n- name: 好人 uids: 7\n";
    let outcome = parse_survey(raw);
    assert_eq!(outcome.persons.len(), 1);
    assert_eq!(outcome.persons[0].name, "好人");
}

#[test]
fn parse_survey_skips_malformed_entries_lines() {
    let raw = "## ENTRIES\n\
               - uid=1 action: ghost\n\
               - uid=2 這行沒有 action 欄\n\
               - uid=3 action: carry\n";
    let outcome = parse_survey(raw);
    assert_eq!(outcome.verdicts.len(), 1);
    assert_eq!(outcome.verdicts[0].uid, "3");
}

#[test]
fn parse_survey_skips_malformed_splits_lines() {
    let raw = "## SPLITS\n\
               - span: abc route: statusbar\n\
               - span: 7#s1 route: ghost\n\
               - span: 7#s2 route: person\n\
               - span: 7#s3 route: gm\n";
    let outcome = parse_survey(raw);
    assert_eq!(outcome.splits.len(), 1);
    assert_eq!(outcome.splits[0].span, "7#s3");
}

#[test]
fn parse_survey_skips_malformed_groups_lines() {
    let raw = "## GROUPS\n\
               - id: g1 title: 缺欄位 kind: setting\n\
               - id: g2 title: 壞種類 kind: ghost spans: 1#s1\n\
               - id: g3 title: 好組 kind: mechanism spans: 1#s1,2#s2\n";
    let outcome = parse_survey(raw);
    assert_eq!(outcome.groups.len(), 1);
    assert_eq!(outcome.groups[0].id, "g3");
}
