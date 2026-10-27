//! 重構套用的機制部分：規則／觸發候選（路徑對齊、篩選、同欄多份規則的合併裁決），以及收編後開回報管道
//! （incremental＋把規則與觸發讀的欄位在樹上備好初值）。人物落檔不在這裡（apply.rs）。

use super::rule_merge::{min_uid, reconcile, RuleCandidate};
use super::types::{RefactorOutcome, RefactorSelection};
use crate::data::{self, DataResult, FieldKind, FieldRule, StateNode, WorldState};
use crate::mechanism;
use std::collections::{BTreeMap, BTreeSet};

/// 平欄裡由 ```state 圍欄回報的三個鍵：不當規則欄位搬進樹。接管桌不收圍欄，寫殼時整組清掉
/// （`flat_migration::clear_scene_keys`）。
pub(super) const BASE_TABLE_KEYS: [&str; 3] = ["time", "place", "present"];

/// 對齊用的命名空間：套用前的樹，加上開場狀態平欄裡 time／place／present 以外的鍵（當頂層葉子）。
/// 非 MVU、狀態區塊不是 YAML 的卡，開場狀態只在平欄（`驻守天数：第 1 天`），AI 照玩家語言自取的
/// `駐守天數` 要對回它。
pub(super) fn namespace(
    tree_before: &BTreeMap<String, StateNode>,
    table: &BTreeMap<String, String>,
) -> BTreeMap<String, StateNode> {
    let mut space = tree_before.clone();
    for (key, value) in table {
        if !BASE_TABLE_KEYS.contains(&key.as_str()) {
            space
                .entry(key.clone())
                .or_insert_with(|| StateNode::Leaf(value.clone()));
        }
    }
    space
}

/// 點分路徑對齊到命名空間裡的寫法：只差簡繁字形才改，樹上確實沒有的新路徑原樣留；
/// 同一層有多個字形候選（人物.沈郁／人物.沈鬱 對 人物.瀋鬱）分不出是哪個＝歧義，回錯整次拒套。
fn align_path(
    path: &str,
    namespace: &BTreeMap<String, StateNode>,
    fold_last: bool,
) -> DataResult<String> {
    if path.is_empty() {
        return Ok(String::new());
    }
    let ambiguous = |candidates: Vec<String>| {
        data::invalid_data(format!(
            "refactor path {path} is ambiguous among {}",
            candidates.join(" / ")
        ))
    };
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    let aligned = mechanism::canonical_path(namespace, &segments, fold_last).map_err(ambiguous)?;
    if fold_last && aligned.len() == 1 && !namespace.contains_key(&aligned[0]) {
        if let Some(key) = slash_head_match(namespace, &aligned[0]).map_err(ambiguous)? {
            return Ok(key);
        }
    }
    Ok(aligned.join("."))
}

/// 單段路徑對不上任何鍵時，找含「/」的頂層鍵：第一個「/」之前那段跟它精確或只差字形
/// （AI 規則寫 `劇情階段`、平欄鍵是 `剧情阶段/要塞事件`）。唯一回 Some，多個回 Err，沒有回 None。
pub(super) fn slash_head_match(
    namespace: &BTreeMap<String, StateNode>,
    segment: &str,
) -> Result<Option<String>, Vec<String>> {
    let matches: Vec<String> = namespace
        .keys()
        .filter(|key| {
            key.split_once('/')
                .is_some_and(|(head, _)| head == segment || data::glyph_equal(head, segment))
        })
        .cloned()
        .collect();
    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.into_iter().next()),
        _ => Err(matches),
    }
}

/// 對齊後的規則：對齊路徑 → 落到這裡的原始候選（原路徑, 規則），依原路徑字典序。同條目兩種字形
/// 對到同一路徑時兩份都留著，等 `Candidates::collect` 整欄收齊後一次裁決。
type AlignedRules = BTreeMap<String, Vec<(String, FieldRule)>>;

fn align_rules(
    rules: &BTreeMap<String, FieldRule>,
    namespace: &BTreeMap<String, StateNode>,
) -> DataResult<AlignedRules> {
    let mut grouped = AlignedRules::new();
    for (path, rule) in rules {
        grouped
            .entry(align_path(path, namespace, true)?)
            .or_default()
            .push((path.clone(), rule.clone()));
    }
    Ok(grouped)
}

fn align_triggers(
    triggers: &[data::Trigger],
    namespace: &BTreeMap<String, StateNode>,
) -> DataResult<Vec<data::Trigger>> {
    triggers
        .iter()
        .map(|trigger| align_trigger(trigger, namespace))
        .collect()
}

/// 觸發的條件路徑、scope、命中文本與前言裡的 `{{state:…}}` 佔位整條對齊（條件對齊了、佔位沒對齊，
/// 會命中卻插出空字串）；旗標是新鍵，最後一段不折（不能被當成拼錯的既有欄位）。
fn align_trigger(
    trigger: &data::Trigger,
    namespace: &BTreeMap<String, StateNode>,
) -> DataResult<data::Trigger> {
    let mut trigger = trigger.clone();
    let align_text = |text: &str| {
        mechanism::map_state_placeholders(text, |path| {
            align_path(path, namespace, true).map(|aligned| format!("{{{{state:{aligned}}}}}"))
        })
    };
    for case in &mut trigger.cases {
        for condition in &mut case.when {
            match condition {
                data::Condition::Range { path, .. } | data::Condition::Contains { path, .. } => {
                    *path = align_path(path, namespace, true)?;
                }
                data::Condition::Flag { path, .. } => {
                    *path = align_path(path, namespace, false)?;
                }
            }
        }
        case.text = align_text(&case.text)?;
    }
    trigger.preamble = align_text(&trigger.preamble)?;
    if let Some(flag) = &trigger.flag {
        trigger.flag = Some(align_path(flag, namespace, false)?);
    }
    if !trigger.scope.is_empty() {
        let joined = trigger.scope.join(".");
        trigger.scope = align_path(&joined, namespace, true)?
            .split('.')
            .map(str::to_owned)
            .collect();
    }
    Ok(trigger)
}

/// 篩選用的頂層鍵集合：套用前樹空就不篩（None）；非空時＝套用前頂層 ∪ 開場狀態平欄的鍵（對齊後的規則
/// 可能落在平欄搬過來的鍵）∪ 本次套用介面的頂層。
pub(super) fn filter_scope(
    tree: &BTreeMap<String, data::StateNode>,
    table: &BTreeMap<String, String>,
    interface_fields: Option<&serde_json::Value>,
) -> Option<BTreeSet<String>> {
    if tree.is_empty() {
        return None;
    }
    let mut scope: BTreeSet<String> = tree.keys().cloned().collect();
    scope.extend(
        table
            .keys()
            .filter(|key| !BASE_TABLE_KEYS.contains(&key.as_str()))
            .cloned(),
    );
    if let Some(object) = interface_fields.and_then(serde_json::Value::as_object) {
        scope.extend(object.keys().cloned());
    }
    Some(scope)
}

fn first_segment(path: &str) -> &str {
    path.split('.').next().unwrap_or_default()
}

/// 規則留不留：首段 `*`（合法萬用）一律留；否則首段要在集合裡。不篩時全留。
fn keep_rule(path: &str, scope: Option<&BTreeSet<String>>) -> bool {
    let head = first_segment(path);
    head == "*" || scope.is_none_or(|scope| scope.contains(head))
}

/// 觸發留不留：任一條件路徑或 `trigger.flag` 有一段是 `*` 就丟（求值不支援萬用，帶 default 的
/// range 會每回合用 default 求值，once 命中會長出字面 `*` 分支），樹空也丟；其餘 range（沒帶
/// default）／contains 首段要在集合裡，flag 條件與 `trigger.flag` 是新旗標、不比首段。
fn keep_trigger(trigger: &data::Trigger, scope: Option<&BTreeSet<String>>) -> bool {
    let wildcard = |path: &str| path.split('.').any(|segment| segment == "*");
    if trigger.flag.as_deref().is_some_and(wildcard) {
        return false; // once 命中會長出字面 `*` 分支
    }
    trigger
        .cases
        .iter()
        .flat_map(|case| case.when.iter())
        .all(|condition| {
            let (path, checked) = match condition {
                data::Condition::Range { path, default, .. } => (path, default.is_none()),
                data::Condition::Contains { path, .. } => (path, true),
                data::Condition::Flag { path, .. } => (path, false),
            };
            if wildcard(path) {
                return false;
            }
            !checked || scope.is_none_or(|scope| scope.contains(first_segment(path)))
        })
}

pub(super) type RuleSet = (BTreeMap<String, FieldRule>, Vec<data::Trigger>);

/// 本次要併入的規則／觸發（已篩），依產物 index 存；沒勾的不收。
pub(super) struct Candidates {
    pub(super) entries: BTreeMap<usize, RuleSet>,
    pub(super) mechanisms: BTreeMap<usize, RuleSet>,
}

impl Candidates {
    /// 先把路徑對齊到這桌既有的寫法（`namespace`：套用前的樹＋開場狀態平欄），再依 `scope` 篩；
    /// 對齊遇字形歧義＝整次拒套（呼叫端在任何寫入之前）。同一路徑的所有原始候選（跨條目、同條目
    /// 不同字形）整欄收齊後只合併或裁決一次（`rule_merge::reconcile`），結果寫回每一份，呼叫端依序
    /// extend 時誰後寫都一樣。
    pub(super) fn collect(
        outcome: &RefactorOutcome,
        selection: &RefactorSelection,
        scope: Option<BTreeSet<String>>,
        namespace: &BTreeMap<String, StateNode>,
    ) -> DataResult<Self> {
        let scope = scope.as_ref();
        let mut dropped = (0usize, 0usize);
        let mut filter = |rules: AlignedRules, triggers: Vec<data::Trigger>| {
            let total = rules.values().map(Vec::len).sum::<usize>();
            let kept_rules: AlignedRules = rules
                .into_iter()
                .filter(|(path, _)| keep_rule(path, scope))
                .collect();
            let kept_triggers: Vec<data::Trigger> = triggers
                .iter()
                .filter(|trigger| keep_trigger(trigger, scope))
                .cloned()
                .collect();
            dropped.0 += total - kept_rules.values().map(Vec::len).sum::<usize>();
            dropped.1 += triggers.len() - kept_triggers.len();
            (kept_rules, kept_triggers)
        };
        let mut aligned_entries = BTreeMap::new();
        for &index in &selection.entry_indices {
            let Some(entry) = outcome.entries.get(index) else {
                continue;
            };
            if entry.kind == "mechanism" {
                aligned_entries.insert(
                    index,
                    filter(
                        align_rules(&entry.rules, namespace)?,
                        align_triggers(&entry.triggers, namespace)?,
                    ),
                );
            }
        }
        let mut aligned_mechanisms = BTreeMap::new();
        for &index in &selection.mechanism_indices {
            let Some(mechanism) = outcome.mechanisms.get(index) else {
                continue;
            };
            aligned_mechanisms.insert(
                index,
                filter(
                    align_rules(&mechanism.rules, namespace)?,
                    align_triggers(&mechanism.triggers, namespace)?,
                ),
            );
        }
        if dropped != (0, 0) {
            log::warn!(
                "重構套用：{} 條規則、{} 組觸發對不到狀態樹，已略過",
                dropped.0,
                dropped.1
            );
        }
        // 整欄收齊：每份原始候選帶來源 uid，先後＝條目 index、機制 index、條目內原路徑字典序
        let mut grouped: BTreeMap<String, Vec<RuleCandidate>> = BTreeMap::new();
        let mut gather = |rules: &AlignedRules, uid: Option<u64>, origin: &str| {
            for (path, originals) in rules {
                for (original, rule) in originals {
                    grouped
                        .entry(path.clone())
                        .or_default()
                        .push(RuleCandidate {
                            rule: rule.clone(),
                            uid,
                            origin: format!("{origin} 的 {original}"),
                        });
                }
            }
        };
        for (index, (rules, _)) in &aligned_entries {
            let entry = &outcome.entries[*index];
            gather(
                rules,
                min_uid(&entry.source_uids),
                &format!("條目「{}」", entry.title),
            );
        }
        for (index, (rules, _)) in &aligned_mechanisms {
            let mechanism = &outcome.mechanisms[*index];
            gather(
                rules,
                min_uid([&mechanism.source_uid]),
                &format!("機制（來源 {}）", mechanism.source_uid),
            );
        }
        let resolved: BTreeMap<String, FieldRule> = grouped
            .into_iter()
            .map(|(path, candidates)| {
                let rule = reconcile(&path, &candidates);
                (path, rule)
            })
            .collect();
        let settle = |aligned: BTreeMap<usize, (AlignedRules, Vec<data::Trigger>)>| {
            aligned
                .into_iter()
                .map(|(index, (rules, triggers))| {
                    let rules = rules
                        .into_keys()
                        .map(|path| {
                            let rule = resolved[&path].clone();
                            (path, rule)
                        })
                        .collect();
                    (index, (rules, triggers))
                })
                .collect::<BTreeMap<usize, RuleSet>>()
        };
        let entries = settle(aligned_entries);
        let mechanisms = settle(aligned_mechanisms);
        Ok(Self {
            entries,
            mechanisms,
        })
    }
}

/// 收編了規則或觸發、這桌卻還沒開增量協定（非 MVU 卡的 characters 重構、介面沒套成的桌）：開協定讓
/// GM 有 `<UpdateVariable>` 可回報，並把規則與觸發條件讀的欄位在樹上備好初值。
/// - 樹上已有的葉子（含遊玩中的現值）一律不動。
/// - 樹上沒有、但開場狀態平欄有同名鍵：搬進樹（平欄那份刪掉，免得 GM 看到兩份）。數字型規則只認
///   明確格式取數字（純數字、`現值/上限`、`第 N 天` 一類，原寫法記成顯示模板）；取不到就照原文字搬、
///   規則改成文字欄。
/// - 平欄其餘 time／place／present 以外的鍵也全搬進樹並補文字規則（樹上已有同名的以樹為準）。
/// - 只有觸發在讀、沒有規則的欄位補一條規則（range→number、contains→text 每輪送），回合尾才列得出來。
///
/// 回傳從平欄搬進樹的鍵（呼叫端同步把事件快照裡的平欄也拿掉）。
pub(super) fn open_rule_reporting(
    state: &mut WorldState,
    rules: &BTreeMap<String, FieldRule>,
    triggers: &[data::Trigger],
) -> Vec<String> {
    let mut moved = Vec::new();
    if state.mechanism.incremental || (rules.is_empty() && triggers.is_empty()) {
        return moved;
    }
    state.mechanism.incremental = true;
    state.mechanism.incremental_by_refactor = true;

    let mut wanted: Vec<(String, Option<FieldKind>)> = rules
        .iter()
        .map(|(path, rule)| (path.clone(), Some(rule.kind)))
        .collect();
    for condition in triggers
        .iter()
        .flat_map(|trigger| trigger.cases.iter())
        .flat_map(|case| case.when.iter())
    {
        match condition {
            data::Condition::Range { path, .. } => wanted.push((path.clone(), None)),
            data::Condition::Contains { path, .. } => wanted.push((path.clone(), None)),
            data::Condition::Flag { .. } => {}
        }
    }
    for (path, kind) in wanted {
        let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
        if path.is_empty() || segments.iter().any(|segment| segment == "*") {
            continue;
        }
        let range_only = kind.is_none()
            && triggers.iter().flat_map(|t| t.cases.iter()).flat_map(|c| c.when.iter()).any(
                |condition| matches!(condition, data::Condition::Range { path: p, .. } if *p == path),
            );
        let numeric = match kind {
            Some(kind) => matches!(
                kind,
                FieldKind::Number | FieldKind::Counter | FieldKind::Pair
            ),
            None => range_only,
        };
        let mut as_text = false;
        if data::node_at(&state.state.tree, &segments).is_none() && segments.len() == 1 {
            if let Some(value) = state.state.table.remove(&path) {
                let initial = if numeric {
                    explicit_number(&value)
                } else {
                    Some((value.clone(), None))
                };
                // 數字型卻不是明確格式：照原文字搬、規則改文字——留平欄等於這欄永遠凍住
                let (initial, format) = initial.unwrap_or_else(|| {
                    as_text = numeric;
                    (value, None)
                });
                data::set_tree_value(&mut state.state.tree, &segments, &initial);
                if let Some(format) = format {
                    state.mechanism.display_formats.insert(path.clone(), format);
                }
                if as_text {
                    log::warn!("重構套用：{path} 的開場值不是明確數字格式，改以文字欄搬進狀態樹");
                }
                if as_text && kind.is_some() {
                    state
                        .mechanism
                        .rules
                        .insert(path.clone(), turn_rule(FieldKind::Text));
                }
                moved.push(path.clone());
            }
        }
        if kind.is_none()
            && data::node_at(&state.state.tree, &segments).is_some()
            && !mechanism::has_rule(&state.mechanism, &segments)
        {
            let kind = if numeric && !as_text {
                FieldKind::Number
            } else {
                FieldKind::Text
            };
            state.mechanism.rules.insert(path.clone(), turn_rule(kind));
        }
    }
    // 其餘開場平欄（沒有規則或觸發讀的）也全搬進樹、補文字規則：留在平欄 GM 看得到卻回報不了，
    // 用 insert 硬寫又會在樹上長出第二份。
    let rest: Vec<String> = state
        .state
        .table
        .keys()
        .filter(|key| !BASE_TABLE_KEYS.contains(&key.as_str()))
        .cloned()
        .collect();
    for key in rest {
        let Some(value) = state.state.table.remove(&key) else {
            continue;
        };
        let segments = vec![key.clone()];
        if data::node_at(&state.state.tree, &segments).is_none() {
            data::set_tree_value(&mut state.state.tree, &segments, &value);
        }
        if !mechanism::has_rule(&state.mechanism, &segments) {
            state
                .mechanism
                .rules
                .insert(key.clone(), turn_rule(FieldKind::Text));
        }
        moved.push(key);
    }
    moved
}

pub(super) fn turn_rule(kind: FieldKind) -> FieldRule {
    let mut rule = FieldRule::for_kind(kind);
    rule.inject = data::InjectLevel::Turn;
    rule
}

/// 明確格式才取數字：純數字照留、`3/5` 照留（現值/上限）、`第 7 天` 這類「第 N 單位」取 N，
/// 並回傳原寫法的顯示模板（`第 {} 天`，數字位置換成 `{}`）讓畫面照原格式顯示。
pub(super) fn explicit_number(value: &str) -> Option<(String, Option<String>)> {
    let trimmed = value.trim();
    if trimmed.parse::<f64>().is_ok_and(f64::is_finite) {
        return Some((trimmed.to_owned(), None));
    }
    if let Some((current, max)) = trimmed.split_once('/') {
        if current.trim().parse::<f64>().is_ok() && max.trim().parse::<f64>().is_ok() {
            return Some((trimmed.to_owned(), None));
        }
    }
    let inner = trimmed.strip_prefix('第')?;
    let unit_free = inner
        .trim_end_matches(['天', '日', '週', '周', '月', '年', '回', '章', '夜'])
        .trim();
    (unit_free.len() < inner.trim().len() && unit_free.parse::<u64>().is_ok()).then(|| {
        (
            unit_free.to_owned(),
            Some(trimmed.replacen(unit_free, "{}", 1)),
        )
    })
}
