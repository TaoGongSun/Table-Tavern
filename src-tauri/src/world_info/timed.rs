//! sticky／cooldown／delay：照網頁版 `TimedEffects`（ST `WorldInfoTimedEffects`）。單位是訊息則數。

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use super::entry::WiEntry;

fn is_false(flag: &bool) -> bool {
    !flag
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimedEffect {
    pub start: f64,
    pub end: f64,
    pub protected: bool,
    /// 桌面版：這條 sticky 被記下時是私密觸發（沿用到它生效的每一輪）；網頁版契約沒有這欄
    #[serde(default, skip_serializing_if = "is_false")]
    pub confidential: bool,
}

/// sticky／cooldown 各一張「條目 ID → 計時」；delay 每次由則數算，不存。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WiTimed {
    #[serde(default)]
    pub sticky: BTreeMap<String, TimedEffect>,
    #[serde(default)]
    pub cooldown: BTreeMap<String, TimedEffect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimedType {
    Sticky,
    Cooldown,
    Delay,
}

fn amount(entry: &WiEntry, kind: TimedType) -> Option<f64> {
    match kind {
        TimedType::Sticky => entry.sticky,
        TimedType::Cooldown => entry.cooldown,
        TimedType::Delay => entry.delay,
    }
}

/// `!entry[type]`：沒設、0、NaN 都算沒有計時。
fn has_timer(entry: &WiEntry, kind: TimedType) -> bool {
    amount(entry, kind).is_some_and(|value| value != 0.0 && !value.is_nan())
}

pub struct TimedEffects<'a> {
    chat_length: f64,
    entries: &'a [WiEntry],
    pub state: WiTimed,
    sticky: HashSet<String>,
    cooldown: HashSet<String>,
    delay: HashSet<String>,
}

impl<'a> TimedEffects<'a> {
    pub fn new(chat_length: usize, entries: &'a [WiEntry], state: WiTimed) -> Self {
        Self {
            chat_length: chat_length as f64,
            entries,
            state,
            sticky: HashSet::new(),
            cooldown: HashSet::new(),
            delay: HashSet::new(),
        }
    }

    fn effect(&self, kind: TimedType, entry: &WiEntry, protected: bool) -> TimedEffect {
        TimedEffect {
            start: self.chat_length,
            end: self.chat_length + amount(entry, kind).unwrap_or(f64::NAN),
            protected,
            confidential: false,
        }
    }

    fn table(&mut self, kind: TimedType) -> &mut BTreeMap<String, TimedEffect> {
        match kind {
            TimedType::Sticky => &mut self.state.sticky,
            _ => &mut self.state.cooldown,
        }
    }

    fn buffer(&mut self, kind: TimedType) -> &mut HashSet<String> {
        match kind {
            TimedType::Sticky => &mut self.sticky,
            TimedType::Cooldown => &mut self.cooldown,
            TimedType::Delay => &mut self.delay,
        }
    }

    /// 回傳到期（且條目還在）的條目 ID，給 sticky 接冷卻用。
    fn check_type(&mut self, kind: TimedType) -> Vec<String> {
        let mut ended = Vec::new();
        let snapshot: Vec<(String, TimedEffect)> = self
            .table(kind)
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        for (key, value) in snapshot {
            let entry = self.entries.iter().find(|candidate| candidate.id == key);
            if self.chat_length <= value.start && !value.protected {
                self.table(kind).remove(&key);
                continue;
            }
            let Some(entry) = entry else {
                if self.chat_length >= value.end {
                    self.table(kind).remove(&key);
                }
                continue;
            };
            if !has_timer(entry, kind) {
                self.table(kind).remove(&key);
                continue;
            }
            if self.chat_length >= value.end {
                self.table(kind).remove(&key);
                ended.push(key);
                continue;
            }
            self.buffer(kind).insert(key);
        }
        ended
    }

    pub fn check(&mut self) {
        // sticky 結束時若有 cooldown 就立刻開始冷卻（受保護：對話沒往前也不撤）
        for key in self.check_type(TimedType::Sticky) {
            let Some(entry) = self.entries.iter().find(|candidate| candidate.id == key) else {
                continue;
            };
            if !has_timer(entry, TimedType::Cooldown) {
                continue;
            }
            let effect = self.effect(TimedType::Cooldown, entry, true);
            self.state.cooldown.insert(key.clone(), effect);
            self.cooldown.insert(key);
        }
        self.check_type(TimedType::Cooldown);
        let chat_length = self.chat_length;
        let delayed: Vec<String> = self
            .entries
            .iter()
            .filter(|entry| has_timer(entry, TimedType::Delay))
            .filter(|entry| chat_length < entry.delay.unwrap_or(0.0))
            .map(|entry| entry.id.clone())
            .collect();
        self.delay.extend(delayed);
    }

    pub fn is_active(&self, kind: TimedType, id: &str) -> bool {
        match kind {
            TimedType::Sticky => self.sticky.contains(id),
            TimedType::Cooldown => self.cooldown.contains(id),
            TimedType::Delay => self.delay.contains(id),
        }
    }

    /// 生效中的 sticky 是否帶私密標記。
    pub fn sticky_confidential(&self, id: &str) -> bool {
        self.state
            .sticky
            .get(id)
            .is_some_and(|effect| effect.confidential)
    }

    /// 觸發的條目記計時（表裡還沒有才記）；`confidential` 只標在 sticky 上。
    pub fn set(&mut self, activated: &[(&WiEntry, bool)]) {
        for (entry, confidential) in activated {
            for kind in [TimedType::Sticky, TimedType::Cooldown] {
                if !has_timer(entry, kind) || self.table(kind).contains_key(&entry.id) {
                    continue;
                }
                let mut effect = self.effect(kind, entry, false);
                effect.confidential = kind == TimedType::Sticky && *confidential;
                self.table(kind).insert(entry.id.clone(), effect);
            }
        }
    }
}
