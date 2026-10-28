//! 回覆收尾的控制標記清理（char-line-status-strip）：角色台詞、GM 中止半截、網頁存檔匯入共用。
//! 剝除本體仍是 `extract_state_block`；這裡補三件它不管的事——自閉合控制標籤、未閉合的尾巴、
//! 角色自加的本輪「名字：」。未閉合規則與前端串流截斷共用一份案例
//! （`src/shared/contracts/reply-cleanup/cases.json`）。
use super::response::extract_state_block;
use crate::data::{state_update_headings, StateNode};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

/// 自閉合的狀態類標籤（`<StatusPlaceHolderImpl/>`、`<status …/>`）。`find_state_tag` 不認自閉合，
/// 留著會把它配到後面的 `</status>`，中間正文一起被刪，所以 extract 之前先拿掉。
static SELF_CLOSING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<status[^<>]*/>").expect("valid regex"));

const STATE_FENCE_INFO: [&str; 4] = ["state", "status", "状态栏", "狀態欄"];
/// 中止時結尾停在這些標記的前半（至少兩字）就切；`<maintext>` 外殼的前半也算
const FRAGMENT_MARKERS: [&str; 5] = [
    "<updatevariable",
    "<status",
    "<details",
    "<maintext>",
    "</maintext>",
];

pub fn strip_self_closing_controls(text: &str) -> String {
    SELF_CLOSING.replace_all(text, "").into_owned()
}

/// 自閉合標籤先拿掉、再剝狀態區塊；回傳 trim_start 後的顯示文字。
fn control_free(text: &str) -> String {
    extract_state_block(&strip_self_closing_controls(text))
        .display
        .trim_start()
        .to_owned()
}

/// 切未閉合的尾巴。中止與完成都切：完整標記開頭但沒閉合的 `<UpdateVariable`、`<status…`、
/// ```state 圍欄，以及任何沒閉合的 `<details`。只在中止時切：結尾停在標記前半（至少兩字），
/// 以及任何沒閉合的 ```。單獨的 `<`、單雙反引號都不切。比對不分大小寫。
pub fn cut_unclosed_tail(text: &str, aborted: bool) -> &str {
    // to_ascii_lowercase 不改位元組長度，位置可以拿回原字串切
    let lower = text.to_ascii_lowercase();
    let mut cut = text.len();
    for marker in ["<updatevariable", "<status"] {
        if let Some(at) = lower.find(marker) {
            cut = cut.min(at);
        }
    }
    // details 照層級配對：留在堆疊裡的開標籤都沒閉合，從最外層那個切
    let mut open = Vec::new();
    let mut search = 0;
    loop {
        let next_open = lower[search..].find("<details").map(|at| search + at);
        let next_close = lower[search..].find("</details>").map(|at| search + at);
        match (next_open, next_close) {
            (Some(at), Some(close)) if close < at => {
                open.pop();
                search = close + "</details>".len();
            }
            (Some(at), _) => {
                open.push(at);
                search = at + "<details".len();
            }
            (None, Some(close)) => {
                open.pop();
                search = close + "</details>".len();
            }
            (None, None) => break,
        }
    }
    if let Some(&outer) = open.first() {
        cut = cut.min(outer);
    }
    let fences: Vec<usize> = lower.match_indices("```").map(|(at, _)| at).collect();
    if fences.len() % 2 == 1 {
        let at = *fences.last().expect("odd count is non-empty");
        let info_end = lower[at + 3..]
            .find('\n')
            .map_or(lower.len(), |index| at + 3 + index);
        let info = lower[at + 3..info_end].trim();
        if aborted || STATE_FENCE_INFO.contains(&info) {
            cut = cut.min(at);
        }
    }
    if aborted {
        let tail = lower[..cut].trim_end();
        for marker in FRAGMENT_MARKERS {
            if let Some(length) = (2..marker.len())
                .rev()
                .find(|&k| tail.ends_with(&marker[..k]))
            {
                cut = cut.min(tail.len() - length);
            }
        }
    }
    &text[..cut]
}

/// 最終文字（不處理名字前綴）：自閉合標籤→剝狀態區塊→切未閉合尾巴→去頭尾空白。
/// 網頁存檔匯入與共用案例的「最終結果」欄都用它。
pub fn final_reply_text(text: &str, aborted: bool) -> String {
    cut_unclosed_tail(&control_free(text), aborted)
        .trim_end()
        .to_owned()
}

/// 開頭（略過空白）是本輪前綴就剝；剝完是空也剝。英文前綴帶結尾空格，空回覆時只剩「Name:」也算。
fn strip_leading_prefix<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    if prefix.is_empty() {
        return None;
    }
    let text = text.trim_start();
    text.strip_prefix(prefix).or_else(|| {
        (!prefix.trim_end().is_empty() && text.trim_end() == prefix.trim_end()).then_some("")
    })
}

/// 角色回覆收尾的結果：`text` 落檔、送回模型、續聊對帳都用它；`raw` 是剝完前綴、還沒剝狀態區塊
/// 的原文（給卡片介面），與 `text` 相同就是 None。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterReply {
    pub text: String,
    pub raw: Option<String>,
    pub aborted: bool,
}

/// 角色回覆收尾（純函式）。順序：剝本輪前綴→自閉合標籤與狀態區塊→trim_start→
/// 第一次沒剝到前綴時、剝殼後才露出的前綴再剝一次（只剝一層）→切未閉合尾巴→trim_end。
pub fn finish_character_reply(reply: &str, prefix: &str, aborted: bool) -> CharacterReply {
    let (mut raw, first_stripped) = match strip_leading_prefix(reply, prefix) {
        Some(rest) => (rest.to_owned(), true),
        None => (reply.to_owned(), false),
    };
    let mut display = control_free(&raw);
    if !first_stripped {
        if let Some(rest) = strip_leading_prefix(&display, prefix) {
            let rest = rest.trim_start().to_owned();
            // raw 只拿掉正文開頭那一處前綴：逐一試拿掉原文裡的每個出現處，剝殼結果吻合的才是它
            // （控制區塊裡也可能有「名字：」，不能取第一個符合的位置）
            let target = rest.as_str();
            let mut candidates: Vec<(usize, usize)> = raw
                .match_indices(prefix)
                .map(|(at, found)| (at, found.len()))
                .collect();
            let trimmed = prefix.trim_end();
            if trimmed != prefix && !trimmed.is_empty() {
                candidates.extend(
                    raw.match_indices(trimmed)
                        .map(|(at, found)| (at, found.len())),
                );
            }
            if let Some(found) = candidates.into_iter().find_map(|(at, length)| {
                let candidate = format!("{}{}", &raw[..at], &raw[at + length..]);
                (control_free(&candidate).trim_start() == target).then_some(candidate)
            }) {
                raw = found;
            }
            display = rest;
        }
    }
    let text = cut_unclosed_tail(&display, aborted).trim_end().to_owned();
    let raw = (raw != text).then_some(raw);
    CharacterReply { text, raw, aborted }
}

/// 模型照抄進正文的「系統」標頭寫法（`system_line` 的兩種骨架，另收簡體）。
const SYSTEM_PREFIXES: [&str; 4] = ["（系統）", "（系统）", "(System) ", "(System)"];

/// GM 正文尾端照抄的狀態變動段（state-update-echo-in-story 的保險；主修是送模型的標頭）。
/// 傳入的是剝完狀態圍欄、更新區塊與下一位行的顯示文字；全部成立才剝，回剝完的文字：
/// 有一行（行首不縮排）完全等於「系統前綴＋狀態變動標頭」（字典十語系或送模型標頭）、不在 CommonMark 程式碼區塊裡（圍欄或縮排），
/// 且它之後到結尾每個非空行都是「本桌狀態樹葉子路徑：值」。任何一行對不上就回 None、整段保留。
pub fn strip_state_echo(display: &str, tree: &BTreeMap<String, StateNode>) -> Option<String> {
    let headings: BTreeSet<String> = state_update_headings()
        .into_iter()
        .flat_map(|heading| SYSTEM_PREFIXES.map(|prefix| format!("{prefix}{heading}")))
        .collect();
    let mut offset = 0;
    let mut start = None;
    for line in display.split_inclusive('\n') {
        // trim_end：行首有縮排的不算標頭（四空白縮排的程式碼區塊自然保留）
        if headings.contains(line.trim_end()) {
            start = Some(offset);
        }
        offset += line.len();
    }
    let start = start?;
    if in_code_block(display, start) {
        return None;
    }
    let mut paths = BTreeSet::new();
    collect_leaf_paths(tree, &mut Vec::new(), &mut paths);
    let mut body = display[start..]
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty());
    let is_path_line = |line: &str| {
        line.find(['：', ':'])
            .is_some_and(|at| paths.contains(line[..at].trim()))
    };
    let first = body.next()?;
    if !is_path_line(first) || !body.all(is_path_line) {
        return None;
    }
    Some(display[..start].trim_end().to_owned())
}

/// GM 送模型的旁白標籤（新「GM：」與舊「（旁白）」，中英；英文帶不帶空白都認，後面的空白一起剝）。
const NARRATOR_LABELS: [&str; 4] = ["（旁白）", "(Narration)", "GM：", "GM:"];

/// GM 正文開頭照抄的旁白標籤（narrator-prefix-echo 的保險；主修是送模型改標「GM：」）。
/// 只看顯示正文開頭（trim_start 後），連續幾個都剝；中段不碰。沒剝到回 None。
/// 剝完可能是空字串：呼叫端照「沒有正文」處理（gm_narrate 回 AI_EMPTY_RESPONSE）。
pub fn strip_narrator_label(display: &str) -> Option<String> {
    let mut rest = display.trim_start();
    let mut stripped = false;
    while let Some(label) = NARRATOR_LABELS
        .iter()
        .find(|label| rest.starts_with(*label))
    {
        rest = rest[label.len()..].trim_start();
        stripped = true;
    }
    stripped.then(|| rest.to_owned())
}

/// `at` 這個位置是否落在程式碼區塊裡（圍欄、縮排；沒閉合的圍欄照 CommonMark 延伸到文末）。
fn in_code_block(text: &str, at: usize) -> bool {
    use pulldown_cmark::{Event, Parser, Tag};
    Parser::new(text).into_offset_iter().any(|(event, range)| {
        matches!(event, Event::Start(Tag::CodeBlock(_))) && range.contains(&at)
    })
}

fn collect_leaf_paths(
    tree: &BTreeMap<String, StateNode>,
    path: &mut Vec<String>,
    paths: &mut BTreeSet<String>,
) {
    for (key, node) in tree {
        path.push(key.clone());
        match node {
            StateNode::Leaf(_) => {
                paths.insert(path.join("."));
            }
            StateNode::Branch(children) => collect_leaf_paths(children, path, paths),
        }
        path.pop();
    }
}

/// 剝完控制區塊沒有正文的回合錯誤（GM 與角色線同一個錯誤碼）。
pub fn empty_reply_error(reply: &str) -> String {
    format!(
        "AI_EMPTY_RESPONSE: no_text_after_control_lines raw_len={}",
        reply.chars().count()
    )
}

#[cfg(test)]
#[path = "reply_cleanup_tests.rs"]
mod tests;
