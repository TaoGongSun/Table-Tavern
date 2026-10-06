use super::*;
use std::path::PathBuf;

fn msg(role: &str, content: &str) -> ChatMessage {
    ChatMessage {
        role: role.to_owned(),
        content: content.to_owned(),
    }
}

#[test]
fn flatten_restores_speaker_prefix_and_appends_turn_instruction() {
    let messages = [
        msg("system", "你在扮演狐狸"),
        msg("user", "玩家：晚安\n（旁白）打烊前"),
        msg("assistant", "晚安，要來一杯嗎？"),
        msg("user", "玩家：好啊"),
    ];
    let (system, prompt) = flatten_messages("狐狸", "現在輪到「狐狸」回應。", &messages, "zh-TW");
    assert_eq!(system, "你在扮演狐狸");
    assert!(prompt.contains("玩家：晚安\n（旁白）打烊前"));
    assert!(prompt.contains("狐狸：晚安，要來一杯嗎？"));
    assert!(prompt.ends_with("現在輪到「狐狸」回應。"));
}

/// agy 1.1.8 以下不認 --output-format，要在打過去之前擋下；版本字串認不得就放行，
/// 讓呼叫失敗時帶著 CLI 自己的錯誤訊息，而不是因為格式換了就把整條路擋死。
#[test]
fn agy_stream_json_support_gates_on_1_1_8() {
    assert!(!agy_supports_stream_json("1.1.7"));
    assert!(!agy_supports_stream_json("1.0.99"));
    assert!(!agy_supports_stream_json("0.9.9"));
    assert!(agy_supports_stream_json("1.1.8"));
    assert!(agy_supports_stream_json("1.1.17"));
    assert!(agy_supports_stream_json("2.0.0"));
    assert!(agy_supports_stream_json("agy version 1.1.17 (darwin)"));
    assert!(agy_supports_stream_json("")); // 認不得就放行
    assert!(agy_supports_stream_json("nightly"));
}

/// 共線後 messages 已自足：label 傳空就不再補名字前綴（否則「加爾：雷恩：……」），
/// closing 傳空就不再接收尾指示（否則本輪指定會出現兩次）。
#[test]
fn flatten_skips_label_and_closing_when_self_contained() {
    let messages = vec![
        msg("system", "共用 system"),
        msg("assistant", "加爾：抬起頭。"),
        msg("user", "現在你是「雷恩」。"),
    ];
    let (system, prompt) = flatten_messages("", "", &messages, "zh-TW");
    assert_eq!(system, "共用 system");
    assert_eq!(
        prompt,
        "以下是到目前為止的對話紀錄：\n\n加爾：抬起頭。\n\n現在你是「雷恩」。"
    );
    assert!(!prompt.contains("——")); // closing 為空就不留分隔線
                                     // 舊行為不變：有 label 就補前綴、有 closing 就接在後面
    let (_, legacy) = flatten_messages("雷恩", "收尾指示", &messages, "zh-TW");
    assert!(legacy.contains("雷恩：加爾：抬起頭。"));
    assert!(legacy.ends_with("——\n收尾指示"));
}

/// 正文一律不進命令列：沒有 -p、整串參數裡找不到正文（正文走 stdin）。
#[test]
fn agy_args_carry_no_prompt_on_the_command_line() {
    assert_eq!(
        agy_args(Some("Claude Sonnet 4.6 (Thinking)"), false),
        [
            "--model",
            "Claude Sonnet 4.6 (Thinking)",
            "--output-format",
            "stream-json"
        ]
    );
    assert_eq!(agy_args(None, false), ["--output-format", "stream-json"]);
    let image = agy_args(None, true);
    assert!(image.contains(&"--dangerously-skip-permissions".to_owned()));
    assert!(!image.contains(&"-p".to_owned()));
}

#[test]
fn agy_session_resumes_exact_id_and_body_sends_only_delta() {
    let open = agy_session_args(Some("gemini-x"), None);
    assert!(!open.contains(&"--conversation".to_owned()));
    assert!(!open.contains(&"-p".to_owned()));
    assert_eq!(
        agy_session_body("穩定 system", "第一輪", None),
        "穩定 system\n\n第一輪"
    );

    let resumed = agy_session_args(Some("gemini-x"), Some("conversation-1"));
    assert!(resumed
        .windows(2)
        .any(|pair| pair == ["--conversation", "conversation-1"]));
    assert!(!resumed.contains(&"--continue".to_owned()));
    assert!(!resumed.contains(&"-p".to_owned()));
    assert_eq!(
        agy_session_body("穩定 system", "只有新回合", Some("conversation-1")),
        "只有新回合"
    );
}

#[test]
fn grok_args_disable_every_tool_and_read_prompt_from_file() {
    let profile = PathBuf::from("/tmp/p/profile.md");
    let prompt_file = PathBuf::from("/tmp/p/prompt.txt");
    let args = grok_args(Some("grok-4.5"), Some(&profile), &prompt_file, false);
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--output-format", "streaming-json"]));
    assert!(args.windows(2).any(|pair| pair == ["--deny", "*"]));
    assert!(args.contains(&"--disable-web-search".to_owned()));
    assert!(args.contains(&"--no-plan".to_owned()));
    assert!(args.contains(&"--no-subagents".to_owned()));
    assert!(args.windows(2).any(|pair| pair == ["--max-turns", "1"]));
    // low 是 grok-4.6／4.5 選單的最低檔；none 會被 CLI 判成未知等級、整次生成中止
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--reasoning-effort", "low"]));
    assert!(args
        .windows(2)
        .any(|pair| pair[0] == "--disallowed-tools" && pair[1].contains("image_gen")));
    assert!(args.windows(2).any(|pair| pair == ["-m", "grok-4.5"]));
    // 文字通道：system 走 profile、正文走檔案且原樣送（不被搬檔成節錄）
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--agent", "/tmp/p/profile.md"]));
    assert!(args.contains(&"--verbatim".to_owned()));
    assert_eq!(
        args[args.len() - 2..],
        ["--prompt-file", "/tmp/p/prompt.txt"]
    );
    assert!(!args.contains(&"-p".to_owned()));
    assert!(!args.contains(&"--system-prompt-override".to_owned()));

    // 生圖要跑「呼叫工具→拿結果→回一句」，帶 max-turns 1 會斷在工具回傳那步；
    // 推理等級也維持原樣，免得把叫工具那步壓掉；保留原生 agent（不帶 profile、不加 verbatim）
    let image_args = grok_args(Some("grok-4.5"), None, &prompt_file, true);
    assert!(!image_args.contains(&"--disallowed-tools".to_owned()));
    assert!(!image_args.contains(&"--max-turns".to_owned()));
    assert!(!image_args.contains(&"--reasoning-effort".to_owned()));
    assert!(!image_args.contains(&"--agent".to_owned()));
    assert!(!image_args.contains(&"--verbatim".to_owned()));
    assert_eq!(
        image_args[image_args.len() - 2..],
        ["--prompt-file", "/tmp/p/prompt.txt"]
    );

    // 清單是逗號串，不能混進換行或空白（續行寫壞的話 CLI 會把整包當一個工具名）
    let list = args
        .windows(2)
        .find(|pair| pair[0] == "--disallowed-tools")
        .map(|pair| pair[1].clone())
        .expect("聊天單發要帶 --disallowed-tools");
    assert!(!list.contains(char::is_whitespace));
    assert_eq!(list.split(',').count(), 27);
    // shell 的顯示名與過濾 ID 不同名，只寫顯示名會靜默無效——兩個都要在
    assert!(list.contains("run_terminal_cmd,"));
    assert!(list.contains("run_terminal_command,"));
    // grok 1.0.46 新增的工具：漏列會讓 tool_count 停在 1
    assert!(list.ends_with(",send_feedback"));
}

#[test]
fn grok_payload_splits_system_into_profile_only_for_text_calls() {
    let (profile, body) = grok_payload("本桌 system", "整包 prompt", false);
    assert_eq!(profile.unwrap(), grok_agent_profile("本桌 system"));
    assert_eq!(body, "整包 prompt");
    // 生圖：system 併進正文、不換 grok 原生 system
    let (profile, body) = grok_payload("本桌 system", "整包 prompt", true);
    assert!(profile.is_none());
    assert_eq!(body, "本桌 system\n\n整包 prompt");
    // system 空（訊息串空）：不帶 profile，也不要在正文前面留兩個空行
    for allow_tools in [false, true] {
        let (profile, body) = grok_payload("", "整包 prompt", allow_tools);
        assert!(profile.is_none());
        assert_eq!(body, "整包 prompt");
    }
}

/// 字串層級：raw 包裝與單趟跳脫。是不是逐 byte 渲染回原文要看下面那支真 CLI 測試。
#[test]
fn grok_agent_profile_wraps_body_in_raw_and_escapes_tag_openers_once() {
    let profile = grok_agent_profile("a${%b${%${% endraw %}c");
    assert!(profile.starts_with(
        "---\nname: table-tavern\ndescription: Table Tavern\npromptMode: full\n---\n${% raw %}"
    ));
    assert!(profile.ends_with("${% endraw %}"));
    let body = &profile[profile.find("${% raw %}").unwrap()..];
    assert_eq!(
        body,
        concat!(
            "${% raw %}a",
            "${% endraw %}${{ \"${%\" }}${% raw %}b",
            "${% endraw %}${{ \"${%\" }}${% raw %}",
            "${% endraw %}${{ \"${%\" }}${% raw %} endraw %}c",
            "${% endraw %}"
        )
    );
    assert_eq!(
        grok_agent_profile(""),
        "---\nname: table-tavern\ndescription: Table Tavern\npromptMode: full\n---\n${% raw %}${% endraw %}"
    );
}

/// 真 grok CLI 渲染驗證（手動跑：`cargo test grok_agent_profile_renders_verbatim -- --ignored`）。
/// grok 在打 API 之前就把渲染後的 system 落到 session 的 system_prompt.txt，這裡逐 byte 比對。
/// 需要本機有 grok 且已登入（`GROK_HOME` 預設 ~/.grok）；額度用完（402）也照樣驗得到。
#[test]
#[ignore]
fn grok_agent_profile_renders_verbatim_with_real_cli() {
    let grok = crate::cli::detect::find_binary("grok").expect("找不到 grok");
    let base = std::env::temp_dir().join(format!("tt-grok-profile-{}", ulid::Ulid::generate()));
    let workspace = base.join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let grok_home = std::env::var_os("GROK_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap()).join(".grok"));
    let cases: Vec<(&str, String)> = vec![
        ("plain", "你是酒館的 GM。".to_owned()),
        ("edges", "\n\n  頭尾空白與縮排  \n\n".to_owned()),
        ("crlf", "第一行\r\n第二行\r\n\r\n".to_owned()),
        (
            "tags",
            "${{ tools.by_kind.read }} ${% if x %}A${% endif %} {{user}} {% raw %} ${# 註解 #}"
                .to_owned(),
        ),
        (
            "breakout",
            "${% endraw %}${%- endraw -%}${%endraw%} 之後 ${{ 1+1 }}".to_owned(),
        ),
        ("consecutive", "${%${%${% ${%%} $${% ${{%".to_owned()),
        ("dollar-tail", "結尾是錢號$".to_owned()),
        ("brace-tail", "結尾是 ${".to_owned()),
        ("frontmatter", "---\nname: fake\n---\n內文".to_owned()),
        ("big", "中文長桌測試。".repeat(150_000)),
    ];
    let mut failures = Vec::new();
    for (name, system) in &cases {
        let profile = base.join(format!("{name}.md"));
        std::fs::write(&profile, grok_agent_profile(system)).unwrap();
        let prompt = base.join(format!("{name}.txt"));
        std::fs::write(&prompt, "ok").unwrap();
        let session = uuid_like();
        let mut args = grok_session_args(
            Some("grok-4.7-build-fast"),
            Some(&profile),
            &prompt,
            &CliSession::Open(&session),
        );
        args.splice(
            0..0,
            ["--cwd".to_owned(), workspace.to_string_lossy().into_owned()],
        );
        let _ = std::process::Command::new(&grok)
            .args(&args)
            .env("GROK_HOME", &grok_home)
            .output()
            .unwrap();
        let rendered = std::fs::read_dir(grok_home.join("sessions"))
            .unwrap()
            .flatten()
            .map(|dir| dir.path().join(&session).join("system_prompt.txt"))
            .find(|path| path.exists())
            .map(|path| std::fs::read(path).unwrap());
        match rendered {
            Some(bytes) if bytes == system.as_bytes() => {}
            Some(bytes) => failures.push(format!(
                "{name}: 渲染結果 {} bytes 與原文 {} bytes 不同：{:?}",
                bytes.len(),
                system.len(),
                String::from_utf8_lossy(&bytes[..bytes.len().min(200)])
            )),
            None => failures.push(format!("{name}: 找不到 session 的 system_prompt.txt")),
        }
    }
    let _ = std::fs::remove_dir_all(&base);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// 正文那一側：`--prompt-file`＋`--verbatim` 送出的 user 訊息＝檔案內容去掉頭尾空白（不包
/// `<user_query>`、不渲染模板、超過 100KB 不搬檔）。頭尾空白是 grok 自己修的，舊的 `-p`
/// 一樣會修（2026-10-06 實測），不是本案引入。空 system 不帶 profile，所以「空字串」放在
/// 這一側驗：grok 對空正文直接回 `prompt is empty`、不建立回合；我們的正文一定帶本輪指示，碰不到。
/// 手動跑：`cargo test grok_prompt_file_is_sent_verbatim -- --ignored --nocapture`。
#[test]
#[ignore]
fn grok_prompt_file_is_sent_verbatim_with_real_cli() {
    let grok = crate::cli::detect::find_binary("grok").expect("找不到 grok");
    let base = std::env::temp_dir().join(format!("tt-grok-prompt-{}", ulid::Ulid::generate()));
    let workspace = base.join("ws");
    std::fs::create_dir_all(&workspace).unwrap();
    let grok_home = std::env::var_os("GROK_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap()).join(".grok"));
    let profile = base.join("profile.md");
    std::fs::write(&profile, grok_agent_profile("你是酒館的 GM。")).unwrap();
    let cases: Vec<(&str, String)> = vec![
        ("empty", String::new()),
        ("edges", "\n  頭尾空白\r\n第二行  \n\n".to_owned()),
        (
            "tags",
            "${{ x }} ${% if y %}Z${% endif %} <user_query>".to_owned(),
        ),
        ("big", "玩家：今晚的酒館很熱鬧。\n".repeat(12_000)),
    ];
    let mut failures = Vec::new();
    for (name, body) in &cases {
        let prompt = base.join(format!("{name}.txt"));
        std::fs::write(&prompt, body).unwrap();
        let session = uuid_like();
        let mut args = grok_session_args(
            Some("grok-4.7-build-fast"),
            Some(&profile),
            &prompt,
            &CliSession::Open(&session),
        );
        args.splice(
            0..0,
            ["--cwd".to_owned(), workspace.to_string_lossy().into_owned()],
        );
        let output = std::process::Command::new(&grok)
            .args(&args)
            .env("GROK_HOME", &grok_home)
            .output()
            .unwrap();
        let history = std::fs::read_dir(grok_home.join("sessions"))
            .unwrap()
            .flatten()
            .map(|dir| dir.path().join(&session).join("chat_history.jsonl"))
            .find(|path| path.exists());
        let sent = history.and_then(|path| {
            std::fs::read_to_string(path)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                .find(|line| !line["prompt_index"].is_null())
                .map(|line| line["content"][0]["text"].as_str().unwrap_or("").to_owned())
        });
        match sent {
            Some(text) if text == body.trim() => {
                println!(
                    "{name}: 與檔案（去頭尾空白）逐 byte 相同（{} bytes）",
                    text.len()
                )
            }
            Some(text) => failures.push(format!(
                "{name}: 送出 {} bytes 與檔案 {} bytes 不同：{:?}",
                text.len(),
                body.trim().len(),
                text.chars().take(120).collect::<String>()
            )),
            None if body.is_empty()
                && String::from_utf8_lossy(&output.stderr).contains("prompt is empty") =>
            {
                println!("{name}: grok 拒絕空正文（prompt is empty），沒建立回合")
            }
            None => failures.push(format!("{name}: 找不到送出的 user 訊息")),
        }
    }
    let _ = std::fs::remove_dir_all(&base);
    assert!(failures.is_empty(), "{failures:#?}");
}

fn uuid_like() -> String {
    let hex = format!("{:032x}", u128::from(ulid::Ulid::generate()));
    format!(
        "{}-{}-4{}-a{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[13..16],
        &hex[17..20],
        &hex[20..]
    )
}

#[test]
fn grok_session_open_carries_profile_and_resume_does_not() {
    let profile = PathBuf::from("/tmp/p/profile.md");
    let prompt = PathBuf::from("/tmp/p/prompt.txt");
    let open = grok_session_args(
        Some("grok-4.6"),
        Some(&profile),
        &prompt,
        &CliSession::Open("sid-1"),
    );
    // 開線：-s 建 session，system 這時才坐進去（grok 把它凍在 session 建立那刻）
    assert!(open.windows(2).any(|pair| pair == ["-s", "sid-1"]));
    assert!(open
        .windows(2)
        .any(|pair| pair == ["--agent", "/tmp/p/profile.md"]));
    assert!(open.contains(&"--verbatim".to_owned()));
    assert_eq!(
        open[open.len() - 2..],
        ["--prompt-file", "/tmp/p/prompt.txt"]
    );
    // 聊天單發那組硬化旗標一個都不能少（工具全拆、單輪、低推理）
    assert!(open.windows(2).any(|pair| pair == ["--max-turns", "1"]));
    assert!(open
        .windows(2)
        .any(|pair| pair == ["--reasoning-effort", "low"]));
    assert!(open
        .windows(2)
        .any(|pair| pair[0] == "--disallowed-tools" && pair[1].contains("image_gen")));
    assert!(open.windows(2).any(|pair| pair == ["--deny", "*"]));
    assert!(open.windows(2).any(|pair| pair == ["-m", "grok-4.6"]));

    // 續聊：-r 接同一條線，system 不重帶（重帶無效又會打散前綴），增量進正文檔
    let resume = grok_session_args(
        Some("grok-4.6"),
        Some(&profile),
        &prompt,
        &CliSession::Resume("sid-1"),
    );
    assert!(resume.windows(2).any(|pair| pair == ["-r", "sid-1"]));
    assert!(!resume.contains(&"--agent".to_owned()));
    assert!(!resume.contains(&"-s".to_owned()));
    assert!(resume.contains(&"--verbatim".to_owned()));
    assert_eq!(
        resume[resume.len() - 2..],
        ["--prompt-file", "/tmp/p/prompt.txt"]
    );

    // 未覆寫模型＝不帶 -m，由 CLI 自己選預設；system 空就不帶 profile
    let default_model = grok_session_args(None, None, &prompt, &CliSession::Open("sid-2"));
    assert!(!default_model.contains(&"-m".to_owned()));
    assert!(!default_model.contains(&"--agent".to_owned()));
}

#[test]
fn grok_envs_point_home_and_grok_home_at_the_app_profile() {
    let envs = grok_envs(
        &PathBuf::from("/app/cli-home"),
        &PathBuf::from("/app/grok-home"),
    );
    // HOME 換掉才擋得住 ~/.claude 的 hooks／CLAUDE.md；Windows 認的是 USERPROFILE
    assert!(envs.contains(&("HOME".to_owned(), "/app/cli-home".to_owned())));
    assert!(envs.contains(&("USERPROFILE".to_owned(), "/app/cli-home".to_owned())));
    // GROK_HOME 另指一處，登入態才不會跟使用者終端機的 ~/.grok 混在一起
    assert!(envs.contains(&("GROK_HOME".to_owned(), "/app/grok-home".to_owned())));
    // 取樣參數與關伺服器端工具走 GROK_CONFIG 疊加層：只在 app 這幾次呼叫生效，不寫進任何 config.toml
    assert!(envs.contains(&(
        "GROK_CONFIG".to_owned(),
        r#"{"models":{"temperature":1.1,"top_p":1.0},"features":{"backend_tools":false}}"#
            .to_owned()
    )));
    // 伺服器端工具的環境變數入口（--disable-web-search 管不到 x_search）
    assert!(envs.contains(&("GROK_BACKEND_SEARCH".to_owned(), "0".to_owned())));
    // 關掉遠端 campaign，否則 -m 非 campaign 預設模型時 system override 會失效
    assert!(envs.contains(&("GROK_CAMPAIGNS".to_owned(), "0".to_owned())));
}

#[test]
fn tier_mappings_cover_all_tiers() {
    assert_eq!(claude_model_for(Tier::Best), "opus");
    assert_eq!(claude_model_for(Tier::Fast), "haiku");
    assert_eq!(codex_effort_for(Tier::Balanced), "medium");
    let args = codex_args(None, codex_effort_for(Tier::Best), false);
    assert!(args.contains(&"model_reasoning_effort=\"high\"".to_owned()));
    assert!(!args.contains(&"-m".to_owned()));
    assert_eq!(args.last().unwrap(), "-");
    let args = codex_args(Some("gpt-5.6-terra"), codex_effort_for(Tier::Fast), false);
    assert!(args.windows(2).any(|w| w == ["-m", "gpt-5.6-terra"]));
    let args = claude_args(
        claude_model_for(Tier::Fast),
        &PathBuf::from("/tmp/p/system.txt"),
    );
    assert!(args.windows(2).any(|w| w == ["--model", "haiku"]));
    assert!(args
        .windows(2)
        .any(|w| w == ["--system-prompt-file", "/tmp/p/system.txt"]));
    assert!(!args.contains(&"--system-prompt".to_owned()));
}

/// lane 續聊參數必須保留 session 落檔（無 --no-session-persistence），
/// 開線帶 --session-id、續聊帶 --resume，其餘旗標與單發相同。
#[test]
fn claude_session_args_keep_persistence_and_pick_session_flag() {
    let system = PathBuf::from("/tmp/p/system.txt");
    let opened = claude_session_args("sonnet", &system, &CliSession::Open("uuid-1"));
    assert!(!opened.contains(&"--no-session-persistence".to_owned()));
    assert!(opened.windows(2).any(|w| w == ["--session-id", "uuid-1"]));
    assert!(opened
        .windows(2)
        .any(|w| w == ["--system-prompt-file", "/tmp/p/system.txt"]));
    assert!(opened.windows(2).any(|w| w == ["--model", "sonnet"]));
    let resumed = claude_session_args("sonnet", &system, &CliSession::Resume("uuid-1"));
    assert!(resumed.windows(2).any(|w| w == ["--resume", "uuid-1"]));
    assert!(!resumed.contains(&"--session-id".to_owned()));
}

#[test]
fn tier_override_reads_prefixed_keys_and_ignores_blank() {
    let mut map = std::collections::BTreeMap::new();
    map.insert("claude:best".to_owned(), "claude-fable-5".to_owned());
    map.insert("claude:fast".to_owned(), "  ".to_owned());
    map.insert("best".to_owned(), "vendor/api-model".to_owned()); // API 檔位不受影響
    assert_eq!(
        tier_override(&map, "claude", Tier::Best),
        Some("claude-fable-5")
    );
    assert_eq!(tier_override(&map, "claude", Tier::Fast), None); // 空白＝未設
    assert_eq!(tier_override(&map, "codex", Tier::Best), None);
}
