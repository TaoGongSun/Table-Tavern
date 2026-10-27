//! 清單欄 YAML 鍵的字形校正（takeover-yaml-key-glyph）：接管桌的清單欄（例：「在場人物」整串 YAML）是一個
//! 字串葉子，鍵藏在值裡，路徑的字形折疊（`canonical_path`）管不到。模型被語言規範推著把 `行动` 寫成
//! `行動`，原卡腳本只認原字，人物卡片就少欄。這裡在寫入前把「只差字形」的鍵改回原卡寫法：
//!
//! - 只動明確規則判為清單欄（精確或萬用規則，不含依現值推定）的字串值；一般文字欄、物件、非 YAML 不動。
//! - 鍵的定位只認 `scan` 的有限子集，認不得整份保留原文；只換鍵那幾個字，冒號與值逐位元不動。
//! - 參考鍵按欄位、按結構位置（父鍵與序列項目，見 `scan::KeySite::parent`）取，同折疊的鍵在別的分支互不影響。依序：重構產物該欄的初始值 → guide 裡該欄的範本（只補前者沒有的折疊類）→ 這一批開始前
//!   的目前值。初始值與目前值只收出現兩次以上的鍵（一次的可能是人名這類動態鍵）；同一層裡同一折疊類有兩種寫法
//!   就整類移出、不再由後面的層補。參考在整批開始前就定好，前一筆更新不影響後一筆。
//! - 精確相符優先；折疊後對上唯一參考寫法才改；同一個映射裡改名會撞出重複鍵就不改。
//! - 成功的校正不回饋模型，寫入成功後記一筆 Ignored 進帳本。

mod scan;
#[cfg(test)]
mod tests;

use crate::data::{self, FieldKind, Mechanism, StateNode};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::rules::{has_rule, rule_for};
use super::types::{Patch, PatchOp, Record, RecordKind};

/// 參考鍵的來源（記帳用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Seed,
    Guide,
    Current,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Source::Seed => "初始值",
            Source::Guide => "欄位說明範本",
            Source::Current => "目前值",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Choice {
    Key(String, Source),
    /// 同一層同一折疊類有兩種寫法：不猜
    Conflict,
}

/// 一個鍵：結構位置（父鍵與序列項目記號）＋鍵本身，都是原字。
type Located = (Vec<String>, String);

/// 比對單位：折疊後的結構位置＋折疊後的鍵。父鍵本身換了字形也對得到同一個位置。
fn slot(parent: &[String], key: &str) -> (Vec<String>, String) {
    (
        parent
            .iter()
            .map(|segment| data::fold_glyphs(segment).into_owned())
            .collect(),
        data::fold_glyphs(key).into_owned(),
    )
}

/// 一個欄位的參考鍵：（位置, 折疊類）→ 原卡寫法。
#[derive(Debug, Default, Clone, PartialEq)]
struct References {
    classes: BTreeMap<(Vec<String>, String), Choice>,
}

impl References {
    fn add_layer(&mut self, keys: Vec<Located>, source: Source) {
        let mut layer: BTreeMap<(Vec<String>, String), BTreeSet<String>> = BTreeMap::new();
        for (parent, key) in keys {
            layer.entry(slot(&parent, &key)).or_default().insert(key);
        }
        for (class, spellings) in layer {
            if self.classes.contains_key(&class) {
                continue;
            }
            let choice = match spellings.len() {
                1 => Choice::Key(spellings.into_iter().next().unwrap_or_default(), source),
                _ => Choice::Conflict,
            };
            self.classes.insert(class, choice);
        }
    }
}

/// 一處改名：第幾行、舊鍵、新鍵、依據。
#[derive(Debug, Clone, PartialEq)]
struct Fix {
    line: usize,
    from: String,
    to: String,
    source: Source,
}

/// 這一批要寫的清單欄已改好鍵的 patch，與寫入成功後要記的帳。
pub(super) struct Prepared {
    pub patch: Patch,
    fixes: Vec<Fix>,
}

impl Prepared {
    /// 寫入成功後才記：欄位、行號、舊鍵→新鍵、依據。
    pub fn record(&self) -> Record {
        let items: Vec<String> = self
            .fixes
            .iter()
            .map(|fix| {
                format!(
                    "第 {} 行 {}→{}（依{}）",
                    fix.line,
                    fix.from,
                    fix.to,
                    fix.source.label()
                )
            })
            .collect();
        Record::new(
            RecordKind::Ignored,
            self.patch.path.join("."),
            format!("清單欄的鍵改回原卡寫法：{}", items.join("；")),
        )
    }
}

/// 整批開始前的參考快照：之後的寫入不影響參考。
pub(super) struct BatchKeys {
    before: Option<BTreeMap<String, StateNode>>,
}

impl BatchKeys {
    /// 這一批有 replace／insert 字串寫入才拍快照。
    pub fn new(
        tree: &BTreeMap<String, StateNode>,
        mechanism: &Mechanism,
        patches: &[&Patch],
    ) -> Self {
        // 這裡的路徑還沒對回樹上寫法（模型可能連頂層路徑都換了字形），不能用規則判；有字串寫入就拍
        let needed = !mechanism.rules.is_empty()
            && patches.iter().any(|patch| {
                matches!(patch.op, PatchOp::Replace | PatchOp::Insert)
                    && patch.value.as_ref().is_some_and(|value| value.is_string())
            });
        Self {
            before: needed.then(|| tree.clone()),
        }
    }

    /// 路徑已對回樹上寫法的 patch：是清單欄字串值而且有鍵要改回原卡寫法才回 Some。
    pub fn prepare(&self, mechanism: &Mechanism, patch: &Patch) -> Option<Prepared> {
        let value = list_string(mechanism, patch)?;
        let before = self.before.as_ref()?;
        let current = data::node_at(before, &patch.path).and_then(|node| match node {
            StateNode::Leaf(text) => Some(text.as_str()),
            StateNode::Branch(_) => None,
        });
        let references = references(mechanism, &patch.path, current);
        let (fixed, fixes) = normalize(value, &references)?;
        Some(Prepared {
            patch: Patch {
                value: Some(serde_json::Value::String(fixed)),
                ..patch.clone()
            },
            fixes,
        })
    }
}

/// replace／insert 的字串值，目標有明確的清單規則。
fn list_string<'a>(mechanism: &Mechanism, patch: &'a Patch) -> Option<&'a str> {
    if !matches!(patch.op, PatchOp::Replace | PatchOp::Insert) || !has_rule(mechanism, &patch.path)
    {
        return None;
    }
    if rule_for(mechanism, &patch.path, None).kind != FieldKind::List {
        return None;
    }
    patch.value.as_ref()?.as_str()
}

fn references(mechanism: &Mechanism, path: &[String], current: Option<&str>) -> References {
    let mut references = References::default();
    if let Some(seed) = seed_for(&mechanism.list_key_seeds, path) {
        if let Some(keys) = repeated_keys(seed) {
            references.add_layer(keys, Source::Seed);
        }
    }
    if let Some(keys) = guide_keys(&mechanism.guide, path, &known_fields(mechanism, path)) {
        references.add_layer(keys, Source::Guide);
    }
    if let Some(keys) = current.and_then(repeated_keys) {
        references.add_layer(keys, Source::Current);
    }
    references
}

/// 初始值找欄位：精確優先，否則逐段只差字形且唯一。
fn seed_for<'a>(seeds: &'a BTreeMap<Vec<String>, String>, path: &[String]) -> Option<&'a str> {
    if let Some(seed) = seeds.get(path) {
        return Some(seed);
    }
    let mut found = seeds.iter().filter(|(seed_path, _)| {
        seed_path.len() == path.len()
            && seed_path
                .iter()
                .zip(path)
                .all(|(a, b)| data::glyph_equal(a, b))
    });
    let (_, seed) = found.next()?;
    found.next().is_none().then_some(seed.as_str())
}

/// 同一個結構位置出現兩次以上的鍵（固定欄位會在每一筆重複；只出現一次的可能是人名這類動態鍵）。
fn repeated_keys(text: &str) -> Option<Vec<Located>> {
    let sites = scan::scan(text)?;
    let mut counts: BTreeMap<(Vec<String>, String), usize> = BTreeMap::new();
    for site in &sites {
        *counts
            .entry(slot(&site.parent, &text[site.start..site.end]))
            .or_default() += 1;
    }
    Some(
        sites
            .iter()
            .filter(|site| counts[&slot(&site.parent, &text[site.start..site.end])] >= 2)
            .map(|site| (site.parent.clone(), text[site.start..site.end].to_owned()))
            .collect(),
    )
}

/// 這桌已知的欄位路徑（規則、初始值、目標本身）：guide 只比末段名稱時，用來確認末段只對到一個欄位。
fn known_fields(mechanism: &Mechanism, path: &[String]) -> Vec<Vec<String>> {
    let mut fields: BTreeSet<Vec<String>> = mechanism
        .rules
        .keys()
        .map(|rule| rule.split('.').map(str::to_owned).collect())
        .collect();
    fields.extend(mechanism.list_key_seeds.keys().cloned());
    fields.insert(path.to_vec());
    fields.into_iter().collect()
}

/// guide 裡這個欄位的 YAML 範本的鍵。欄位條目是行首 `- 欄位：` 或 `- 欄位:`，範本是它底下（到下一個行首
/// 非空行為止）看起來像 YAML 的行。條目名稱是完整點分路徑的優先；沒有才比末段名稱，而且末段要只對到這桌
/// 一個欄位（`甲.人物`、`乙.人物` 並存就不用 `- 人物：`）。綁不到欄位、或同一欄位有多份不同的範本，就不猜。
fn guide_keys(guide: &str, path: &[String], fields: &[Vec<String>]) -> Option<Vec<Located>> {
    let dotted = path.join(".");
    let last = path.last()?;
    let bullets = guide_bullets(guide);
    let full: Vec<&(String, String)> = bullets
        .iter()
        .filter(|(label, _)| label == &dotted || data::glyph_equal(label, &dotted))
        .collect();
    let chosen = if !full.is_empty() {
        full
    } else {
        let same_last = fields
            .iter()
            .filter(|field| {
                field
                    .last()
                    .is_some_and(|name| data::glyph_equal(name, last))
            })
            .count();
        if same_last != 1 {
            return None;
        }
        bullets
            .iter()
            .filter(|(label, _)| label == last || data::glyph_equal(label, last))
            .collect()
    };
    let mut found: Option<Vec<Located>> = None;
    for (_, text) in chosen {
        let keys: Vec<Located> = scan::scan(text)?
            .into_iter()
            .map(|site| (site.parent, text[site.start..site.end].to_owned()))
            .collect();
        match &found {
            Some(previous) if *previous != keys => return None,
            _ => found = Some(keys),
        }
    }
    found.filter(|keys| !keys.is_empty())
}

/// guide 的欄位條目：（名稱, 底下看起來像 YAML 的範本行）。
fn guide_bullets(guide: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = guide.lines().collect();
    let mut bullets = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let Some(label) = bullet_label(lines[index]) else {
            index += 1;
            continue;
        };
        index += 1;
        let start = index;
        while index < lines.len()
            && (lines[index].trim().is_empty() || lines[index].starts_with(' '))
        {
            index += 1;
        }
        let template: Vec<&str> = lines[start..index]
            .iter()
            .copied()
            .filter(|line| looks_yaml(line))
            .collect();
        bullets.push((label.to_owned(), template.join("\n")));
    }
    bullets
}

/// 行首 `- 名稱：…`／`- 名稱:…` 的名稱。
fn bullet_label(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("- ")?;
    let end = rest.find(['：', ':'])?;
    let label = rest[..end].trim();
    (!label.is_empty()).then_some(label)
}

/// 範本行：縮排的序列項目或 `鍵:` 行；說明文字不算。
fn looks_yaml(line: &str) -> bool {
    let trimmed = line.trim_start_matches(' ');
    if trimmed.starts_with("- ") {
        return true;
    }
    match trimmed.find(':') {
        Some(position) => {
            let key = &trimmed[..position];
            !key.is_empty()
                && !key.contains(['"', '\'', ' ', '：'])
                && trimmed[position + 1..]
                    .chars()
                    .next()
                    .is_none_or(|ch| ch == ' ')
        }
        None => false,
    }
}

/// 把值裡只差字形的鍵改回參考寫法；沒有要改的回 None。
fn normalize(value: &str, references: &References) -> Option<(String, Vec<Fix>)> {
    let sites = scan::scan(value)?;
    let mut originals: BTreeMap<usize, BTreeSet<&str>> = BTreeMap::new();
    for site in &sites {
        originals
            .entry(site.mapping)
            .or_default()
            .insert(&value[site.start..site.end]);
    }
    let mut candidates: Vec<(usize, &str, Source)> = Vec::new();
    for (index, site) in sites.iter().enumerate() {
        let key = &value[site.start..site.end];
        let Some(Choice::Key(target, source)) = references.classes.get(&slot(&site.parent, key))
        else {
            continue;
        };
        if target != key {
            candidates.push((index, target.as_str(), *source));
        }
    }
    // 同一個映射裡：改成已存在的鍵、或兩個鍵改成同一個，都會撞出重複鍵，不改
    let mut targets: BTreeMap<(usize, &str), usize> = BTreeMap::new();
    for (index, target, _) in &candidates {
        *targets.entry((sites[*index].mapping, *target)).or_default() += 1;
    }
    candidates.retain(|(index, target, _)| {
        let mapping = sites[*index].mapping;
        targets[&(mapping, *target)] == 1 && !originals[&mapping].contains(target)
    });
    if candidates.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(value.len());
    let mut cursor = 0;
    let mut fixes = Vec::new();
    for (index, target, source) in candidates {
        let site = &sites[index];
        out.push_str(&value[cursor..site.start]);
        out.push_str(target);
        cursor = site.end;
        fixes.push(Fix {
            line: site.line,
            from: value[site.start..site.end].to_owned(),
            to: target.to_owned(),
            source,
        });
    }
    out.push_str(&value[cursor..]);
    Some((out, fixes))
}

/// 載入層：重構產物（refactor-outcome.json）介面的初始值，路徑 → 字串值。讀不到、格式不對都回空。
pub fn resolve_list_key_seeds(root: &Path, world_id: &str) -> BTreeMap<Vec<String>, String> {
    let mut seeds = BTreeMap::new();
    let Ok(Some(text)) = data::read_refactor_outcome(root, world_id) else {
        return seeds;
    };
    let Ok(card) = serde_json::from_str::<serde_json::Value>(&text) else {
        return seeds;
    };
    let fields = card
        .pointer("/outcome/interface/state_fields")
        .or_else(|| card.pointer("/interface/state_fields"));
    if let Some(fields) = fields {
        collect_strings(fields, &mut Vec::new(), &mut seeds);
    }
    seeds
}

fn collect_strings(
    value: &serde_json::Value,
    path: &mut Vec<String>,
    out: &mut BTreeMap<Vec<String>, String>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                path.push(key.clone());
                collect_strings(child, path, out);
                path.pop();
            }
        }
        serde_json::Value::String(text) if !path.is_empty() => {
            out.insert(path.clone(), text.clone());
        }
        _ => {}
    }
}
