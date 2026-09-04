use super::parse_common::{join_trim, parse_blocks, parse_json_block, strip_html_fence, strip_json_fence};
use super::types::{
    EntryKind, GroupKind, RefactorAbsorbOutcome, RefactorExpandOutcome, RefactorNewEntry,
    RefactorPersonExpandOutcome, RefactorRewriteOutcome,
};
use crate::data::{FieldRule, Trigger};
use crate::refactor::{RefactorCharacter, RefactorInterface};
use std::collections::BTreeMap;

fn parse_character_body(lines: &[String]) -> (String, String, String) {
    let joined = lines.join("\n");
    let blocks = parse_blocks(&joined, &["EMOJI", "PUBLIC", "PRIVATE"]);
    let mut emoji = None;
    let mut public_md = String::new();
    let mut private_md = String::new();
    for block in &blocks {
        match block.marker {
            "EMOJI" => {
                // 容忍兩種寫法：同行「EMOJI: 🗡️」（value）或另起一行（lines）。
                let value = block.value.trim();
                let value = if value.is_empty() {
                    join_trim(&block.lines)
                } else {
                    value.to_owned()
                };
                if !value.is_empty() {
                    emoji = Some(value);
                }
            }
            "PUBLIC" => public_md = join_trim(&block.lines),
            "PRIVATE" => private_md = join_trim(&block.lines),
            _ => {}
        }
    }
    (
        emoji.unwrap_or_else(|| "🎭".to_owned()),
        public_md,
        private_md,
    )
}

/// person 展開：一人一次呼叫的結果只有一個角色。suspected_player 由呼叫端依盤點結果直接填入
/// （不是這裡自己判斷）；截斷輸出一樣保留已讀到的部分內容，不整批丟棄。
pub fn parse_person_expand(
    raw: &str,
    name: &str,
    source_uids: &[String],
    suspected_player: bool,
) -> RefactorPersonExpandOutcome {
    let lines: Vec<String> = raw.lines().map(str::to_owned).collect();
    if parse_blocks(raw, &["EMOJI", "PUBLIC", "PRIVATE"]).is_empty() {
        return RefactorPersonExpandOutcome {
            character: None,
            raw: raw.to_owned(),
        };
    }
    let (emoji, public_md, private_md) = parse_character_body(&lines);
    // solo_entry_md 不叫 AI 產：public_md＋空行＋private_md 拼成。
    let solo_entry_md = format!("{public_md}\n\n{private_md}");
    RefactorPersonExpandOutcome {
        character: Some(RefactorCharacter {
            name: name.to_owned(),
            emoji,
            public_md,
            private_md,
            source_uids: source_uids.to_vec(),
            solo_entry_md,
            suspected_player,
        }),
        raw: raw.to_owned(),
    }
}

/// interface 展開：STATE 區塊剝 ```json 圍欄後整段當 JSON 解；標記缺席或 JSON 壞掉一律 None，
/// 呼叫端退回 ExpandOutcome.raw（雙軌保底）。SHELL 區塊（```html 圍欄，只有 interface_shell
/// 變體會產，選配）另外抽：缺席或抽出來是空字串就 shell=None，不影響 state_fields 解不解析得
/// 出來；輸出被截斷（沒有結尾圍欄）也不會壞事，能抽多少算多少。
fn parse_interface_expand(raw: &str, entry_uid: &str) -> Option<RefactorInterface> {
    let blocks = parse_blocks(raw, &["STATE", "SHELL", "RULES", "GUIDE"]);
    let state_block = blocks.iter().find(|block| block.marker == "STATE")?;
    let text = join_trim(&state_block.lines);
    let state_fields: serde_json::Value = serde_json::from_str(strip_json_fence(&text)).ok()?;
    let shell = blocks
        .iter()
        .find(|block| block.marker == "SHELL")
        .map(|block| strip_html_fence(&join_trim(&block.lines)).to_owned())
        .filter(|shell| !shell.is_empty());
    let rules = parse_json_block(blocks.iter().find(|block| block.marker == "RULES"))
        .unwrap_or_default();
    let guide = blocks
        .iter()
        .find(|block| block.marker == "GUIDE")
        .map(|block| join_trim(&block.lines))
        .unwrap_or_default();
    Some(RefactorInterface {
        state_fields,
        source_uids: vec![entry_uid.to_owned()],
        raw: text,
        shell,
        rules,
        guide,
    })
}

pub fn parse_expand(kind: EntryKind, entry_uid: &str, raw: &str) -> RefactorExpandOutcome {
    let mut outcome = RefactorExpandOutcome {
        interface: None,
        raw: raw.to_owned(),
    };
    match kind {
        EntryKind::Interface | EntryKind::InterfaceShell => {
            outcome.interface = parse_interface_expand(raw, entry_uid)
        }
    }
    outcome
}

/// 接管解析：RULES／TRIGGERS 都走 `parse_json_block` 慣例——缺席或壞 JSON 退空集合，raw 留
/// 證據。
pub fn parse_absorb(raw: &str) -> RefactorAbsorbOutcome {
    let blocks = parse_blocks(raw, &["RULES", "TRIGGERS"]);
    let rules = parse_json_block::<BTreeMap<String, FieldRule>>(
        blocks.iter().find(|block| block.marker == "RULES"),
    )
    .unwrap_or_default();
    let triggers =
        parse_json_block::<Vec<Trigger>>(blocks.iter().find(|block| block.marker == "TRIGGERS"))
            .unwrap_or_default();
    RefactorAbsorbOutcome {
        rules,
        triggers,
        raw: raw.to_owned(),
    }
}

/// 合組解析：CONTENT 是主產物、必要（缺席＝整條失敗回 None）；RULES／TRIGGERS 是附加抽取，
/// 缺席或 JSON 壞掉都退成空集合、不拖垮 CONTENT。kind=setting 的呼叫本來就不會產出 RULES／
/// TRIGGERS 區塊，一樣走這條路徑（缺席即空集合，行為自然正確）。
pub fn parse_group(
    raw: &str,
    title: &str,
    kind: GroupKind,
    source_uids: &[String],
) -> RefactorRewriteOutcome {
    let blocks = parse_blocks(raw, &["CONTENT", "RULES", "TRIGGERS"]);
    let Some(content_block) = blocks.iter().find(|block| block.marker == "CONTENT") else {
        return RefactorRewriteOutcome {
            entry: None,
            raw: raw.to_owned(),
        };
    };
    let content = join_trim(&content_block.lines);
    if content.is_empty() {
        return RefactorRewriteOutcome {
            entry: None,
            raw: raw.to_owned(),
        };
    }
    let rules = parse_json_block::<BTreeMap<String, FieldRule>>(
        blocks.iter().find(|block| block.marker == "RULES"),
    )
    .unwrap_or_default();
    let triggers =
        parse_json_block::<Vec<Trigger>>(blocks.iter().find(|block| block.marker == "TRIGGERS"))
            .unwrap_or_default();
    RefactorRewriteOutcome {
        entry: Some(RefactorNewEntry {
            title: title.to_owned(),
            kind: kind.as_str().to_owned(),
            content,
            source_uids: source_uids.to_vec(),
            rules,
            triggers,
            meta: None,
        }),
        raw: raw.to_owned(),
    }
}

/// `{{span:uid#sN}}` 佔位符：absorb 的 TRIGGERS、group 的 CONTENT 用它指位引用原文段落，App
/// 組裝時換成該段全文（trim 過）。
fn span_placeholder_regex() -> &'static regex::Regex {
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| {
        regex::Regex::new(r"\{\{span:([^}]+)\}\}").expect("硬編碼 regex 必為合法樣式")
    })
}

/// 把文字裡的 `{{span:uid#sN}}` 佔位符換成該段原文（trim 過）：lookup 傳入佔位符裡的
/// `uid#sN` 引用字串、回傳該段原文；找不到（uid／段號無效、或那個 uid 根本不存在）就回
/// None，佔位符原樣保留、不炸也不留殘缺標記。呼叫端（absorb／split_group 的 tauri
/// command）已經有 by_uid，接 `refactor_assemble::resolve_span` 就是現成的 lookup。
pub fn expand_span_placeholders(text: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    span_placeholder_regex()
        .replace_all(text, |caps: &regex::Captures| {
            lookup(caps[1].trim())
                .map(|resolved| resolved.trim().to_owned())
                .unwrap_or_else(|| caps[0].to_owned())
        })
        .into_owned()
}
