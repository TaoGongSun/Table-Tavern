//! AI 卡重構本地組裝：小抄合約 v1（判官／survey）定案後，carry 整條照搬／drop 淘汰／split
//! 逐段路由／clean 人物這幾類不必再問 AI 的部分，App 本地零呼叫組裝，並跑四項機械稽核把關
//! 涵蓋與守恆。absorb 條目與 statusbar／group 段的 AI 呼叫是下一包（包 3）的事，這裡不碰——
//! 那些 span 在這裡只算「已有下落」，不產出任何本地內容。
//!
//! 四項稽核（RefactorAuditItem.kind）：
//! - "coverage"：世界書條目沒出現在 PERSONS／INTERFACE／ENTRIES 任何一處，自動補照搬。
//! - "mechanism"：結構預掃訊號落在沒附 reason 的照搬條目，可能漏接了機制。
//! - "split"：split 條目有段落沒被任何有效路由接住，自動併入「（餘段）」照搬條目。
//! - "drop_rule"：淘汰缺編號或編號不在 1–4，自動退回照搬。

use crate::data::{self, DataResult, WorldbookEntry};
use crate::refactor::RefactorCharacter;
use crate::refactor_ai::{
    self, EntrySpan, RefactorEntryMeta, RefactorEntryVerdict, RefactorNewEntry, RefactorSpanRoute,
    RefactorSurveyOutcome,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

/// 整條淘汰（ENTRIES action=drop，span=""）或半條淘汰（SPLITS route=drop，span="uid#sN"）的
/// 內容快照，供玩家展開查看、一鍵放回（轉 carry 進 entries）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorDroppedEntry {
    pub uid: String,
    pub span: String,
    pub title: String,
    pub content: String,
    pub rule: u8,
}

/// app 尚無執行機構的機制段：原文已經照搬進對應的 GM 規則條目（資料不會遺失），這裡只是給
/// 玩家看「有哪些機制還沒被系統接管」的清單。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorUnabsorbedItem {
    pub uid: String,
    pub span: String,
    pub title: String,
    pub note: String,
}

/// 機械稽核紅字：kind 見本檔開頭說明；span 空字串代表整條層級的稽核項（沒有特定段落）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorAuditItem {
    pub kind: String,
    pub uid: String,
    pub span: String,
    pub detail: String,
}

/// 本地零呼叫組裝的完整產物：entries／characters 由前端併入 RefactorOutcome 送 apply()；
/// clean_person_names 讓前端知道哪些人不必再排展開佇列；dropped／unabsorbed／audit 純資訊，
/// 玩家看得到、apply() 不處理。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorLocalAssembly {
    pub entries: Vec<RefactorNewEntry>,
    pub characters: Vec<RefactorCharacter>,
    pub clean_person_names: Vec<String>,
    pub dropped: Vec<RefactorDroppedEntry>,
    pub unabsorbed: Vec<RefactorUnabsorbedItem>,
    pub audit: Vec<RefactorAuditItem>,
}

/// 讀世界書後純本地組裝：不呼叫 AI、零延遲，含四項機械稽核。
pub fn assemble_local(
    root: &Path,
    world_id: &str,
    survey: &RefactorSurveyOutcome,
) -> DataResult<RefactorLocalAssembly> {
    let worldbook = data::read_worldbook(root, world_id)?;
    let by_uid: BTreeMap<u64, &WorldbookEntry> =
        worldbook.iter().map(|entry| (entry.uid, entry)).collect();

    let mut entries = Vec::new();
    let mut used_titles: HashSet<String> = HashSet::new();
    let mut dropped = Vec::new();
    let mut unabsorbed = Vec::new();
    let mut audit = Vec::new();

    assemble_verdicts(
        survey,
        &by_uid,
        &mut entries,
        &mut used_titles,
        &mut dropped,
        &mut audit,
    );
    assemble_splits(
        survey,
        &by_uid,
        &mut entries,
        &mut used_titles,
        &mut dropped,
        &mut unabsorbed,
        &mut audit,
    );
    let (characters, clean_person_names) = assemble_clean_persons(survey, &by_uid, &mut audit);
    assemble_coverage(
        survey,
        &worldbook,
        &mut entries,
        &mut used_titles,
        &mut audit,
    );
    audit_mechanism_conservation(survey, &worldbook, &by_uid, &mut unabsorbed, &mut audit);

    Ok(RefactorLocalAssembly {
        entries,
        characters,
        clean_person_names,
        dropped,
        unabsorbed,
        audit,
    })
}

pub(crate) fn build_meta(entry: &WorldbookEntry) -> RefactorEntryMeta {
    RefactorEntryMeta {
        keys: entry.keys.clone(),
        constant: entry.constant,
        order: entry.order,
        disabled: entry.disabled,
        visibility: entry.visibility.clone(),
        is_person: entry.is_person,
    }
}

/// 整條照搬：content byte 相等＋原條目元資料原樣保留（keys/constant/order/disabled/
/// visibility/is_person）。
fn carry_entry(entry: &WorldbookEntry) -> RefactorNewEntry {
    RefactorNewEntry {
        title: entry.title.clone(),
        kind: "setting".to_owned(),
        content: entry.content.clone(),
        source_uids: vec![entry.uid.to_string()],
        rules: BTreeMap::new(),
        triggers: Vec::new(),
        meta: Some(build_meta(entry)),
    }
}

fn push_carry(
    entry: &WorldbookEntry,
    entries: &mut Vec<RefactorNewEntry>,
    used_titles: &mut HashSet<String>,
) {
    used_titles.insert(entry.title.clone());
    entries.push(carry_entry(entry));
}

/// 解析 `uid#sN` 格式的段落引用；uid 或段號不是合法數字都回 None（garbage in 無聲跳過）。
fn parse_span_ref(text: &str) -> Option<(u64, usize)> {
    let (uid, span_part) = text.split_once("#s")?;
    Some((uid.parse().ok()?, span_part.parse().ok()?))
}

/// 解析段落引用並取得對應的來源條目與段落區間；uid 不存在、段號不合法或越界都回 None——
/// 小抄合約：「該路由無效視同未路由」。
pub(crate) fn resolve_span<'a>(
    by_uid: &BTreeMap<u64, &'a WorldbookEntry>,
    span_ref: &str,
) -> Option<(&'a WorldbookEntry, EntrySpan)> {
    let (uid, span_id) = parse_span_ref(span_ref)?;
    let entry = *by_uid.get(&uid)?;
    let span = refactor_ai::segment_spans(&entry.content)
        .into_iter()
        .find(|span| span.id == span_id)?;
    Some((entry, span))
}

/// ENTRIES 逐條：carry 整條照搬；drop 有效編號進淘汰清單，編號缺席或不在 1–4 自動退回照搬＋
/// audit。absorb／split 不在這裡處理（absorb 是包 3 的事；split 見 `assemble_splits`）。
fn assemble_verdicts(
    survey: &RefactorSurveyOutcome,
    by_uid: &BTreeMap<u64, &WorldbookEntry>,
    entries: &mut Vec<RefactorNewEntry>,
    used_titles: &mut HashSet<String>,
    dropped: &mut Vec<RefactorDroppedEntry>,
    audit: &mut Vec<RefactorAuditItem>,
) {
    for verdict in &survey.verdicts {
        let Ok(uid) = verdict.uid.parse::<u64>() else {
            continue;
        };
        let Some(&entry) = by_uid.get(&uid) else {
            continue;
        };
        match verdict.action.as_str() {
            "carry" => push_carry(entry, entries, used_titles),
            "drop" => match verdict.rule {
                Some(rule) if (1..=4).contains(&rule) => dropped.push(RefactorDroppedEntry {
                    uid: verdict.uid.clone(),
                    span: String::new(),
                    title: entry.title.clone(),
                    content: entry.content.clone(),
                    rule,
                }),
                _ => {
                    push_carry(entry, entries, used_titles);
                    audit.push(RefactorAuditItem {
                        kind: "drop_rule".to_owned(),
                        uid: verdict.uid.clone(),
                        span: String::new(),
                        detail: "淘汰缺編號或編號不在 1–4，自動退回照搬。".to_owned(),
                    });
                }
            },
            _ => {} // absorb／split 不在這裡處理
        }
    }
}

/// SPLITS 逐段路由：entry／gm／unabsorbed／drop／person／group／statusbar 七選一（小抄合約）。
/// 只處理 ENTRIES 判 split 的條目；每個 span 都必須落地，沒被任何有效路由接住的段落合成
/// 「<原標題>（餘段）」carry 型條目兜底（拆組守恆）。
fn assemble_splits(
    survey: &RefactorSurveyOutcome,
    by_uid: &BTreeMap<u64, &WorldbookEntry>,
    entries: &mut Vec<RefactorNewEntry>,
    used_titles: &mut HashSet<String>,
    dropped: &mut Vec<RefactorDroppedEntry>,
    unabsorbed: &mut Vec<RefactorUnabsorbedItem>,
    audit: &mut Vec<RefactorAuditItem>,
) {
    let split_uids: HashSet<u64> = survey
        .verdicts
        .iter()
        .filter(|verdict| verdict.action == "split")
        .filter_map(|verdict| verdict.uid.parse().ok())
        .collect();

    // uid -> 全部段落（僅 split 條目需要）。
    let spans_by_uid: BTreeMap<u64, Vec<EntrySpan>> = split_uids
        .iter()
        .filter_map(|&uid| {
            by_uid
                .get(&uid)
                .map(|entry| (uid, refactor_ai::segment_spans(&entry.content)))
        })
        .collect();

    let mut routed: HashSet<(u64, usize)> = HashSet::new();
    // route=entry：title -> 依 SPLITS 出現順序累積的 (來源 uid, 段文字)。
    let mut entry_groups: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    // route=gm／unabsorbed：來源 uid -> 段號 -> (是否 unabsorbed, 段文字, note)；BTreeMap 依
    // 段號自然排序＝依 span 序。
    let mut gm_groups: BTreeMap<u64, BTreeMap<usize, (bool, String, String)>> = BTreeMap::new();

    for route in &survey.splits {
        let Some((uid, span_id)) = parse_span_ref(&route.span) else {
            continue;
        };
        if !split_uids.contains(&uid) {
            continue;
        }
        let Some(spans) = spans_by_uid.get(&uid) else {
            continue;
        };
        let Some(span) = spans.iter().find(|span| span.id == span_id) else {
            continue; // 段號越界
        };
        if routed.contains(&(uid, span_id)) {
            continue; // 重複路由只認第一筆
        }
        let entry = by_uid[&uid];
        let text = entry.content[span.start..span.end].trim().to_owned();

        match route.route.as_str() {
            "statusbar" => {
                routed.insert((uid, span_id)); // 本包不組裝，算已有下落（包 3 的 AI 呼叫材料）
            }
            "gm" => {
                gm_groups
                    .entry(uid)
                    .or_default()
                    .insert(span_id, (false, text, String::new()));
                routed.insert((uid, span_id));
            }
            "unabsorbed" => {
                gm_groups
                    .entry(uid)
                    .or_default()
                    .insert(span_id, (true, text, route.note.clone()));
                routed.insert((uid, span_id));
            }
            "drop" => match route.rule {
                Some(rule) if (1..=4).contains(&rule) => {
                    dropped.push(RefactorDroppedEntry {
                        uid: uid.to_string(),
                        span: route.span.clone(),
                        title: entry.title.clone(),
                        content: text,
                        rule,
                    });
                    routed.insert((uid, span_id));
                }
                _ => audit.push(RefactorAuditItem {
                    kind: "drop_rule".to_owned(),
                    uid: uid.to_string(),
                    span: route.span.clone(),
                    detail: "淘汰缺編號或編號不在 1–4，此段改併入餘段照搬。".to_owned(),
                }),
            },
            "person" => {
                if survey
                    .persons
                    .iter()
                    .any(|person| person.name == route.name)
                {
                    routed.insert((uid, span_id)); // 併入內容在 assemble_clean_persons 另外處理
                }
            }
            "entry" => {
                entry_groups
                    .entry(route.title.clone())
                    .or_default()
                    .push((uid.to_string(), text));
                routed.insert((uid, span_id));
            }
            "group" => {
                if survey.groups.iter().any(|group| group.id == route.group) {
                    routed.insert((uid, span_id)); // 本包不組裝，算已有下落（包 3 的 AI 呼叫材料）
                }
            }
            _ => {} // 七個封閉字彙外的值 parse_split_line 早已濾掉
        }
    }

    // route=entry：同 title 跨條目依 SPLITS 出現順序串接。
    for (title, members) in entry_groups {
        let mut source_uids = Vec::new();
        for (uid, _) in &members {
            if !source_uids.contains(uid) {
                source_uids.push(uid.clone());
            }
        }
        let content = members
            .iter()
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        used_titles.insert(title.clone());
        entries.push(RefactorNewEntry {
            title,
            kind: "setting".to_owned(),
            content,
            source_uids,
            rules: BTreeMap::new(),
            triggers: Vec::new(),
            meta: None,
        });
    }

    // route=gm／unabsorbed：per 來源條目合成一條，依 span 序串接；unabsorbed 段同時記清單；
    // 標題跟本次其他產物撞名才後綴 " (GM)"。
    for (uid, spans_map) in gm_groups {
        let entry = by_uid[&uid];
        let mut parts = Vec::with_capacity(spans_map.len());
        for (span_id, (is_unabsorbed, text, note)) in &spans_map {
            parts.push(text.clone());
            if *is_unabsorbed {
                unabsorbed.push(RefactorUnabsorbedItem {
                    uid: uid.to_string(),
                    span: format!("{uid}#s{span_id}"),
                    title: entry.title.clone(),
                    note: note.clone(),
                });
            }
        }
        let title = if used_titles.contains(&entry.title) {
            format!("{} (GM)", entry.title)
        } else {
            entry.title.clone()
        };
        used_titles.insert(title.clone());
        entries.push(RefactorNewEntry {
            title,
            kind: "setting".to_owned(),
            content: parts.join("\n\n"),
            source_uids: vec![uid.to_string()],
            rules: BTreeMap::new(),
            triggers: Vec::new(),
            meta: None,
        });
    }

    // 拆組守恆：每個 split 條目的每一段都要有下落，沒被路由到的段合成「（餘段）」carry 型
    // 條目兜底（byte 相等由「slice 原文組裝」保證）。
    for (&uid, spans) in &spans_by_uid {
        let entry = by_uid[&uid];
        let mut leftovers = Vec::new();
        for span in spans {
            if routed.contains(&(uid, span.id)) {
                continue;
            }
            leftovers.push(entry.content[span.start..span.end].trim().to_owned());
            audit.push(RefactorAuditItem {
                kind: "split".to_owned(),
                uid: uid.to_string(),
                span: format!("{uid}#s{}", span.id),
                detail: "此段未獲有效路由，已併入「（餘段）」條目照搬。".to_owned(),
            });
        }
        if !leftovers.is_empty() {
            let title = format!("{}（餘段）", entry.title);
            used_titles.insert(title.clone());
            entries.push(RefactorNewEntry {
                title,
                kind: "setting".to_owned(),
                content: leftovers.join("\n\n"),
                source_uids: vec![uid.to_string()],
                rules: BTreeMap::new(),
                triggers: Vec::new(),
                meta: None,
            });
        }
    }
}

/// PERSONS mode=clean：spans 全部引用有效才組卡（原文依序串接，private 挑出私密段）；任一
/// 無效就不出卡，交回前端既有的展開佇列（mode≠clean 的人本來就不在這裡處理）。SPLITS
/// route=person 指到這個人的段（依 SPLITS 出現順序）追加到 public_md 尾。
fn assemble_clean_persons(
    survey: &RefactorSurveyOutcome,
    by_uid: &BTreeMap<u64, &WorldbookEntry>,
    audit: &mut Vec<RefactorAuditItem>,
) -> (Vec<RefactorCharacter>, Vec<String>) {
    let mut characters = Vec::new();
    let mut clean_person_names = Vec::new();

    for person in &survey.persons {
        if person.mode != "clean" || person.spans.is_empty() {
            continue;
        }
        let private_set: HashSet<&str> = person.private_spans.iter().map(String::as_str).collect();
        let mut public_parts = Vec::new();
        let mut private_parts = Vec::new();
        let mut invalid_span: Option<&str> = None;
        for span_ref in &person.spans {
            match resolve_span(by_uid, span_ref) {
                Some((entry, span)) => {
                    let text = entry.content[span.start..span.end].trim().to_owned();
                    if private_set.contains(span_ref.as_str()) {
                        private_parts.push(text);
                    } else {
                        public_parts.push(text);
                    }
                }
                None => {
                    invalid_span = Some(span_ref);
                    break;
                }
            }
        }
        let Some(invalid_span) = invalid_span else {
            // 全部 spans 引用有效：SPLITS route=person 指到他的段追加到 public_md 尾。
            for route in &survey.splits {
                if route.route == "person" && route.name == person.name {
                    if let Some((entry, span)) = resolve_span(by_uid, &route.span) {
                        public_parts.push(entry.content[span.start..span.end].trim().to_owned());
                    }
                }
            }
            let public_md = public_parts.join("\n\n");
            let private_md = private_parts.join("\n\n");
            let solo_entry_md = format!("{public_md}\n\n{private_md}");
            characters.push(RefactorCharacter {
                name: person.name.clone(),
                emoji: "🎭".to_owned(),
                public_md,
                private_md,
                source_uids: person.uids.clone(),
                solo_entry_md,
                suspected_player: person.is_player,
            });
            clean_person_names.push(person.name.clone());
            continue;
        };
        audit.push(RefactorAuditItem {
            kind: "split".to_owned(),
            uid: person.uids.first().cloned().unwrap_or_default(),
            span: invalid_span.to_owned(),
            detail: format!(
                "人物「{}」mode=clean 但段落引用無效，退回展開佇列。",
                person.name
            ),
        });
    }

    (characters, clean_person_names)
}

/// 涵蓋稽核：世界書每個 uid 必須出現在 persons／interface／verdicts 三處之一；漏網的自動補
/// carry（含 meta）＋audit。
fn assemble_coverage(
    survey: &RefactorSurveyOutcome,
    worldbook: &[WorldbookEntry],
    entries: &mut Vec<RefactorNewEntry>,
    used_titles: &mut HashSet<String>,
    audit: &mut Vec<RefactorAuditItem>,
) {
    let mut covered: HashSet<u64> = HashSet::new();
    for person in &survey.persons {
        // clean 模式只有 spans／private 實際引用的 uid 會進卡片產物；uids 欄多列的（判官
        // 敷衍亂塞）名義有下落、實際無產物，套用後原條殘留——不算 covered，讓漏網補 carry
        // 接住（2026-08-12 镇北王府實測洞）。tangled／未標 mode 是整條餵 AI，uids 全算。
        if person.mode == "clean" {
            for span_ref in person.spans.iter().chain(person.private_spans.iter()) {
                if let Some((uid, _)) = span_ref.split_once('#') {
                    if let Ok(uid) = uid.parse::<u64>() {
                        covered.insert(uid);
                    }
                }
            }
            continue;
        }
        for uid in &person.uids {
            if let Ok(uid) = uid.parse::<u64>() {
                covered.insert(uid);
            }
        }
    }
    for uid in &survey.interface_uids {
        if let Ok(uid) = uid.parse::<u64>() {
            covered.insert(uid);
        }
    }
    for verdict in &survey.verdicts {
        if let Ok(uid) = verdict.uid.parse::<u64>() {
            covered.insert(uid);
        }
    }

    for entry in worldbook {
        if covered.contains(&entry.uid) {
            continue;
        }
        push_carry(entry, entries, used_titles);
        audit.push(RefactorAuditItem {
            kind: "coverage".to_owned(),
            uid: entry.uid.to_string(),
            span: String::new(),
            detail: "此條目未出現在人物／介面／條目分類任何一處，自動補列照搬。".to_owned(),
        });
    }
}

/// 機制守恆稽核：重算結構預掃，每個訊號 span 必須落在 absorb／interface／persons／
/// statusbar／gm／group(kind=mechanism)／unabsorbed／合法 drop 之一才算 OK；否則若這個 uid
/// 判 carry（含涵蓋漏網自動補的隱性 carry）且沒附 reason，才紅字——split 路由到 entry／person
/// 等其餘去處視為判官已經考慮過，不重複稽核。
fn audit_mechanism_conservation(
    survey: &RefactorSurveyOutcome,
    worldbook: &[WorldbookEntry],
    by_uid: &BTreeMap<u64, &WorldbookEntry>,
    unabsorbed: &mut Vec<RefactorUnabsorbedItem>,
    audit: &mut Vec<RefactorAuditItem>,
) {
    let mut verdict_by_uid: BTreeMap<u64, &RefactorEntryVerdict> = BTreeMap::new();
    for verdict in &survey.verdicts {
        if let Ok(uid) = verdict.uid.parse::<u64>() {
            verdict_by_uid.insert(uid, verdict);
        }
    }
    let mut route_by_span: BTreeMap<(u64, usize), &RefactorSpanRoute> = BTreeMap::new();
    for route in &survey.splits {
        if let Some(key) = parse_span_ref(&route.span) {
            route_by_span.entry(key).or_insert(route);
        }
    }
    let group_kind_by_id: BTreeMap<&str, &str> = survey
        .groups
        .iter()
        .map(|group| (group.id.as_str(), group.kind.as_str()))
        .collect();
    let interface_uids: HashSet<&str> = survey.interface_uids.iter().map(String::as_str).collect();

    for signal in refactor_ai::prescan_worldbook(worldbook) {
        let Some((uid, span_id)) = parse_span_ref(&signal.span) else {
            continue;
        };
        let uid_str = uid.to_string();
        let verdict = verdict_by_uid.get(&uid).copied();

        let ok = verdict.is_some_and(|v| v.action == "absorb")
            || interface_uids.contains(uid_str.as_str())
            || survey
                .persons
                .iter()
                .any(|person| person.uids.iter().any(|u| u == &uid_str))
            || route_by_span
                .get(&(uid, span_id))
                .is_some_and(|route| match route.route.as_str() {
                    "statusbar" | "gm" | "unabsorbed" => true,
                    "drop" => route.rule.is_some_and(|rule| (1..=4).contains(&rule)),
                    "group" => {
                        group_kind_by_id.get(route.group.as_str()).copied() == Some("mechanism")
                    }
                    _ => false,
                })
            || verdict.is_some_and(|v| {
                v.action == "drop" && v.rule.is_some_and(|rule| (1..=4).contains(&rule))
            });
        if ok {
            continue;
        }

        // 剩下：uid 判 carry（含隱性——沒有 verdict 就是涵蓋稽核會自動補 carry，等同判 carry）。
        let is_effective_carry = verdict.map(|v| v.action == "carry").unwrap_or(true);
        let reason = verdict.map(|v| v.reason.as_str()).unwrap_or("");
        if !is_effective_carry {
            continue;
        }
        if reason.is_empty() {
            audit.push(RefactorAuditItem {
                kind: "mechanism".to_owned(),
                uid: uid_str.clone(),
                span: signal.span.clone(),
                detail: format!(
                    "結構預掃訊號（{}）落在照搬條目，未附 reason 說明。",
                    signal.pattern
                ),
            });
            unabsorbed.push(RefactorUnabsorbedItem {
                uid: uid_str,
                span: signal.span,
                title: by_uid
                    .get(&uid)
                    .map(|entry| entry.title.clone())
                    .unwrap_or_default(),
                note: "預掃訊號落在照搬條目".to_owned(),
            });
        } else {
            audit.push(RefactorAuditItem {
                kind: "excused".to_owned(),
                uid: uid_str,
                span: signal.span,
                detail: format!("照搬理由：{reason}"),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Visibility;
    use crate::refactor_ai::RefactorSurveyPerson;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(std::path::PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "table-tavern-refactor-assemble-{}-{}",
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

    fn seed(root: &Path, world_id: &str, title: &str, content: &str) -> u64 {
        data::upsert_worldbook_entry(
            root,
            world_id,
            WorldbookEntry {
                uid: u64::MAX,
                title: title.to_owned(),
                keys: Vec::new(),
                content: content.to_owned(),
                constant: false,
                order: 0,
                disabled: false,
                visibility: Visibility::Gm,
                is_person: false,
                locked: false,
            },
        )
        .unwrap()
    }

    fn empty_survey() -> RefactorSurveyOutcome {
        RefactorSurveyOutcome {
            persons: Vec::new(),
            interface_uids: Vec::new(),
            playable_interface_uids: Vec::new(),
            verdicts: Vec::new(),
            splits: Vec::new(),
            groups: Vec::new(),
            fields: Vec::new(),
            raw: String::new(),
        }
    }

    fn verdict(uid: u64, action: &str) -> RefactorEntryVerdict {
        RefactorEntryVerdict {
            uid: uid.to_string(),
            action: action.to_owned(),
            rule: None,
            reason: String::new(),
        }
    }

    // ---- carry：byte 相等＋meta 原樣 ----

    #[test]
    fn assemble_local_carry_preserves_content_and_meta() {
        let root = TestRoot::new();
        let world_id = data::create_world(&root.0, "測試").unwrap();
        let uid = seed(&root.0, &world_id, "設定條目", "第一段。\n\n第二段。");
        let mut entry = data::read_worldbook(&root.0, &world_id).unwrap().remove(0);
        entry.constant = true;
        entry.order = 42;
        entry.disabled = true;
        entry.keys = vec!["關鍵字".to_owned()];
        data::upsert_worldbook_entry(&root.0, &world_id, entry).unwrap();

        let survey = RefactorSurveyOutcome {
            verdicts: vec![verdict(uid, "carry")],
            ..empty_survey()
        };

        let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
        assert_eq!(assembly.entries.len(), 1);
        let produced = &assembly.entries[0];
        assert_eq!(produced.content, "第一段。\n\n第二段。");
        assert_eq!(produced.source_uids, vec![uid.to_string()]);
        let meta = produced.meta.as_ref().unwrap();
        assert!(meta.constant);
        assert_eq!(meta.order, 42);
        assert!(meta.disabled);
        assert_eq!(meta.keys, vec!["關鍵字".to_owned()]);
        assert!(assembly.audit.is_empty());
    }

    // ---- drop：有效編號進淘汰清單，缺編號退回照搬 ----

    #[test]
    fn assemble_local_drop_with_valid_rule_goes_to_dropped_list() {
        let root = TestRoot::new();
        let world_id = data::create_world(&root.0, "測試").unwrap();
        let uid = seed(&root.0, &world_id, "版本紀錄", "v1.2 更新內容");

        let survey = RefactorSurveyOutcome {
            verdicts: vec![RefactorEntryVerdict {
                uid: uid.to_string(),
                action: "drop".to_owned(),
                rule: Some(2),
                reason: String::new(),
            }],
            ..empty_survey()
        };
        let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
        assert!(assembly.entries.is_empty());
        assert_eq!(assembly.dropped.len(), 1);
        assert_eq!(assembly.dropped[0].content, "v1.2 更新內容");
        assert_eq!(assembly.dropped[0].rule, 2);
        assert!(assembly.audit.is_empty());
    }

    #[test]
    fn assemble_local_drop_without_valid_rule_falls_back_to_carry_with_audit() {
        let root = TestRoot::new();
        let world_id = data::create_world(&root.0, "測試").unwrap();
        let uid = seed(&root.0, &world_id, "版本紀錄", "v1.2 更新內容");

        let survey = RefactorSurveyOutcome {
            verdicts: vec![RefactorEntryVerdict {
                uid: uid.to_string(),
                action: "drop".to_owned(),
                rule: None, // 缺編號
                reason: String::new(),
            }],
            ..empty_survey()
        };
        let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
        assert!(assembly.dropped.is_empty());
        assert_eq!(assembly.entries.len(), 1);
        assert_eq!(assembly.entries[0].content, "v1.2 更新內容");
        assert_eq!(assembly.audit.len(), 1);
        assert_eq!(assembly.audit[0].kind, "drop_rule");
    }

    // ---- split 全路由組裝：entry 跨條目串接／gm+unabsorbed 合條／person 段併卡／餘段兜底 ----

    #[test]
    fn assemble_local_split_routes_entry_gm_person_and_leftover_together() {
        let root = TestRoot::new();
        let world_id = data::create_world(&root.0, "測試").unwrap();
        let uid_a = seed(
            &root.0,
            &world_id,
            "條目A",
            "共同段落甲。\n\nGM 段落。\n\n未接管機制段。\n\n沒人要的段。",
        );
        let uid_b = seed(&root.0, &world_id, "條目B", "共同段落乙。");
        let uid_c = seed(
            &root.0,
            &world_id,
            "霍玄設定",
            "霍玄的基本介紹。\n\n霍玄的秘密心事。",
        );
        let uid_d = seed(&root.0, &world_id, "霍玄補充", "霍玄額外的公開段落。");

        let survey = RefactorSurveyOutcome {
            persons: vec![RefactorSurveyPerson {
                name: "霍玄".to_owned(),
                uids: vec![uid_c.to_string()],
                is_player: false,
                mode: "clean".to_owned(),
                spans: vec![format!("{uid_c}#s1"), format!("{uid_c}#s2")],
                private_spans: vec![format!("{uid_c}#s2")],
            }],
            verdicts: vec![
                verdict(uid_a, "split"),
                verdict(uid_b, "split"),
                verdict(uid_d, "split"),
            ],
            splits: vec![
                RefactorSpanRoute {
                    span: format!("{uid_a}#s1"),
                    route: "entry".to_owned(),
                    rule: None,
                    name: String::new(),
                    title: "共同設定".to_owned(),
                    group: String::new(),
                    note: String::new(),
                },
                RefactorSpanRoute {
                    span: format!("{uid_b}#s1"),
                    route: "entry".to_owned(),
                    rule: None,
                    name: String::new(),
                    title: "共同設定".to_owned(),
                    group: String::new(),
                    note: String::new(),
                },
                RefactorSpanRoute {
                    span: format!("{uid_a}#s2"),
                    route: "gm".to_owned(),
                    rule: None,
                    name: String::new(),
                    title: String::new(),
                    group: String::new(),
                    note: String::new(),
                },
                RefactorSpanRoute {
                    span: format!("{uid_a}#s3"),
                    route: "unabsorbed".to_owned(),
                    rule: None,
                    name: String::new(),
                    title: String::new(),
                    group: String::new(),
                    note: "擲骰檢定".to_owned(),
                },
                RefactorSpanRoute {
                    span: format!("{uid_d}#s1"),
                    route: "person".to_owned(),
                    rule: None,
                    name: "霍玄".to_owned(),
                    title: String::new(),
                    group: String::new(),
                    note: String::new(),
                },
                // uid_a#s4 故意不路由：驗證餘段兜底。
            ],
            ..empty_survey()
        };

        let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();

        let merged = assembly
            .entries
            .iter()
            .find(|e| e.title == "共同設定")
            .unwrap();
        assert_eq!(merged.content, "共同段落甲。\n\n共同段落乙。");
        assert_eq!(
            merged.source_uids,
            vec![uid_a.to_string(), uid_b.to_string()]
        );

        let gm = assembly
            .entries
            .iter()
            .find(|e| e.title == "條目A")
            .unwrap();
        assert_eq!(gm.content, "GM 段落。\n\n未接管機制段。");
        assert_eq!(assembly.unabsorbed.len(), 1);
        assert_eq!(assembly.unabsorbed[0].span, format!("{uid_a}#s3"));
        assert_eq!(assembly.unabsorbed[0].note, "擲骰檢定");

        let leftover = assembly
            .entries
            .iter()
            .find(|e| e.title == "條目A（餘段）")
            .unwrap();
        assert_eq!(leftover.content, "沒人要的段。");
        assert!(leftover.meta.is_none());
        assert_eq!(assembly.audit.len(), 1);
        assert_eq!(assembly.audit[0].kind, "split");
        assert_eq!(assembly.audit[0].span, format!("{uid_a}#s4"));

        let character = assembly
            .characters
            .iter()
            .find(|c| c.name == "霍玄")
            .unwrap();
        assert_eq!(
            character.public_md,
            "霍玄的基本介紹。\n\n霍玄額外的公開段落。"
        );
        assert_eq!(character.private_md, "霍玄的秘密心事。");
        assert!(assembly.clean_person_names.contains(&"霍玄".to_owned()));
    }

    // ---- clean 人物：壞引用退回展開佇列 ----

    #[test]
    fn assemble_local_clean_person_with_invalid_span_is_skipped_with_audit() {
        let root = TestRoot::new();
        let world_id = data::create_world(&root.0, "測試").unwrap();
        let uid = seed(&root.0, &world_id, "阿蘭設定", "阿蘭的介紹。");

        let survey = RefactorSurveyOutcome {
            persons: vec![RefactorSurveyPerson {
                name: "阿蘭".to_owned(),
                uids: vec![uid.to_string()],
                is_player: false,
                mode: "clean".to_owned(),
                spans: vec![format!("{uid}#s1"), format!("{uid}#s9")], // s9 越界，無效引用
                private_spans: Vec::new(),
            }],
            ..empty_survey()
        };

        let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
        assert!(assembly.characters.is_empty());
        assert!(assembly.clean_person_names.is_empty());
        assert_eq!(assembly.audit.len(), 1);
        assert_eq!(assembly.audit[0].kind, "split");
    }

    // ---- 涵蓋：漏網 uid 自動 carry ----

    #[test]
    fn assemble_local_uncovered_uid_falls_back_to_carry_with_coverage_audit() {
        let root = TestRoot::new();
        let world_id = data::create_world(&root.0, "測試").unwrap();
        let uid = seed(&root.0, &world_id, "漏網條目", "沒被判官提到的內容。");
        // survey 完全沒提到這個 uid（不在 persons/interface/verdicts 任何一處）。
        let survey = empty_survey();

        let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
        assert_eq!(assembly.entries.len(), 1);
        assert_eq!(assembly.entries[0].content, "沒被判官提到的內容。");
        assert!(assembly.entries[0].meta.is_some());
        assert_eq!(assembly.audit.len(), 1);
        assert_eq!(assembly.audit[0].kind, "coverage");
        assert_eq!(assembly.audit[0].uid, uid.to_string());
    }

    // clean 人物的 uids 欄多列了 spans 沒引用的 uid（判官敷衍亂塞）：名義下落不算 covered，
    // 必須補 carry＋紅字（2026-08-12 镇北王府實測洞：兩條舊條無聲殘留）
    #[test]
    fn assemble_local_clean_person_extra_uid_without_span_reference_still_counts_uncovered() {
        let root = TestRoot::new();
        let world_id = data::create_world(&root.0, "測試").unwrap();
        let used_uid = seed(&root.0, &world_id, "人物條目", "霍玄的完整設定。");
        let stray_uid = seed(&root.0, &world_id, "美化状态栏", "| 体力 | 心情 |\n| 100 | 好 |");
        let mut survey = empty_survey();
        survey.persons = vec![RefactorSurveyPerson {
            name: "霍玄".to_owned(),
            uids: vec![used_uid.to_string(), stray_uid.to_string()],
            is_player: false,
            mode: "clean".to_owned(),
            spans: vec![format!("{used_uid}#s1")],
            private_spans: Vec::new(),
        }];

        let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
        // stray uid 被漏網稽核接住：補 carry＋coverage 紅字；used uid 是人物來源不補
        assert!(assembly.entries.iter().any(|e| e.title == "美化状态栏"));
        assert!(assembly
            .audit
            .iter()
            .any(|a| a.kind == "coverage" && a.uid == stray_uid.to_string()));
        assert!(!assembly.entries.iter().any(|e| e.title == "人物條目"));
    }

    // ---- 機制守恆：carry 無 reason 觸發 audit，有 reason 放行 ----

    #[test]
    fn assemble_local_mechanism_signal_needs_reason_to_pass_carry() {
        let root = TestRoot::new();
        let world_id = data::create_world(&root.0, "測試").unwrap();
        let flagged_uid = seed(
            &root.0,
            &world_id,
            "無說明條目",
            "trigger: 好感度達到 50 時告白",
        );
        let excused_uid = seed(
            &root.0,
            &world_id,
            "有說明條目",
            "trigger: 這只是歷史紀錄的關鍵字",
        );

        let survey = RefactorSurveyOutcome {
            verdicts: vec![
                verdict(flagged_uid, "carry"),
                RefactorEntryVerdict {
                    uid: excused_uid.to_string(),
                    action: "carry".to_owned(),
                    rule: None,
                    reason: "歷史紀錄，非即時機制".to_owned(),
                },
            ],
            ..empty_survey()
        };

        let assembly = assemble_local(&root.0, &world_id, &survey).unwrap();
        assert_eq!(assembly.entries.len(), 2); // 兩條都照搬
        let mechanism_audits: Vec<_> = assembly
            .audit
            .iter()
            .filter(|item| item.kind == "mechanism")
            .collect();
        assert_eq!(mechanism_audits.len(), 1);
        assert_eq!(mechanism_audits[0].uid, flagged_uid.to_string());
        // 附了 reason 的放行照搬要落一筆 excused，理由原文可見（調整階段檢查資料）。
        let excused_audits: Vec<_> = assembly
            .audit
            .iter()
            .filter(|item| item.kind == "excused")
            .collect();
        assert_eq!(excused_audits.len(), 1);
        assert_eq!(excused_audits[0].uid, excused_uid.to_string());
        assert_eq!(excused_audits[0].detail, "照搬理由：歷史紀錄，非即時機制");
        assert!(assembly
            .unabsorbed
            .iter()
            .any(|item| item.uid == flagged_uid.to_string()));
        assert!(!assembly
            .unabsorbed
            .iter()
            .any(|item| item.uid == excused_uid.to_string()));
    }
}
