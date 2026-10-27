use super::*;
use crate::import::import_character;
use crate::import::test_support::TestRoot;
use serde_json::json;

#[test]
fn read_card_interfaces_filters_to_output_display_scripts() {
    let root = TestRoot::new("interfaces-filter");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let raw = json!({
        "data": {
            "name": "莉亞",
            "extensions": {
                "regex_scripts": [
                    {
                        "scriptName": "顯示介面",
                        "findRegex": "/.+/s",
                        "replaceString": "<div>ok</div>",
                        "trimStrings": ["a"],
                        "minDepth": 1,
                        "maxDepth": 2,
                        "placement": [2]
                    },
                    {
                        "scriptName": "已停用",
                        "findRegex": "/x/",
                        "replaceString": "y",
                        "disabled": true,
                        "placement": [2]
                    },
                    {
                        "scriptName": "只套提示詞",
                        "findRegex": "/x/",
                        "replaceString": "y",
                        "promptOnly": true,
                        "placement": [2]
                    },
                    {
                        "scriptName": "作用在使用者輸入",
                        "findRegex": "/x/",
                        "replaceString": "y",
                        "placement": [1]
                    }
                ]
            }
        }
    })
    .to_string();

    let meta =
        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
    let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
    assert_eq!(interfaces.len(), 1);
    assert_eq!(interfaces[0].character_id, meta.id);
    assert_eq!(interfaces[0].unsupported, None);
    assert_eq!(interfaces[0].scripts.len(), 1);
    let script = &interfaces[0].scripts[0];
    assert_eq!(script.name, "顯示介面");
    assert_eq!(script.find_regex, "/.+/s");
    assert_eq!(script.replace_string, "<div>ok</div>");
    assert_eq!(script.trim_strings, vec!["a".to_owned()]);
    assert_eq!(script.min_depth, Some(1));
    assert_eq!(script.max_depth, Some(2));
}

#[test]
fn read_card_interfaces_detects_scrypt_cards() {
    let root = TestRoot::new("interfaces-scrypt");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let raw = json!({
        "data": {
            "name": "莉亞",
            "first_mes": "<!--SCRYPT PROTECTED-->"
        }
    })
    .to_string();

    import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
    let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
    assert_eq!(interfaces.len(), 1);
    assert_eq!(interfaces[0].unsupported, Some("scrypt".to_owned()));
    assert!(interfaces[0].scripts.is_empty());
}

#[test]
fn read_card_interfaces_detects_remote_loader_cards() {
    let root = TestRoot::new("interfaces-remote-loader");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let raw = json!({
        "data": {
            "name": "莉亞",
            "extensions": {
                "regex_scripts": [{
                    "scriptName": "雲端介面",
                    "findRegex": "/.+/s",
                    "replaceString": "```\n<body>\n<script>\n$('body').load('https://example.github.io/x/index.html')\n</script>\n</body>\n```",
                    "placement": [2]
                }]
            }
        }
    })
    .to_string();

    import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
    let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
    assert_eq!(interfaces.len(), 1);
    assert_eq!(interfaces[0].unsupported, Some("remote_loader".to_owned()));
    assert!(interfaces[0].scripts.is_empty());
}

#[test]
fn read_card_interfaces_handles_plain_cards_without_extensions() {
    let root = TestRoot::new("interfaces-plain");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let raw = json!({"data": {"name": "莉亞"}}).to_string();

    import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
    let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
    assert_eq!(interfaces.len(), 1);
    assert!(interfaces[0].scripts.is_empty());
    assert_eq!(interfaces[0].unsupported, None);
    assert!(!interfaces[0].mvu);
}

/// 載入 MVU 的判斷：新版 tavern_helper 與舊版 TavernHelper_scripts 都認，停用的腳本不算。
#[test]
fn card_interface_detects_mvu_script() {
    let script = |enabled: bool| {
        json!({"name": "MVU基础脚本", "enabled": enabled, "type": "script",
            "content": "import'https://testingcf.jsdelivr.net/gh/MagicalAstrogy/MagVarUpdate/artifact/bundle.js';"})
    };
    let card = |extensions: Value| json!({"name": "卡", "extensions": extensions});
    let mvu = |extensions: Value| card_interface("c", "卡", &card(extensions)).mvu;
    assert!(mvu(
        json!({"tavern_helper": {"scripts": [script(true)], "variables": {}}})
    ));
    assert!(mvu(
        json!({"TavernHelper_scripts": [{"type": "script", "value": script(true)}]})
    ));
    assert!(!mvu(json!({"tavern_helper": {"scripts": [script(false)]}})));
    assert!(!mvu(
        json!({"tavern_helper": {"scripts": [{"name": "別的", "content": "console.log(1)"}]}})
    ));
    assert!(!mvu(json!({"regex_scripts": []})));
}

fn scripted_card(name: &str) -> String {
    json!({"data": {"name": name, "extensions": {"regex_scripts": [{
        "scriptName": "顯示介面", "findRegex": "/.+/s",
        "replaceString": "<div>ok</div>", "placement": [2]
    }]}}})
    .to_string()
}

fn raw_file(root: &Path, world_id: &str, id: &str, extension: &str) -> std::path::PathBuf {
    data::character_path(root, world_id, id)
        .unwrap()
        .with_extension(extension)
}

fn assert_unreadable(card: &CardInterface, id: &str, name: &str) {
    assert_eq!(card.character_id, id);
    assert_eq!(card.character_name, name);
    assert_eq!(card.unsupported.as_deref(), Some(UNREADABLE));
    assert!(card.scripts.is_empty());
    assert!(!card.mvu);
    assert_eq!(card.opening, None);
}

/// 原始卡檔壞掉（非法 JSON）：不讓整份清單報錯，回一筆 unreadable，重構才擋得下。
#[test]
fn read_card_interfaces_reports_corrupted_raw_file_as_unreadable() {
    let root = TestRoot::new("interfaces-corrupt");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let raw = json!({"data": {"name": "莉亞"}}).to_string();
    let meta =
        import_character(root.path(), &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
    fs::write(
        raw_file(root.path(), &world_id, &meta.id, "import.json"),
        b"\xff\xfe not json",
    )
    .unwrap();

    let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
    assert_eq!(interfaces.len(), 1);
    assert_unreadable(&interfaces[0], &meta.id, "莉亞");
}

/// JSON 匯入後換了角色圖：顯示圖從不當原卡，照讀 .import.json。
#[test]
fn read_card_interfaces_falls_back_to_import_json_behind_plain_png() {
    let root = TestRoot::new("interfaces-plain-png-json");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let meta = import_character(
        root.path(),
        &world_id,
        scripted_card("莉亞").as_bytes(),
        "#3366ff",
        "zh-TW",
    )
    .unwrap();
    crate::import::save_character_image(
        root.path(),
        &world_id,
        &meta.id,
        &crate::import::png_image::test_png::real_png(4, 3),
    )
    .unwrap();

    let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
    assert_eq!(interfaces.len(), 1);
    assert_eq!(interfaces[0].unsupported, None);
    assert_eq!(interfaces[0].scripts.len(), 1);
}

/// PNG 匯入後換角色圖（換成另一張 ST 卡也一樣）、再刪圖：原卡 `.card.png` 不動，介面照舊是原卡的，
/// 存下的顯示圖不帶那張卡的資料，介面不多一份。
#[test]
fn character_image_changes_never_touch_the_original_card() {
    let root = TestRoot::new("interfaces-image-swap");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let png = crate::import::test_support::card_png(&scripted_card("莉亞"));
    let meta = import_character(root.path(), &world_id, &png, "#3366ff", "zh-TW").unwrap();
    let card_file = raw_file(root.path(), &world_id, &meta.id, "card.png");
    assert_eq!(fs::read(&card_file).unwrap(), png);
    let other = json!({"data": {"name": "艾倫", "extensions": {"regex_scripts": [
        {"scriptName": "甲", "findRegex": "/a/", "replaceString": "<b>甲</b>", "placement": [2]},
        {"scriptName": "乙", "findRegex": "/b/", "replaceString": "<b>乙</b>", "placement": [2]}
    ]}}})
    .to_string();
    crate::import::save_character_image(
        root.path(),
        &world_id,
        &meta.id,
        &crate::import::test_support::card_png(&other),
    )
    .unwrap();
    let display = fs::read(raw_file(root.path(), &world_id, &meta.id, "png")).unwrap();
    assert_eq!(find_png_card(&display).unwrap(), None);
    let check = || {
        let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].character_name, "莉亞");
        assert_eq!(interfaces[0].unsupported, None);
        assert_eq!(interfaces[0].scripts.len(), 1);
        assert_eq!(interfaces[0].scripts[0].replace_string, "<div>ok</div>");
        assert_eq!(fs::read(&card_file).unwrap(), png);
    };
    check();
    crate::import::delete_character_image(root.path(), &world_id, &meta.id).unwrap();
    check();
}

/// `.card.png` 只在匯入時寫、必帶卡片資料：讀出是一般圖（結構完好、沒卡片資料）也算損毀。
#[test]
fn read_card_interfaces_reports_plain_card_png_as_unreadable() {
    let root = TestRoot::new("interfaces-plain-card-png");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let png = crate::import::test_support::card_png(&scripted_card("莉亞"));
    let meta = import_character(root.path(), &world_id, &png, "#3366ff", "zh-TW").unwrap();
    fs::write(
        raw_file(root.path(), &world_id, &meta.id, "card.png"),
        crate::import::png_image::test_png::real_png(4, 3),
    )
    .unwrap();

    let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
    assert_eq!(interfaces.len(), 1);
    assert_unreadable(&interfaces[0], &meta.id, "莉亞");
}

/// `.card.png` 結構壞掉（截斷）＝損毀，回 unreadable。
#[test]
fn read_card_interfaces_reports_truncated_png_as_unreadable() {
    let root = TestRoot::new("interfaces-truncated-png");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let png = crate::import::test_support::card_png(&scripted_card("莉亞"));
    let meta = import_character(root.path(), &world_id, &png, "#3366ff", "zh-TW").unwrap();
    let mut truncated = PNG_MAGIC.to_vec();
    truncated.extend_from_slice(&[0, 0, 0, 9]);
    fs::write(
        raw_file(root.path(), &world_id, &meta.id, "card.png"),
        truncated,
    )
    .unwrap();

    let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
    assert_eq!(interfaces.len(), 1);
    assert_unreadable(&interfaces[0], &meta.id, "莉亞");
}

/// 兩份原卡檔的組合：一般圖 .card.png＋壞 JSON＝unreadable；有效 .card.png＋壞 JSON＝用 .card.png；
/// 壞 .card.png＋好 JSON＝用 JSON。
#[test]
fn read_card_interfaces_combines_png_and_import_json() {
    let root = TestRoot::new("interfaces-combos");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let plain = crate::import::png_image::test_png::real_png(4, 3);
    let mut truncated = PNG_MAGIC.to_vec();
    truncated.extend_from_slice(&[0, 0, 0, 9]);
    let import = |raw: &[u8]| {
        import_character(root.path(), &world_id, raw, "#3366ff", "zh-TW")
            .unwrap()
            .id
    };

    let plain_bad = import(scripted_card("甲").as_bytes());
    fs::write(
        raw_file(root.path(), &world_id, &plain_bad, "card.png"),
        &plain,
    )
    .unwrap();
    fs::write(
        raw_file(root.path(), &world_id, &plain_bad, "import.json"),
        b"not json",
    )
    .unwrap();

    let card_bad = import(&crate::import::test_support::card_png(&scripted_card("乙")));
    fs::write(
        raw_file(root.path(), &world_id, &card_bad, "import.json"),
        b"not json",
    )
    .unwrap();

    let bad_good = import(scripted_card("丙").as_bytes());
    fs::write(
        raw_file(root.path(), &world_id, &bad_good, "card.png"),
        &truncated,
    )
    .unwrap();

    let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
    let find = |id: &str| {
        interfaces
            .iter()
            .find(|card| card.character_id == id)
            .unwrap()
    };
    assert_unreadable(find(&plain_bad), &plain_bad, "甲");
    for id in [&card_bad, &bad_good] {
        assert_eq!(find(id).unsupported, None);
        assert_eq!(find(id).scripts.len(), 1);
    }
}

/// 讀檔本身失敗（路徑是資料夾）也算 unreadable；同桌正常的卡照樣列出。
#[test]
fn read_card_interfaces_reports_read_failure_beside_healthy_card() {
    let root = TestRoot::new("interfaces-read-failure");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let broken = import_character(
        root.path(),
        &world_id,
        json!({"data": {"name": "莉亞"}}).to_string().as_bytes(),
        "#3366ff",
        "zh-TW",
    )
    .unwrap();
    let healthy = import_character(
        root.path(),
        &world_id,
        scripted_card("艾倫").as_bytes(),
        "#3366ff",
        "zh-TW",
    )
    .unwrap();
    let path = raw_file(root.path(), &world_id, &broken.id, "import.json");
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();

    let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
    assert_eq!(interfaces.len(), 2);
    let broken_card = interfaces
        .iter()
        .find(|card| card.character_id == broken.id)
        .unwrap();
    assert_unreadable(broken_card, &broken.id, "莉亞");
    let healthy_card = interfaces
        .iter()
        .find(|card| card.character_id == healthy.id)
        .unwrap();
    assert_eq!(healthy_card.unsupported, None);
    assert_eq!(healthy_card.scripts.len(), 1);
}

/// 世界書路徑：帶顯示腳本的卡存下來後，讀出的介面與角色卡路徑篩選結果一致。
#[test]
fn save_world_card_persists_display_scripts_for_worldbook_path() {
    let root = TestRoot::new("world-card-scripts");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let raw = json!({
        "data": {
            "name": "莉亞",
            "extensions": {
                "regex_scripts": [{
                    "scriptName": "顯示介面",
                    "findRegex": "/.+/s",
                    "replaceString": "<div>ok</div>",
                    "placement": [2]
                }]
            }
        }
    })
    .to_string();

    assert!(save_world_card(root.path(), &world_id, raw.as_bytes()));

    let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
    assert_eq!(interfaces.len(), 1);
    assert_eq!(interfaces[0].character_id, "");
    assert_eq!(interfaces[0].character_name, "莉亞");
    assert_eq!(interfaces[0].unsupported, None);
    assert_eq!(interfaces[0].scripts.len(), 1);
    assert_eq!(interfaces[0].scripts[0].name, "顯示介面");
}

/// 純世界書 JSON（沒有 regex_scripts）不必白存一份原始卡檔。
#[test]
fn save_world_card_skips_plain_worldbook_json() {
    let root = TestRoot::new("world-card-plain");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let raw = r#"{"entries":{"0":{"uid":0,"key":["龍"],"content":"沉睡"}}}"#;

    assert!(!save_world_card(root.path(), &world_id, raw.as_bytes()));

    let png_path = data::world_card_path(root.path(), &world_id, "png").unwrap();
    let json_path = data::world_card_path(root.path(), &world_id, "import.json").unwrap();
    assert!(!png_path.exists());
    assert!(!json_path.exists());
}

/// 卡片自帶開場白：有值就填入、空字串當沒有。
#[test]
fn card_interface_fills_opening_from_first_mes() {
    let with_opening = card_interface(
        "id-1",
        "莉亞",
        &json!({"name": "莉亞", "first_mes": "妳來了。"}),
    );
    assert_eq!(with_opening.opening, Some("妳來了。".to_owned()));

    let without_opening = card_interface("id-2", "莉亞", &json!({"name": "莉亞", "first_mes": ""}));
    assert_eq!(without_opening.opening, None);
}

/// 世界層級的原始卡檔壞掉：一樣回一筆 unreadable（character_id 與名字都空）。
#[test]
fn read_card_interfaces_reports_corrupted_world_level_card_as_unreadable() {
    let root = TestRoot::new("world-card-corrupt");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let path = data::world_card_path(root.path(), &world_id, "import.json").unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"\xff\xfe not json").unwrap();

    let interfaces = read_card_interfaces(root.path(), &world_id).unwrap();
    assert_eq!(interfaces.len(), 1);
    assert_unreadable(&interfaces[0], "", "");
}

fn interface_script_for_test(find_regex: &str) -> InterfaceScript {
    InterfaceScript {
        name: "顯示".to_owned(),
        find_regex: find_regex.to_owned(),
        replace_string: String::new(),
        trim_strings: Vec::new(),
        min_depth: None,
        max_depth: None,
        opening_only: false,
    }
}

/// 走正式的提示選取，測試不另寫一份近似判定。
fn named(scripts: &[InterfaceScript], worldbook: &[data::WorldbookEntry]) -> Option<String> {
    let scan = crate::transport::test_support::legacy::scan_of(
        worldbook,
        crate::world_scan::Viewer::Gm,
        &[],
        None,
        &[],
        "zh-TW",
    );
    card_format_entry(
        scripts,
        &crate::transport::gm_prompt_full_entries(&scan, &[]),
    )
}

fn worldbook_entry_for_test(
    uid: u64,
    title: &str,
    content: &str,
    disabled: bool,
) -> data::WorldbookEntry {
    data::WorldbookEntry {
        uid,
        title: title.to_owned(),
        keys: Vec::new(),
        content: content.to_owned(),
        constant: true,
        order: 0,
        disabled,
        visibility: data::Visibility::Gm,
        is_person: false,
        locked: false,
    }
}

/// 西幻卡的真實 find_regex：世界書裡含同款標籤的條目才是模型該照的格式規定；
/// 停用的條目就算含標籤也不能選中；命中標籤數較多的條目優先。
#[test]
fn card_format_entry_picks_matching_worldbook_entry() {
    let find_regex = r"/<GoldenRPG_UI>.*?<CurrentView>([\s\S]*?)<\/CurrentView>.*?<WorldSystem>([\s\S]*?)<\/WorldSystem>.*?<LocalSystem>([\s\S]*?)<\/LocalSystem>.*?<GuildBoard>([\s\S]*?)<\/GuildBoard>.*?<CharSheet>([\s\S]*?)<\/CharSheet>.*?<\/GoldenRPG_UI>/s";
    let scripts = vec![interface_script_for_test(find_regex)];
    let worldbook = vec![
        worldbook_entry_for_test(
            1,
            "回复规则",
            "你的每次回复【必须且只能】输出一个标准的XML格式数据块：<GoldenRPG_UI><CurrentView>…</CurrentView></GoldenRPG_UI>",
            false,
        ),
        worldbook_entry_for_test(2, "世界觀", "這是一個劍與魔法的大陸。", false),
        worldbook_entry_for_test(
            3,
            "停用格式條目",
            "<GoldenRPG_UI><CurrentView>備用格式</CurrentView></GoldenRPG_UI>",
            true,
        ),
    ];

    assert_eq!(named(&scripts, &worldbook), Some("回复规则".to_owned()));
}

/// 世界書完全沒提到卡片格式要的標籤，就沒有可點名的條目。
#[test]
fn card_format_entry_none_when_no_worldbook_entry_matches() {
    let find_regex = r"/<GoldenRPG_UI>.*?<\/GoldenRPG_UI>/s";
    let scripts = vec![interface_script_for_test(find_regex)];
    let worldbook = vec![worldbook_entry_for_test(
        1,
        "世界觀",
        "這是一個劍與魔法的大陸。",
        false,
    )];

    assert_eq!(named(&scripts, &worldbook), None);
}

// ---- gm-no-state-fence-worldbook-table：開場白美化腳本不算卡片自帶回覆格式 ----

fn display_script(find_regex: &str, replace: &str) -> serde_json::Value {
    json!({
        "scriptName": "腳本",
        "findRegex": find_regex,
        "replaceString": replace,
        "placement": [2],
        "markdownOnly": true
    })
}

fn opening_flags(
    first_mes: &str,
    alternates: &[&str],
    scripts: Vec<serde_json::Value>,
) -> Vec<bool> {
    let card = json!({
        "name": "西幻",
        "first_mes": first_mes,
        "alternate_greetings": alternates,
        "extensions": { "regex_scripts": scripts }
    });
    card_interface("", "西幻", &card)
        .scripts
        .iter()
        .map(|script| script.opening_only)
        .collect()
}

#[test]
fn opening_decoration_recognizes_literal_greeting_line() {
    let menu = "⟡ 命運之卷 ⟡\n選擇你的身份";
    // WestFantasy_NOUI 的形狀：首尾 \s* 包住開場白整行字面
    assert_eq!(
        opening_flags(
            "请选择你的身份",
            &[],
            vec![display_script(r"\s*请选择你的身份\s*", menu)]
        ),
        vec![true]
    );
    // 包 /…/flags、^$、\s+、跳脫標點；字面在 alternate_greetings 的某一行
    assert_eq!(
        opening_flags(
            "嗨。",
            &["第一行\n  開始冒險.  \n第三行"],
            vec![display_script(r"/^\s+開始冒險\.\s*$/gm", menu)]
        ),
        vec![true]
    );
    // i 旗標：大小寫不分
    assert_eq!(
        opening_flags(
            "Choose Your Role",
            &[],
            vec![display_script("/choose your role/i", menu)]
        ),
        vec![true]
    );
}

#[test]
fn opening_decoration_keeps_original_judgement_for_unclear_shapes() {
    let menu = "選單";
    let cases = [
        // 只是開場白裡的一個詞（代名詞替換）：每回合回覆也會命中
        ("她站在門口。", display_script("/她/gi", "")),
        // 萬用式、帶群組、\d、未知旗標、空字面
        ("請選擇", display_script("/.+/s", menu)),
        ("請選擇", display_script(r"/請(選擇)/", menu)),
        ("第1章", display_script(r"第\d章", menu)),
        ("請選擇", display_script("/請選擇/x", menu)),
        ("請選擇", display_script(r"/\s*/", menu)),
        // 字面不在任何開場白
        ("你好", display_script("請選擇", menu)),
        // 格式標籤／佔位符：同一字面可能也是每回合格式（開場白含每輪 placeholder 的反例）
        ("<StatusBlock>", display_script("<StatusBlock>", menu)),
        (r"{{開局}}", display_script(r"\{\{開局\}\}", menu)),
        ("【修仙開局】", display_script("【修仙開局】", menu)),
        // 換成前端：含 script，或長 HTML
        ("請選擇", display_script("請選擇", "<script>run()</script>")),
        (
            "請選擇",
            display_script("請選擇", &format!("<div>{}</div>", "格".repeat(1000))),
        ),
    ];
    for (first_mes, script) in cases {
        let find = script["findRegex"].as_str().unwrap().to_owned();
        assert_eq!(
            opening_flags(first_mes, &[], vec![script]),
            vec![false],
            "{find}"
        );
    }
    // 短 HTML 美化仍算開場白美化
    assert_eq!(
        opening_flags("請選擇", &[], vec![display_script("請選擇", "<b>選單</b>")]),
        vec![true]
    );
}

/// 混合卡（開場白美化＋回覆格式腳本）只留回覆格式那支；只有開場白美化的卡一支都不留；
/// 這桌關了 regex（網頁存檔匯入沒允許）也一支都不留。
#[test]
fn reply_format_scripts_drop_opening_decorations() {
    let root = TestRoot::new("reply-format-scripts");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let card = |name: &str, scripts: Vec<serde_json::Value>| {
        json!({ "data": {
            "name": name,
            "first_mes": "请选择你的身份",
            "extensions": { "regex_scripts": scripts }
        }})
        .to_string()
    };
    let opening = display_script(r"\s*请选择你的身份\s*", "選單");
    import_character(
        root.path(),
        &world_id,
        card("無介面", vec![opening.clone()]).as_bytes(),
        "#3366ff",
        "zh-TW",
    )
    .unwrap();
    assert!(reply_format_scripts(root.path(), &world_id).is_empty());
    // 前端照樣拿到開場白腳本
    assert_eq!(
        read_card_interfaces(root.path(), &world_id).unwrap()[0]
            .scripts
            .len(),
        1
    );

    let reply = display_script(
        r"/<StatusBlock>([\s\S]*?)<\/StatusBlock>/s",
        "<div>$1</div>",
    );
    import_character(
        root.path(),
        &world_id,
        card("有介面", vec![opening, reply]).as_bytes(),
        "#3366ff",
        "zh-TW",
    )
    .unwrap();
    let scripts = reply_format_scripts(root.path(), &world_id);
    assert_eq!(scripts.len(), 1);
    assert!(scripts[0].find_regex.contains("StatusBlock"));

    let mut state = data::read_state(root.path(), &world_id).unwrap();
    state.regex_allowed = false;
    data::write_state(root.path(), &world_id, &state).unwrap();
    assert!(reply_format_scripts(root.path(), &world_id).is_empty());
}

/// 回合指示（chat_assembly）與換幕結算（data::requires_present_report）同一判定：
/// 只有開場白美化的卡走一般旁白＋```state，混了回覆格式腳本的卡兩邊都讓路給卡片格式。
#[test]
fn gm_instruction_and_present_report_agree_on_opening_decoration() {
    let root = TestRoot::new("opening-agree");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let opening = display_script(r"\s*请选择你的身份\s*", "選單");
    let raw = |scripts: Vec<serde_json::Value>| {
        json!({ "data": {
            "name": "艾琳",
            "first_mes": "请选择你的身份",
            "extensions": { "regex_scripts": scripts }
        }})
        .to_string()
    };
    let check = |expect_narration: bool| {
        let materials = crate::chat_assembly::gm_materials(root.path(), &world_id).unwrap();
        let scan = crate::chat_assembly::test_gm_scan(root.path(), &world_id, &materials, "zh-TW");
        let (instruction, _) = crate::chat_assembly::gm_instruction(
            root.path(),
            &world_id,
            &materials,
            &scan,
            "zh-TW",
        );
        assert_eq!(instruction.content.contains("```state"), expect_narration);
        assert_eq!(
            data::requires_present_report(root.path(), &world_id, None),
            expect_narration
        );
    };
    let meta = import_character(
        root.path(),
        &world_id,
        raw(vec![opening.clone()]).as_bytes(),
        "#3366ff",
        "zh-TW",
    )
    .unwrap();
    check(true);
    data::delete_character(root.path(), &world_id, &meta.id).unwrap();
    let reply = display_script(
        r"/<StatusBlock>([\s\S]*?)<\/StatusBlock>/s",
        "<div>$1</div>",
    );
    import_character(
        root.path(),
        &world_id,
        raw(vec![opening, reply]).as_bytes(),
        "#3366ff",
        "zh-TW",
    )
    .unwrap();
    check(false);
}
