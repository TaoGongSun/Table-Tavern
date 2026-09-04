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
