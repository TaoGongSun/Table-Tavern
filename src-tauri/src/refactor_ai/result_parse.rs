use super::parse_common::{
    join_trim, parse_blocks, parse_json_block, strip_code_fence, strip_json_fence,
};
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

/// interface 展開（兩種 kind 同一份契約）：STATE／SHELL／RULES／GUIDE 四塊都要完整才算產物。
/// 任何一塊缺席、壞掉或彼此對不上（佔位符引用的路徑不是 STATE 的葉子、或沒有欄位規則）一律回 None，
/// 呼叫端把這條記成失敗、來源條目不被消耗——絕不落成「只有 STATE」的半套桌。
fn parse_interface_expand(raw: &str, entry_uid: &str) -> Option<RefactorInterface> {
    let blocks = parse_blocks(raw, &["STATE", "SHELL", "RULES", "GUIDE"]);
    let block = |marker: &str| blocks.iter().find(|block| block.marker == marker);
    let text = join_trim(&block("STATE")?.lines);
    let state_fields: serde_json::Value = serde_json::from_str(strip_json_fence(&text)).ok()?;
    if !state_fields.is_object() {
        return None;
    }
    let shell = strip_code_fence(&join_trim(&block("SHELL")?.lines)).to_owned();
    let rules_block = block("RULES")?;
    if join_trim(&rules_block.lines).is_empty() {
        return None;
    }
    let rules: BTreeMap<String, FieldRule> = parse_json_block(Some(rules_block))?;
    let guide = join_trim(&block("GUIDE")?.lines);
    if shell.is_empty() || guide.is_empty() || !placeholders_complete(&shell, &state_fields, &rules)
    {
        return None;
    }
    Some(RefactorInterface {
        state_fields,
        source_uids: vec![entry_uid.to_owned()],
        raw: text,
        shell: Some(shell),
        rules,
        guide,
    })
}

/// 正文槽：App 每回合拿模型的訊息正文填進去。
const BODY_PLACEHOLDER: &str = "本回合.正文";

/// 原卡的酒館巨集清單：前後端共用 src/shared/contracts/st-macros/st-macros.json 這一份，不各寫各的。
#[derive(serde::Deserialize)]
struct StMacros {
    names: Vec<String>,
    argument_names: Vec<String>,
}

static ST_MACROS: std::sync::LazyLock<StMacros> = std::sync::LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../../../src/shared/contracts/st-macros/st-macros.json"
    ))
    .expect("st-macros.json 格式錯誤")
});

/// 是不是已知的酒館巨集：無參名稱，或 `名稱::參數`／`名稱:參數` 的已知帶參名稱（不分大小寫）。
pub(super) fn is_st_macro(token: &str) -> bool {
    let lower = token.trim().to_lowercase();
    if ST_MACROS.names.iter().any(|name| *name == lower) {
        return true;
    }
    lower
        .find(':')
        .filter(|colon| *colon > 0)
        .is_some_and(|colon| {
            ST_MACROS
                .argument_names
                .iter()
                .any(|name| *name == lower[..colon])
        })
}

/// 骨架佔位符契約（與前端 classifyPlaceholder 相同）：正文槽；STATE 有這個葉子就是狀態引用（優先於巨集名）；
/// 已知酒館巨集原樣保留；其餘一律當狀態路徑（含未知的冒號寫法）。佔位符以外是固定文字。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum PlaceholderKind {
    Body,
    Macro,
    Path,
}

pub(super) fn classify_placeholder(token: &str, state: &serde_json::Value) -> PlaceholderKind {
    let trimmed = token.trim();
    if trimmed == BODY_PLACEHOLDER {
        PlaceholderKind::Body
    } else if leaf_at(state, trimmed) {
        PlaceholderKind::Path
    } else if is_st_macro(trimmed) {
        PlaceholderKind::Macro
    } else {
        PlaceholderKind::Path
    }
}

fn leaf_at(state: &serde_json::Value, path: &str) -> bool {
    let mut node = state;
    for key in path.split('.') {
        match node.get(key) {
            Some(next) => node = next,
            None => return false,
        }
    }
    !node.is_object() && !node.is_array()
}

/// 骨架裡每個狀態路徑佔位符都要是 STATE 的葉子、且有一條欄位規則；正文槽與已知巨集不算路徑。
/// 佔位符只認 `{{...}}` 內不含花括號與換行的形式（與前端填值相同），其餘照固定文字看。
pub(super) fn placeholders_complete(
    shell: &str,
    state: &serde_json::Value,
    rules: &BTreeMap<String, FieldRule>,
) -> bool {
    let mut rest = shell;
    while let Some(open) = rest.find("{{") {
        let after = &rest[open + 2..];
        let Some(close) = after.find("}}") else {
            break;
        };
        let token = &after[..close];
        rest = &after[close + 2..];
        if token.contains(['{', '}', '\n']) {
            continue;
        }
        if classify_placeholder(token, state) != PlaceholderKind::Path {
            continue;
        }
        let path = token.trim();
        if path.is_empty() || !rules.contains_key(path) || !leaf_at(state, path) {
            return false;
        }
    }
    true
}

pub fn parse_expand(kind: EntryKind, entry_uid: &str, raw: &str) -> RefactorExpandOutcome {
    let mut outcome = RefactorExpandOutcome {
        interface: None,
        raw: raw.to_owned(),
    };
    match kind {
        EntryKind::InterfaceShell | EntryKind::InterfaceStatusbar => {
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
