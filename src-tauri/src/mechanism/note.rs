//! 給下一輪 GM 的回饋句（notes）：路徑寫法、整條字數上限、列鍵截斷、「路徑不存在」的分組合併。
use super::types::{NoteGroup, Record, RecordKind};

/// 一條回饋句的硬上限（Unicode 字元）。
pub(super) const NOTE_MAX: usize = 240;
/// 一輪最多回饋幾條。
const NOTES_MAX: usize = 5;
/// 提示裡的路徑超過這個長度就中段省略。
const PATH_MAX: usize = 120;
/// 單一鍵名超過這個長度就截斷。
const KEY_MAX: usize = 24;
/// 合併尾綴「本輪 N 筆未套用：…」的鍵名清單最多占多少字。
const LEAVES_BUDGET: usize = 60;

/// 拒收記錄 → 回饋句：依記錄順序取前 5 條；同文去重；同一分組（路徑不存在的同一個前綴）合成一條。
pub(super) fn build_notes(records: &[Record]) -> Vec<String> {
    enum Slot<'a> {
        Plain(&'a str),
        Group(&'a NoteGroup, Vec<String>),
    }
    let mut slots: Vec<Slot> = Vec::new();
    for record in records
        .iter()
        .filter(|record| record.kind == RecordKind::Rejected)
    {
        match &record.group {
            Some(group) => {
                let existing = slots.iter_mut().find_map(|slot| match slot {
                    Slot::Group(other, leaves) if other.key == group.key => Some(leaves),
                    _ => None,
                });
                if let Some(leaves) = existing {
                    leaves.push(group.leaf.clone());
                } else if slots.len() < NOTES_MAX {
                    slots.push(Slot::Group(group, vec![group.leaf.clone()]));
                }
            }
            None => {
                let seen = slots
                    .iter()
                    .any(|slot| matches!(slot, Slot::Plain(detail) if *detail == record.detail));
                if !seen && slots.len() < NOTES_MAX {
                    slots.push(Slot::Plain(&record.detail));
                }
            }
        }
    }
    let mut notes: Vec<String> = Vec::new();
    for slot in slots {
        let note = match slot {
            Slot::Plain(detail) => plain(detail.to_owned()),
            Slot::Group(group, leaves) => render_group(group, &leaves),
        };
        if !notes.contains(&note) {
            notes.push(note);
        }
    }
    notes
}

/// 分組回饋句：一筆就是原句；兩筆以上補「（本輪 N 筆未套用：鍵、鍵…）」，整條仍受上限。
pub(super) fn render_group(group: &NoteGroup, leaves: &[String]) -> String {
    let merged = leaves.len() >= 2;
    let suffix = if merged {
        // 鍵名去重（同一個新鍵寫了兩次），筆數照實
        let mut unique: Vec<String> = Vec::new();
        for leaf in leaves {
            if !unique.contains(leaf) {
                unique.push(leaf.clone());
            }
        }
        let names = fit_list(&unique, unique.len(), LEAVES_BUDGET);
        format!("（本輪 {} 筆未套用：{names}）", leaves.len())
    } else {
        String::new()
    };
    let (head, intro) = if merged {
        (&group.merged_head, &group.merged_intro)
    } else {
        (&group.head, &group.intro)
    };
    compose(
        head,
        intro,
        &group.items,
        group.max_items,
        &format!("{}{suffix}", group.tail),
    )
}

/// GM 要寫的 JSON Pointer：每段 `~`→`~0`、`/`→`~1`，照抄回來拆得回原鍵。
pub(super) fn pointer(path: &[String]) -> String {
    path.iter()
        .map(|segment| format!("/{}", segment.replace('~', "~0").replace('/', "~1")))
        .collect()
}

/// 提示裡顯示的路徑：超過 `PATH_MAX` 字就中段以「…」省略。
pub(super) fn shown_pointer(path: &[String]) -> String {
    clip_middle(&pointer(path), PATH_MAX)
}

fn clip_middle(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_owned();
    }
    let keep = max - 1;
    let head = keep / 2;
    let tail = keep - head;
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);
    out
}

fn clip_end(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// 鍵名用「、」接起來塞進 `budget` 字：超過 `max_items` 或塞不下就從尾端截，補「等 N 項」（N＝總數）。
fn fit_list(items: &[String], max_items: usize, budget: usize) -> String {
    let clipped: Vec<String> = items.iter().map(|item| clip_end(item, KEY_MAX)).collect();
    let total = clipped.len();
    for shown in (0..=total.min(max_items)).rev() {
        let mut text = clipped[..shown].join("、");
        if shown < total {
            if shown == 0 {
                text = format!("共 {total} 項");
            } else {
                text.push_str(&format!(" 等 {total} 項"));
            }
        }
        if text.chars().count() <= budget {
            return text;
        }
    }
    String::new()
}

/// `head` ＋（有列鍵才加的 `intro` ＋ 列鍵）＋ `tail`，列鍵只拿剩下的額度，整條不超過 `NOTE_MAX`。
pub(super) fn compose(
    head: &str,
    intro: &str,
    items: &[String],
    max_items: usize,
    tail: &str,
) -> String {
    let fixed = head.chars().count() + tail.chars().count();
    let mut out = head.to_owned();
    if !items.is_empty() {
        let budget = NOTE_MAX.saturating_sub(fixed + intro.chars().count());
        let list = fit_list(items, max_items, budget);
        if !list.is_empty() {
            out.push_str(intro);
            out.push_str(&list);
        }
    }
    out.push_str(tail);
    clip_end(&out, NOTE_MAX)
}

/// 單句（不列鍵）也走同一個上限。
pub(super) fn plain(text: String) -> String {
    clip_end(&text, NOTE_MAX)
}

/// 引用原文片段：壓成一行、截到 `max` 字。
pub(super) fn snippet(text: &str, max: usize) -> String {
    let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    clip_end(&one_line, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_escapes_tilde_and_slash() {
        let path = vec!["a/b".to_owned(), "c~d".to_owned()];
        assert_eq!(pointer(&path), "/a~1b/c~0d");
    }

    #[test]
    fn fit_list_cuts_from_the_end_and_counts_total() {
        let items: Vec<String> = (0..12).map(|i| format!("k{i}")).collect();
        assert_eq!(
            fit_list(&items, 8, 200),
            "k0、k1、k2、k3、k4、k5、k6、k7 等 12 項"
        );
        assert_eq!(fit_list(&items, 8, 12), "k0、k1 等 12 項");
    }

    #[test]
    fn compose_never_exceeds_the_cap() {
        let items: Vec<String> = (0..30).map(|i| "長".repeat(30) + &i.to_string()).collect();
        let note = compose(&"頭".repeat(150), "現有：", &items, 20, "。尾");
        assert!(note.chars().count() <= NOTE_MAX, "{}", note.chars().count());
        assert!(note.ends_with("。尾"));
    }
}
