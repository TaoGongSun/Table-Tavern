//! 世界書段落的渲染（方案三之 3 放置表）：前／後兩組每條「### 標題\n內文」，作者註記與依深度條目攤平成
//! 注入段落接在後面，constant 人物條目只收進名冊行。段標由各組裝點自己給。標題與內文在
//! `world_scan::prepare` 已代換好，這裡不再代換。

use super::messages::scaffold_en;
use crate::world_scan::{arrange, Placed};
use std::collections::BTreeSet;

/// 名冊行（AI 卡重構包 4a）：constant 人物條目不進全文，只列名字。
pub(super) fn roster_line(names: &[&str], lang: &str) -> String {
    match scaffold_en(lang) {
        true => format!("Also at this table: {}", names.join(", ")),
        false => format!("這桌還有這些人：{}", names.join("、")),
    }
}

/// 一段世界書的本文（不含段標；沒有內容回空字串）。`entries` 依掃描給的放置前排序。
pub(super) fn section_body(entries: &[&Placed], lang: &str) -> String {
    let (roster, rest): (Vec<&Placed>, Vec<&Placed>) = entries
        .iter()
        .copied()
        .partition(|entry| entry.roster_only());
    let arranged = arrange(&rest);
    let mut body = String::new();
    for entry in arranged.front.iter().chain(arranged.back.iter()) {
        body.push_str(&format!("### {}\n{}\n", entry.title, entry.content));
    }
    // 注入段落沒有標題：與前面的條目、彼此之間各空一行，不黏成上一條的內文
    for injection in &arranged.injected {
        let text = crate::world_scan::trimmed(&injection.text);
        if !text.is_empty() {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(text);
            body.push('\n');
        }
    }
    // 名冊與條目同一個先後（排序的反向，order 小的在前）
    let names: Vec<&str> = roster
        .iter()
        .rev()
        .map(|entry| entry.title.as_str())
        .collect();
    if !names.is_empty() {
        body.push_str(&roster_line(&names, lang));
        body.push('\n');
    }
    body
}

/// 本段實際送出全文的條目（格式條目判定用）：前／後組與注入段落的成員；名冊行不算。outlet 條目只有在
/// `{{outlet::名稱}}` 真的被代入（`outlets_used`）時才算。
pub(super) fn full_text_members<'a>(
    entries: &[&'a Placed],
    outlets_used: &BTreeSet<String>,
) -> Vec<&'a Placed> {
    let rest: Vec<&Placed> = entries
        .iter()
        .copied()
        .filter(|entry| !entry.roster_only())
        .collect();
    let arranged = arrange(&rest);
    let mut members: Vec<&Placed> = arranged.front;
    members.extend(arranged.back);
    for injection in arranged.injected {
        members.extend(injection.members);
    }
    members.extend(rest.iter().copied().filter(|entry| {
        entry.position == crate::world_info::entry::position::OUTLET
            && !entry.content.is_empty()
            && outlets_used.contains(&entry.outlet_name)
    }));
    members
}
