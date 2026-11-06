use super::*;

fn roster() -> VnRoster {
    VnRoster {
        characters: vec![
            ("irene".to_owned(), "艾琳".to_owned()),
            ("fox".to_owned(), "狐狸".to_owned()),
        ],
        player: Some("阿誠".to_owned()),
        ..Default::default()
    }
}

fn gm(text: &str) -> Vec<VnCell> {
    parse_event(text, &VnSource::Gm, &roster())
}

fn narration(text: &str) -> VnCell {
    VnCell::Narration {
        text: text.to_owned(),
    }
}

fn line(id: Option<&str>, name: &str, expression: &str, text: &str) -> VnCell {
    VnCell::Dialogue {
        speaker_id: id.map(str::to_owned),
        speaker_name: name.to_owned(),
        expression: expression.to_owned(),
        variant: None,
        thought: false,
        directive: None,
        text: text.to_owned(),
    }
}

fn choices(options: &[&str]) -> VnCell {
    VnCell::Choices {
        options: options.iter().map(|option| (*option).to_owned()).collect(),
    }
}

/// 方案 §2 的寬容規則，每條一組（落檔版）
#[test]
fn parser_follows_each_lenient_rule() {
    let cases: Vec<(&str, &str, Vec<VnCell>)> = vec![
        (
            "旁白空行分格、連續旁白併一格",
            "海風吹過。\n浪聲很近。\n\n天色暗了。",
            vec![narration("海風吹過。\n浪聲很近。"), narration("天色暗了。")],
        ),
        (
            "有表情的對白自成一格，夾在旁白中間也切開",
            "她回頭。\n艾琳 (happy): 「你來了。」\n風停了。",
            vec![
                narration("她回頭。"),
                line(Some("irene"), "艾琳", "happy", "你來了。"),
                narration("風停了。"),
            ],
        ),
        (
            "無表情寫法＝neutral，全形冒號與 “” 也剝",
            "艾琳: 「走吧。」\n狐狸：“好。”",
            vec![
                line(Some("irene"), "艾琳", "neutral", "走吧。"),
                line(Some("fox"), "狐狸", "neutral", "好。"),
            ],
        ),
        (
            "表情清單外退 neutral、全形括號、玩家名對到哨兵",
            "艾琳（得意）: 「哼。」\n阿誠 (sad): 「抱歉。」",
            vec![
                line(Some("irene"), "艾琳", "neutral", "哼。"),
                line(Some(transport::PLAYER_SENTINEL), "阿誠", "sad", "抱歉。"),
            ],
        ),
        (
            "名單外的名字有括號＝無 id 對白；沒括號的冒號句是旁白",
            "店主 (happy): 「歡迎。」\n門上寫著：禁止進入\n她說：「走吧。」",
            vec![
                line(None, "店主", "happy", "歡迎。"),
                narration("門上寫著：禁止進入\n她說：「走吧。」"),
            ],
        ),
        (
            "[bg] 含／不含時段與天氣，關鍵字不分大小寫，認不得的天氣當沒寫",
            "[bg: 港口 | 黃昏 | 海霧]\n[BG: 教室]\n[bg: 碼頭 | 夜晚 | 心情很差]",
            vec![
                VnCell::Bg {
                    location: "港口".to_owned(),
                    time: Some("黃昏".to_owned()),
                    slug: crate::vn_backgrounds::slug_of("港口"),
                    period: Some("dusk".to_owned()),
                    weather: Some("fog".to_owned()),
                },
                VnCell::Bg {
                    location: "教室".to_owned(),
                    time: None,
                    slug: crate::vn_backgrounds::slug_of("教室"),
                    period: None,
                    weather: None,
                },
                VnCell::Bg {
                    location: "碼頭".to_owned(),
                    time: Some("夜晚".to_owned()),
                    slug: crate::vn_backgrounds::slug_of("碼頭"),
                    period: Some("night".to_owned()),
                    weather: None,
                },
            ],
        ),
        (
            "未知 [...] 行併入旁白；整段沒有標記＝一格旁白",
            "[music: 雨聲]\n雨一直下。",
            vec![narration("[music: 雨聲]\n雨一直下。")],
        ),
        (
            "[chapter] 與 [ending: id]：落檔文字已驗過，結局壓過章末",
            "結束了。\n[chapter]\n[Ending: true_end]",
            vec![
                narration("結束了。"),
                VnCell::Ending {
                    id: "true_end".to_owned(),
                },
            ],
        ),
        ("選項少於 2 個整塊丟掉", "[choices]\n- 只有一個", vec![]),
        (
            "選項多於 4 個取前 4、選項間空行照收",
            "[choices]\n- 一\n- 二\n\n- 三\n-四\n- 五",
            vec![choices(&["一", "二", "三", "四"])],
        ),
        (
            "多個 [choices] 取最後一塊；區塊後的文字回到旁白",
            "[choices]\n- 甲\n- 乙\n後來。\n[choices]\n- 丙\n- 丁",
            vec![narration("後來。"), choices(&["丙", "丁"])],
        ),
        (
            "包 5：[ending]＋[choices] 同則時剝選項（落檔的終止標記都已驗過，取得壓制資格）",
            "[ending: bad]\n[choices]\n- 再試一次\n- 放棄",
            vec![VnCell::Ending {
                id: "bad".to_owned(),
            }],
        ),
    ];
    for (label, text, expected) in cases {
        assert_eq!(gm(text), expected, "{label}");
    }
}

/// thought、@變體原樣保留、指令白名單（第一格是指令也收）與每事件上限 2
#[test]
fn thought_variant_and_directive_cap() {
    let cells = gm("艾琳 (sad @戰甲, closeup) thought: 他記得嗎\n\
         狐狸 (surprised, shake): 「咦」\n\
         艾琳 (closeup): 「看我」\n\
         狐狸 (happy, wave): 「嗨」");
    let picked: Vec<_> = cells
        .iter()
        .map(|cell| match cell {
            VnCell::Dialogue {
                expression,
                variant,
                thought,
                directive,
                text,
                ..
            } => (
                expression.as_str(),
                variant.clone(),
                *thought,
                *directive,
                text.as_str(),
            ),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        picked,
        [
            (
                "sad",
                Some("戰甲".to_owned()),
                true,
                Some(Directive::Closeup),
                "他記得嗎"
            ),
            ("surprised", None, false, Some(Directive::Shake), "咦"),
            // 第三個指令超過每事件上限，清掉
            ("neutral", None, false, None, "看我"),
            ("happy", None, false, None, "嗨"),
        ]
    );
}

/// 優先序接口：取得資格的高優先者剝低優先終止格（落檔文字的終止標記都已在提交時驗過）
#[test]
fn terminal_priority_strips_lower_ranks_only_when_accepted() {
    let cells = vec![
        choices(&["甲", "乙"]),
        VnCell::Chapter,
        VnCell::Ending { id: "a".to_owned() },
    ];
    let only_choices = |cell: &VnCell| matches!(cell, VnCell::Choices { .. });
    assert_eq!(apply_terminal_priority(cells.clone(), only_choices), cells);
    assert_eq!(
        apply_terminal_priority(cells.clone(), |_| true),
        vec![VnCell::Ending { id: "a".to_owned() }]
    );
    assert_eq!(stop_reason(&cells, only_choices), Some(VnStop::Choices));
    assert_eq!(stop_reason(&cells[1..], only_choices), Some(VnStop::Ending));
}

/// VN 桌 GM 回合：任一終止標記＝不點名（next None）並標理由；沒有就照原點名
#[test]
fn gm_stop_overrides_next_on_terminal_markers() {
    let next = || Some("fox".to_owned());
    for (text, stop) in [
        ("走吧。\n[choices]\n- 甲\n- 乙", VnStop::Choices),
        ("第一章完。\n[chapter]", VnStop::Chapter),
        ("[ending: true_end]", VnStop::Ending),
    ] {
        assert_eq!(
            gm_stop(text, &roster(), next(), None),
            (None, Some(stop)),
            "{text}"
        );
    }
    assert_eq!(
        gm_stop("狐狸 (happy): 「嗨」", &roster(), next(), None),
        (next(), None)
    );
    // 只有一個選項＝沒有選項，不停
    assert_eq!(
        gm_stop("[choices]\n- 甲", &roster(), next(), None),
        (next(), None)
    );
    // 第 M 章的 `[chapter]` 提交時已剝、轉收尾：沒有其他終止標記也要停
    assert_eq!(
        gm_stop("旅程到了盡頭。", &roster(), next(), Some(VnStop::Finale)),
        (None, Some(VnStop::Finale))
    );
}

/// 角色線：控制標記整行剝（選項連其下選項行）；別人的對白行文字原樣，解析時改旁白
#[test]
fn character_filter_and_parse() {
    let reply = "[bg: 港口]\n狐狸 (happy): 「嗨」\n艾琳 (sad): 「走開」\n[choices]\n- 甲\n- 乙\n\n狐狸笑了。\n[chapter]\n[ending: x]";
    let filtered = filter_character_reply(reply);
    assert_eq!(
        filtered,
        "狐狸 (happy): 「嗨」\n艾琳 (sad): 「走開」\n狐狸笑了。"
    );
    let source = VnSource::Character {
        self_id: "fox".to_owned(),
    };
    assert_eq!(
        parse_event(&filtered, &source, &roster()),
        vec![
            line(Some("fox"), "狐狸", "happy", "嗨"),
            narration("艾琳 (sad): 「走開」\n狐狸笑了。"),
        ]
    );
    // 收尾：不剝「名字:」前綴（那是對白行），控制區塊照剝
    let finished =
        finish_vn_character_reply("狐狸: 「嗨」\n<UpdateVariable>x</UpdateVariable>", false);
    assert_eq!(finished.text, "狐狸: 「嗨」");
    assert!(finished.raw.is_some());
}

/// 串流入口：先照來源清理再解析，不套刪格規則，最後一格暫定
#[test]
fn stream_entry_cleans_by_source_and_keeps_every_cell() {
    let gm_text =
        "[choices]\n- 甲\n[choices]\n- 乙\n- 丙\n```state\ntime: 夜\n```\n下一位：狐狸\n<UpdateVa";
    let parsed = parse_text(gm_text, &VnSource::Gm, &roster());
    assert!(parsed.tail_tentative);
    assert_eq!(parsed.cells, vec![choices(&["甲"]), choices(&["乙", "丙"])]);
    let source = VnSource::Character {
        self_id: "fox".to_owned(),
    };
    let parsed = parse_text("狐狸: 「嗨」\n[bg: 港口]\n<status>", &source, &roster());
    assert_eq!(
        parsed.cells,
        vec![line(Some("fox"), "狐狸", "neutral", "嗨")]
    );
    assert!(!parse_text("", &VnSource::Gm, &roster()).tail_tentative);
}

/// 換一組只換最後一塊選項區塊，前後文字逐字不動
#[test]
fn replace_choices_rewrites_only_the_last_block() {
    let text = "走吧。\n[choices]\n- 甲\n- 乙\n```state\ntime: 夜\n```";
    let options = vec!["丙".to_owned(), "丁".to_owned()];
    assert_eq!(
        replace_choices(text, &options).as_deref(),
        Some("走吧。\n[choices]\n- 丙\n- 丁\n```state\ntime: 夜\n```")
    );
    assert_eq!(replace_choices("沒有選項", &options), None);
}

/// 卡名帶括號註記：模型只寫本名也對得上，但撞名（兩張卡去註記同名）不認
#[test]
fn names_match_without_the_trailing_note_only_when_unique() {
    let roster = VnRoster {
        characters: vec![
            ("leon".to_owned(), "雷恩（花豹）".to_owned()),
            ("a".to_owned(), "米雅 (學姊)".to_owned()),
            ("b".to_owned(), "米雅(學妹)".to_owned()),
        ],
        player: None,
        ..Default::default()
    };
    assert_eq!(roster.id_of("雷恩").as_deref(), Some("leon"));
    assert_eq!(roster.id_of("雷恩（花豹）").as_deref(), Some("leon"));
    assert_eq!(roster.id_of("米雅"), None);
    assert_eq!(roster.id_of("米雅(學妹)").as_deref(), Some("b"));
    let cells = parse_event("雷恩 (happy): 「來比一場」", &VnSource::Gm, &roster);
    assert_eq!(cells, vec![line(Some("leon"), "雷恩", "happy", "來比一場")]);
}

/// 行首「旁白：」類前綴剝掉當旁白
#[test]
fn narrator_prefixes_are_stripped_into_narration() {
    let source = VnSource::Character {
        self_id: "fox".to_owned(),
    };
    let cells = parse_event(
        "旁白：狐狸走近。\nNarrator: He waits.\n\n狐狸: 「嗨」\n\nnarration:風停了。",
        &source,
        &roster(),
    );
    assert_eq!(
        cells,
        vec![
            narration("狐狸走近。\nHe waits."),
            line(Some("fox"), "狐狸", "neutral", "嗨"),
            narration("風停了。"),
        ]
    );
}

/// VN 卡額外表情只認說話者自己宣告的（別人的退 neutral）；卡地點換成卡上的名稱與 slug
#[test]
fn card_expressions_and_places_from_the_card() {
    use crate::data::vn::{CardOrigin, CastOrigin};
    use crate::data::vn_art::{ArtEntry, CardLocation, CastArt};
    let mut card = CardOrigin::default();
    card.cast.insert(
        "irene".to_owned(),
        CastOrigin {
            art: CastArt {
                expressions: vec![ArtEntry {
                    key: "tender".to_owned(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            ..Default::default()
        },
    );
    card.locations.push(CardLocation {
        key: "workshop".to_owned(),
        name: "拉克的工房".to_owned(),
        times: vec!["day".to_owned(), "dusk".to_owned()],
        ..Default::default()
    });
    let roster = VnRoster {
        characters: roster().characters,
        ..VnRoster::new(&[], None, Some(&card))
    };
    let cells = parse_event(
        "[bg: workshop | 黃昏]\n\n艾琳 (tender): 「嗯」\n\n狐狸 (tender): 「哼」",
        &VnSource::Gm,
        &roster,
    );
    let expressions: Vec<&str> = cells
        .iter()
        .filter_map(|cell| match cell {
            VnCell::Dialogue { expression, .. } => Some(expression.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(expressions, ["tender", "neutral"]);
    assert!(
        matches!(&cells[0], VnCell::Bg { location, period, slug, .. }
        if location == "拉克的工房"
            && period.as_deref() == Some("dusk")
            && *slug != crate::vn_backgrounds::slug_of("workshop"))
    );
}

/// 已落檔的漏剝原文（sunsmell 重播包第 2 章首拍尾段）：讀取時點名行不成格；選項區塊後的也擋
#[test]
fn leaked_relay_lines_never_become_cells() {
    let stored = "她的鼻尖輕輕動了兩下。\n\n\n\n\n\n下一位：老婆婆";
    assert_eq!(gm(stored), vec![narration("她的鼻尖輕輕動了兩下。")]);
    assert_eq!(
        gm("門開了。\n[choices]\n- 進去\n- 離開\n Next: 狐狸 "),
        vec![narration("門開了。"), choices(&["進去", "離開"])]
    );
}
