use super::types::{EntrySpan, PrescanSignal};
use crate::data::{self, DataResult, WorldbookEntry};
use crate::mechanism::{self, RecordKind};
use std::path::Path;

/// 組裝一次 AI 讀卡要看到的完整脈絡：world.md＋世界書全部條目（含 uid／constant／disabled
/// 旗標／全文，讓 AI 好引用 uid）＋既有角色卡（唯讀，AI 不必重拆這些）＋機制帳本已接管／跳過
/// 清單（唯讀，AI 不必再列這些條目）。
pub fn assemble_card_context(root: &Path, world_id: &str) -> DataResult<String> {
    let world_md = data::read_world_md(root, world_id)?;
    let worldbook = data::read_worldbook(root, world_id)?;
    let characters = data::list_characters(root, world_id)?;
    let ledger = mechanism::read_ledger(root, world_id);

    let mut out = String::new();
    out.push_str("### 世界設定（world.md）\n");
    let world_md = world_md.trim();
    out.push_str(if world_md.is_empty() {
        "（無）"
    } else {
        world_md
    });

    out.push_str("\n\n### 世界書條目\n");
    if worldbook.is_empty() {
        out.push_str("（無）\n");
    } else {
        for entry in &worldbook {
            out.push_str(&format_worldbook_entry(entry));
            out.push('\n');
        }
    }

    out.push_str("\n### 既有角色卡（唯讀脈絡，不必重新拆這些）\n");
    if characters.is_empty() {
        out.push_str("（無）\n");
    } else {
        for meta in &characters {
            if let Ok(card) = data::read_character(root, world_id, &meta.id) {
                out.push_str(&format!(
                    "#### {}\nPUBLIC:\n{}\nPRIVATE:\n{}\n\n",
                    card.name, card.public_md, card.private_md
                ));
            }
        }
    }

    if !ledger.entries.is_empty() {
        out.push_str("\n### 機制帳本（唯讀脈絡）\n");
        for entry in &ledger.entries {
            match entry.kind {
                RecordKind::Absorbed => out.push_str(&format!(
                    "- uid={} 《{}》已接管，不必再拆：{}\n",
                    entry.uid, entry.title, entry.detail
                )),
                RecordKind::Skipped => out.push_str(&format!(
                    "- uid={} 《{}》曾嘗試接管失敗（原因：{}），是重構目標\n",
                    entry.uid, entry.title, entry.detail
                )),
                _ => {}
            }
        }
    }

    Ok(out)
}

/// 把條目內容依空行切成一組 span：分隔用的空行 byte 併入前一個 span 尾端，所以全部 span 依序
/// 串接（原 byte 區間）必定等於原文——span 之間不留縫、不重疊。整段沒有空行（或全是空行、沒有
/// 任何實質內容）＝一個 span；空字串沒有內容可切，回空 Vec。
pub fn segment_spans(content: &str) -> Vec<EntrySpan> {
    if content.is_empty() {
        return Vec::new();
    }
    // 逐行取得 (start, end, 是否空行)；end 含換行字元，方便直接拿來當下一行的 start。
    let mut lines: Vec<(usize, usize, bool)> = Vec::new();
    let mut pos = 0usize;
    for line in content.split_inclusive('\n') {
        let end = pos + line.len();
        let stripped = line.strip_suffix('\n').unwrap_or(line);
        let stripped = stripped.strip_suffix('\r').unwrap_or(stripped);
        lines.push((pos, end, stripped.trim().is_empty()));
        pos = end;
    }

    let mut spans = Vec::new();
    let mut span_start = 0usize;
    let mut index = 0usize;
    let mut seen_content = false;
    while index < lines.len() {
        if !lines[index].2 {
            seen_content = true;
            index += 1;
            continue;
        }
        // 空行 run：吃到下一個非空行（或結尾）為止。
        while index < lines.len() && lines[index].2 {
            index += 1;
        }
        // 前面已經有內容、後面還有內容 → 這個空行 run 才是真正的段落分界；純前導或純尾隨空行
        // 併入唯一相鄰的那個 span，不獨立成段。
        if seen_content && index < lines.len() {
            let boundary = lines[index].0;
            spans.push(EntrySpan {
                id: spans.len() + 1,
                start: span_start,
                end: boundary,
            });
            span_start = boundary;
            seen_content = false;
        }
    }
    spans.push(EntrySpan {
        id: spans.len() + 1,
        start: span_start,
        end: content.len(),
    });
    spans
}

/// 把條目內容依 span 切分後，在每個 span 第一行行首插入 `⟦sN⟧` 標記（各條各自從 s1 起編）；
/// span 恰好互不重疊地串成整段原文，插入標記不會遺漏或錯位任何一個 byte。
fn mark_entry_spans(content: &str) -> String {
    let spans = segment_spans(content);
    let mut marked = String::with_capacity(content.len() + spans.len() * 8);
    for span in &spans {
        marked.push_str(&format!("⟦s{}⟧", span.id));
        marked.push_str(&content[span.start..span.end]);
    }
    marked
}

fn format_worldbook_entry(entry: &WorldbookEntry) -> String {
    let mut flags = Vec::new();
    if entry.constant {
        flags.push("constant");
    }
    if entry.disabled {
        flags.push("disabled");
    }
    let flags = if flags.is_empty() {
        String::new()
    } else {
        format!("［{}］", flags.join("、"))
    };
    format!(
        "#### uid={} {}{}\n{}\n",
        entry.uid,
        entry.title,
        flags,
        mark_entry_spans(&entry.content)
    )
}

/// 展開階段要看的「該條目全文」：找不到就回錯，呼叫端（tauri command）轉成 Err(String) 往上拋。
pub fn entry_full_text(root: &Path, world_id: &str, entry_uid: &str) -> DataResult<String> {
    let uid: u64 = entry_uid
        .parse()
        .map_err(|_| data::invalid_data(format!("uid 不是合法數字：{entry_uid}")))?;
    let entry = data::read_worldbook(root, world_id)?
        .into_iter()
        .find(|entry| entry.uid == uid)
        .ok_or_else(|| data::invalid_data(format!("找不到 uid={uid} 的世界書條目")))?;
    Ok(format_worldbook_entry(&entry))
}

/// 逐日樣式 regex：`第[一二三四五六七八九十\d]+天`／`每日`／`\bday ?\d`，三選一命中即算；只編譯
/// 一次（全模組共用，內容不變）。
fn daily_style_regex() -> &'static regex::Regex {
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| {
        regex::Regex::new(r"(?i)第[一二三四五六七八九十\d]+天|每日|\bday ?\d")
            .expect("硬編碼 regex 必為合法樣式")
    })
}

/// 語言無關結構特徵（2026-08-12 拍板：詞彙 regex 逐語言堆是打地鼠，改抓卡片生態跨語言的
/// 「形」）。模板變數排除 {{user}}／{{char}}——人稱代換是敘事慣用法，留著會把大半設定文本
/// 都標成機制訊號。
fn template_var_regex() -> &'static regex::Regex {
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| {
        regex::Regex::new(r"\{\{\s*([^}]{1,40})\}\}").expect("硬編碼 regex 必為合法樣式")
    })
}

fn html_tag_regex() -> &'static regex::Regex {
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| {
        regex::Regex::new(r"(?i)</?[a-z][a-z0-9]*(\s[^>]*)?>").expect("硬編碼 regex 必為合法樣式")
    })
}

fn percent_regex() -> &'static regex::Regex {
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| regex::Regex::new(r"\d+\s*[%％]").expect("硬編碼 regex 必為合法樣式"))
}

/// 對世界書全部條目做結構預掃：逐條切 span，各 span 比對語言無關的結構特徵——模板變數
/// （{{…}}，排除 user/char）、表格（`|…|` 行 ≥3）、代碼塊（```）、HTML 標籤、百分比數值、
/// 逐日樣式，加上 `trigger:`／`rule:` 詞彙（免費加分，非主力）；一個 span 命中多個 pattern
/// 各記一筆。純粹的機械掃描，不代表一定要 absorb——只是給判官一份「這裡可能有機制／格式」
/// 的參考清單，判定衝突（例如命中卻判 carry）由判官在 ENTRIES 附 reason 說明。
pub fn prescan_worldbook(entries: &[WorldbookEntry]) -> Vec<PrescanSignal> {
    let daily = daily_style_regex();
    let template = template_var_regex();
    let html = html_tag_regex();
    let percent = percent_regex();
    let mut signals = Vec::new();
    for entry in entries {
        for span in segment_spans(&entry.content) {
            let text = &entry.content[span.start..span.end];
            let lower = text.to_lowercase();
            let span_ref = format!("{}#s{}", entry.uid, span.id);
            let mut push = |pattern: &str| {
                signals.push(PrescanSignal {
                    uid: entry.uid.to_string(),
                    span: span_ref.clone(),
                    pattern: pattern.to_owned(),
                });
            };
            if lower.contains("trigger:") {
                push("trigger:");
            }
            if lower.contains("rule:") {
                push("rule:");
            }
            if daily.is_match(text) {
                push("逐日樣式");
            }
            if template.captures_iter(text).any(|cap| {
                let name = cap[1].trim().to_lowercase();
                name != "user" && name != "char"
            }) {
                push("模板變數");
            }
            if text.lines().filter(|line| line.trim_start().starts_with('|')).count() >= 3 {
                push("表格");
            }
            if text.contains("```") || html.is_match(text) {
                push("代碼或標籤");
            }
            if percent.is_match(text) {
                push("百分比數值");
            }
        }
    }
    signals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Visibility;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(std::path::PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "table-tavern-refactor-ai-{}-{}",
                std::process::id(),
                NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    // ---- assemble_card_context ----

    #[test]
    fn assemble_card_context_lists_uid_flags_characters_and_ledger() {
        let root = TestRoot::new();
        let world_id = data::create_world(&root.0, "夜港").unwrap();
        data::write_world_md(&root.0, &world_id, "迷霧籠罩碼頭。").unwrap();
        data::upsert_worldbook_entry(
            &root.0,
            &world_id,
            WorldbookEntry {
                uid: u64::MAX,
                title: "船員合集".to_owned(),
                keys: Vec::new(),
                content: "亞瑟與莫斯都在碼頭工作。".to_owned(),
                constant: true,
                order: 0,
                disabled: false,
                visibility: Visibility::Gm,
                is_person: false,
                locked: false,
            },
        )
        .unwrap();
        data::write_character(
            &root.0,
            &world_id,
            &data::CharacterCard {
                id: data::new_id(),
                name: "伊利亞".to_owned(),
                color: "#fff".to_owned(),
                avatar: "🦊".to_owned(),
                tier: data::Tier::Balanced,
                show_image: true,
                archived: false,
                gen_prompt: String::new(),
                public_md: "走私者。".to_owned(),
                private_md: "欠了債。".to_owned(),
            },
        )
        .unwrap();
        mechanism::append_log(
            &root.0,
            &world_id,
            0,
            &[mechanism::Record {
                kind: RecordKind::Absorbed,
                path: "船員合集".to_owned(),
                detail: "偵測到 [initvar] 標記".to_owned(),
            }],
        );

        let context = assemble_card_context(&root.0, &world_id).unwrap();
        assert!(context.contains("迷霧籠罩碼頭。"));
        assert!(context.contains("uid=") && context.contains("船員合集"));
        assert!(context.contains("［constant］"));
        assert!(
            context.contains("伊利亞")
                && context.contains("走私者。")
                && context.contains("欠了債。")
        );
        assert!(context.contains("已接管") && context.contains("偵測到 [initvar] 標記"));
    }

    // ---- span 切分 ----

    /// 把 span 依序切片串接回去；用來驗證「span 依序串接必得原文 byte 相等」這條不變量。
    fn reassemble(content: &str, spans: &[EntrySpan]) -> String {
        spans
            .iter()
            .map(|span| &content[span.start..span.end])
            .collect()
    }

    // 單段（無空行）＝整條一個 span
    #[test]
    fn segment_spans_single_paragraph_yields_one_span() {
        let content = "整段沒有空行的內容。";
        let spans = segment_spans(content);
        assert_eq!(
            spans,
            vec![EntrySpan {
                id: 1,
                start: 0,
                end: content.len()
            }]
        );
    }

    // property 式檢查：不管內容長什麼樣子，span 依序串接（含分隔空行）都必須等於原文 byte
    // 相等，而且 id 從 1 連號、彼此不重疊不留縫。涵蓋多段、多重空行、前導／尾隨空行、全空白、
    // 空字串、中英文與 emoji 混排等形狀。
    #[test]
    fn segment_spans_reassembly_is_byte_identical_to_original_for_various_shapes() {
        let samples = [
            "第一段。\n\n第二段。\n\n第三段。",
            "第一段。\n\n\n第二段（前面兩個空行）。",
            "只有一段，結尾帶換行。\n",
            "\n\n前導空行後才有內容。\n\n又一段。",
            "中英文與 emoji 混排 🎲\n\ntrigger: 第七天 🔥\n\n每日簽到 day 3",
            "",
            "   \n\n   ",
        ];
        for content in samples {
            let spans = segment_spans(content);
            assert_eq!(reassemble(content, &spans), content, "樣本：{content:?}");
            let mut expected_start = 0usize;
            for (index, span) in spans.iter().enumerate() {
                assert_eq!(span.id, index + 1, "樣本：{content:?}");
                assert_eq!(span.start, expected_start, "樣本：{content:?}");
                expected_start = span.end;
            }
            assert_eq!(expected_start, content.len(), "樣本：{content:?}");
        }
    }

    // 空字串沒有內容可切
    #[test]
    fn segment_spans_empty_content_yields_no_spans() {
        assert!(segment_spans("").is_empty());
    }

    // ---- format_worldbook_entry：⟦sN⟧ 標記 ----

    #[test]
    fn format_worldbook_entry_injects_span_markers_at_each_segment_head() {
        let entry = WorldbookEntry {
            uid: 7,
            title: "測試條目".to_owned(),
            keys: Vec::new(),
            content: "第一段內容。\n\n第二段內容。".to_owned(),
            constant: false,
            order: 0,
            disabled: false,
            visibility: Visibility::Gm,
            is_person: false,
            locked: false,
        };
        let formatted = format_worldbook_entry(&entry);
        assert!(formatted.contains("⟦s1⟧第一段內容。"));
        assert!(formatted.contains("⟦s2⟧第二段內容。"));
        // 標記拿掉就是原文，沒有遺漏或錯位任何 byte。
        let stripped = formatted.replace("⟦s1⟧", "").replace("⟦s2⟧", "");
        assert!(stripped.contains(&entry.content));
    }

    // ---- 結構預掃 ----

    fn sample_entry(uid: u64, content: &str) -> WorldbookEntry {
        WorldbookEntry {
            uid,
            title: format!("條目{uid}"),
            keys: Vec::new(),
            content: content.to_owned(),
            constant: false,
            order: 0,
            disabled: false,
            visibility: Visibility::Gm,
            is_person: false,
            locked: false,
        }
    }

    // trigger:／rule: 不分大小寫都要命中
    #[test]
    fn prescan_worldbook_matches_trigger_and_rule_case_insensitively() {
        let entries = vec![
            sample_entry(1, "Trigger: 好感度 >= 50 時觸發告白"),
            sample_entry(2, "RULE: 每次戰鬥扣血"),
        ];
        let signals = prescan_worldbook(&entries);
        assert_eq!(signals.len(), 2);
        assert_eq!(signals[0].uid, "1");
        assert_eq!(signals[0].span, "1#s1");
        assert_eq!(signals[0].pattern, "trigger:");
        assert_eq!(signals[1].uid, "2");
        assert_eq!(signals[1].pattern, "rule:");
    }

    // 逐日樣式三個子樣式（第 X 天／每日／day N）都要命中，且不分大小寫
    #[test]
    fn prescan_worldbook_matches_daily_style_variants() {
        let entries = vec![
            sample_entry(1, "第七天會有商隊經過"),
            sample_entry(2, "每日簽到可以領獎勵"),
            sample_entry(3, "Day 3 之後解鎖新地圖"),
        ];
        let signals = prescan_worldbook(&entries);
        assert_eq!(signals.len(), 3);
        for signal in &signals {
            assert_eq!(signal.pattern, "逐日樣式");
        }
    }

    // 一個 span 命中多個 pattern 各記一筆
    #[test]
    fn prescan_worldbook_records_one_signal_per_pattern_hit_on_same_span() {
        let entries = vec![sample_entry(9, "trigger: 第七天 rule: 扣血")];
        let signals = prescan_worldbook(&entries);
        assert_eq!(signals.len(), 3);
        assert!(signals.iter().all(|signal| signal.span == "9#s1"));
        let patterns: Vec<&str> = signals
            .iter()
            .map(|signal| signal.pattern.as_str())
            .collect();
        assert!(patterns.contains(&"trigger:"));
        assert!(patterns.contains(&"rule:"));
        assert!(patterns.contains(&"逐日樣式"));
    }

    // 不含任何封閉字彙的條目沒有訊號
    #[test]
    fn prescan_worldbook_yields_no_signal_for_plain_narrative() {
        let entries = vec![sample_entry(
            1,
            "這裡只是單純的世界觀敘述，沒有任何機制字樣。",
        )];
        assert!(prescan_worldbook(&entries).is_empty());
    }

    // 語言無關結構特徵（2026-08-12）：模板變數／表格／代碼標籤／百分比跨語言命中；
    // {{user}}／{{char}} 是敘事人稱代換，不算訊號
    #[test]
    fn prescan_worldbook_matches_language_agnostic_structural_features() {
        let entries = vec![
            sample_entry(1, "好感度用 {{affection}} 追蹤，衰減見下表。"),
            sample_entry(
                2,
                "| 阶段 | 天数 | 效果 |\n| 一 | 3 | 无 |\n| 二 | 7 | 强化 |",
            ),
            sample_entry(3, "输出格式：<status>体力值</status> 包裹。"),
            sample_entry(4, "成功率基础 30％，每级加 5%。"),
            sample_entry(5, "{{user}}與{{char}}在王府相遇，純敘事。"),
        ];
        let signals = prescan_worldbook(&entries);
        let hit = |uid: &str, pattern: &str| signals.iter().any(|s| s.uid == uid && s.pattern == pattern);
        assert!(hit("1", "模板變數"));
        assert!(hit("2", "表格"));
        assert!(hit("3", "代碼或標籤"));
        assert!(hit("4", "百分比數值"));
        assert!(!signals.iter().any(|s| s.uid == "5"), "純人稱變數不得成訊號：{signals:?}");
    }

    // 多段條目：訊號要標對 span 序號
    #[test]
    fn prescan_worldbook_references_correct_span_id_across_multiple_paragraphs() {
        let entries = vec![sample_entry(
            4,
            "第一段是單純敘述。\n\n第二段才有 trigger: 條件。",
        )];
        let signals = prescan_worldbook(&entries);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].span, "4#s2");
    }
}
