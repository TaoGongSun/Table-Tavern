//! 重構套用的來源核對與新條目形狀。
//!
//! 重構卡可以跨桌套用：產物引用的來源 uid 在新桌可能不存在，或指向無關的條目。套用開頭一次算出
//! 「已核對來源表」（[`Sources`]），刪除、停用、帳本紀錄、uid 歸屬與新條目的可見度／觸發都只查這張表。

use super::types::RefactorOutcome;
use crate::data::{self, Visibility, WorldbookEntry};
use std::collections::{BTreeMap, BTreeSet};

/// 已核對來源表：產物裡的 uid 字串 → 這桌實際對上的條目 uid。
pub(super) struct Sources {
    resolved: BTreeMap<String, u64>,
    entries: BTreeMap<u64, WorldbookEntry>,
    raw: BTreeMap<u64, serde_json::Value>,
}

impl Sources {
    /// `entries`／`raw` 是套用前的快照（套用中途會刪來源，之後讀不到）。
    ///
    /// 產物帶 `source_fingerprints` 時逐條核對：先照 UID、身分指紋相符才算；不符或找不到再用指紋在快照
    /// 反查，恰好一條才對上。沒有指紋的 uid、零條或多條相符、同一條實際條目被多個產物 uid 對到，都算
    /// 核對失敗。產物沒有 `source_fingerprints`（舊產物）時只照 UID 對。
    pub(super) fn build(
        outcome: &RefactorOutcome,
        entries: &[WorldbookEntry],
        raw: BTreeMap<u64, serde_json::Value>,
    ) -> Self {
        let entries: BTreeMap<u64, WorldbookEntry> = entries
            .iter()
            .map(|entry| (entry.uid, entry.clone()))
            .collect();
        let fingerprints: BTreeMap<u64, String> = raw
            .iter()
            .map(|(uid, value)| (*uid, data::identity_fingerprint(value)))
            .collect();
        let mut resolved: BTreeMap<String, u64> = BTreeMap::new();
        for uid in referenced_uids(outcome) {
            let parsed = uid.parse::<u64>().ok();
            let target = if outcome.source_fingerprints.is_empty() {
                parsed.filter(|parsed| entries.contains_key(parsed))
            } else {
                outcome.source_fingerprints.get(&uid).and_then(|expected| {
                    parsed
                        .filter(|parsed| fingerprints.get(parsed) == Some(expected))
                        .or_else(|| {
                            let mut matches = fingerprints
                                .iter()
                                .filter(|(_, fingerprint)| *fingerprint == expected)
                                .map(|(uid, _)| *uid);
                            match (matches.next(), matches.next()) {
                                (Some(only), None) => Some(only),
                                _ => None,
                            }
                        })
                })
            };
            if let Some(target) = target {
                resolved.insert(uid, target);
            }
        }
        // 一對一：同一條實際條目被多個產物 uid 對到，這些 uid 全部算核對失敗
        let mut hits: BTreeMap<u64, usize> = BTreeMap::new();
        for target in resolved.values() {
            *hits.entry(*target).or_default() += 1;
        }
        resolved.retain(|_, target| hits.get(target) == Some(&1));
        Self {
            resolved,
            entries,
            raw,
        }
    }

    /// 產物 uid 對上的實際 uid；核對失敗是 None（視同這桌沒有這條來源）。
    pub(super) fn resolve(&self, uid: &str) -> Option<u64> {
        self.resolved.get(uid).copied()
    }

    pub(super) fn entry(&self, uid: &str) -> Option<&WorldbookEntry> {
        self.entries.get(&self.resolve(uid)?)
    }

    pub(super) fn raw(&self, uid: &str) -> Option<&serde_json::Value> {
        self.raw.get(&self.resolve(uid)?)
    }
}

/// 產物引用到的所有來源 uid（角色、條目、介面、機制、淘汰、保留、可刪共用合集）。
pub(super) fn referenced_uids(outcome: &RefactorOutcome) -> BTreeSet<String> {
    let mut uids = BTreeSet::new();
    for character in &outcome.characters {
        uids.extend(character.source_uids.iter().cloned());
    }
    for entry in &outcome.entries {
        uids.extend(entry.source_uids.iter().cloned());
    }
    if let Some(interface) = &outcome.interface {
        uids.extend(interface.source_uids.iter().cloned());
    }
    for mechanism in &outcome.mechanisms {
        uids.insert(mechanism.source_uid.clone());
    }
    for dropped in &outcome.dropped {
        uids.insert(dropped.uid.clone());
    }
    uids.extend(outcome.preserve_source_uids.iter().cloned());
    uids.extend(outcome.deletable_shared_uids.iter().cloned());
    uids
}

/// 沒帶 meta 的 setting 新條目的形狀：觸發、可見度、來源卡都從核對成功的來源推。
#[derive(Debug, Clone, PartialEq)]
pub(super) struct NewEntryShape {
    pub keys: Vec<String>,
    pub constant: bool,
    pub disabled: bool,
    pub visibility: Visibility,
    pub source_cards: Vec<String>,
}

/// 觸發政策：只算啟用的來源——主鍵取聯集（去重、保留出現順序），任一啟用來源常駐就常駐；來源全部停用
/// 才停用（此時改用全部來源算主鍵與常駐，維持玩家啟用它時的行為）。「任一啟用的來源會被送出時它就
/// 送出」最接近原卡。可見度：來源全是同一份 `Characters` 名單就沿用，其餘給 GM。來源全部核對失敗
/// （或根本沒有）時給 GM 並常駐，內容至少讓 GM 看得到。
pub(super) fn new_entry_shape(sources: &[(&WorldbookEntry, &serde_json::Value)]) -> NewEntryShape {
    if sources.is_empty() {
        return NewEntryShape {
            keys: Vec::new(),
            constant: true,
            disabled: false,
            visibility: Visibility::Gm,
            source_cards: Vec::new(),
        };
    }
    let enabled: Vec<&WorldbookEntry> = sources
        .iter()
        .map(|(entry, _)| *entry)
        .filter(|entry| !entry.disabled)
        .collect();
    let disabled = enabled.is_empty();
    let basis: Vec<&WorldbookEntry> = if disabled {
        sources.iter().map(|(entry, _)| *entry).collect()
    } else {
        enabled
    };
    let mut keys: Vec<String> = Vec::new();
    for key in basis.iter().flat_map(|entry| entry.keys.iter()) {
        if !keys.contains(key) {
            keys.push(key.clone());
        }
    }
    let constant = basis.iter().any(|entry| entry.constant);
    let name_set = |entry: &WorldbookEntry| match &entry.visibility {
        Visibility::Characters(ids) => Some(ids.iter().cloned().collect::<BTreeSet<String>>()),
        _ => None,
    };
    let first = name_set(sources[0].0);
    let visibility = match &first {
        Some(set)
            if sources
                .iter()
                .all(|(entry, _)| name_set(entry).as_ref() == Some(set)) =>
        {
            sources[0].0.visibility.clone()
        }
        _ => Visibility::Gm,
    };
    let mut source_cards: Vec<String> = Vec::new();
    for card in sources
        .iter()
        .flat_map(|(_, raw)| data::source_cards_of(raw))
    {
        if !source_cards.contains(&card) {
            source_cards.push(card);
        }
    }
    NewEntryShape {
        keys,
        constant,
        disabled,
        visibility,
        source_cards,
    }
}
