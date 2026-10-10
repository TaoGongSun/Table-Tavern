//! `check_world_info`：逐行照網頁版 `world-info-scan.ts`（ST 06bde939 `checkWorldInfo`、`WorldInfoBuffer`、
//! `filterByInclusionGroups`）。網頁版沒有的只有一件事：每條觸發條目的「觸發來源」（公開／私密），
//! 在實際掃描的每一輪就地判定，不另外重跑；觸發集合本身與網頁版逐條相同（見方案三之 3）。

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};

use fancy_regex::Regex;
use serde::Serialize;

use super::entry::{logic, position, WiEntry};
use super::js_semantics::{is_js_whitespace, is_js_word, js_key_order, js_round, js_trim};
use super::regex_key::{parse_regex_from_string, regex_outcome, RegexOutcome};
use super::settings::WiSettings;
use super::sort::{compare_order, stable_sort};
use super::timed::{TimedEffects, TimedType, WiTimed};

const MAX_SCAN_DEPTH: f64 = 1000.0;
const MATCHER: &str = "\u{1}";
const JOINER: &str = "\n\u{1}";

/// 一段全域掃描文字：`full` 是網頁版掃的那段；`public` 是其中公開的部分（判觸發來源用，對拍時兩者相同）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScanField {
    pub full: String,
    pub public: String,
}

impl ScanField {
    #[cfg(test)]
    pub fn public(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            full: text.clone(),
            public: text,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct GlobalScan {
    pub persona_description: ScanField,
    pub character_description: ScanField,
    pub character_personality: ScanField,
    pub character_depth_prompt: ScanField,
    pub scenario: ScanField,
    pub creator_notes: ScanField,
}

/// 代換結果；`private`＝代換讀到了私密來源（巨集引擎回報；掃描本身只負責傳遞）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Substituted {
    pub text: String,
    pub private: bool,
}

/// 掃描需要的外部函式：巨集代換、計數、亂數。
pub trait ScanHooks {
    fn substitute(&mut self, text: &str) -> Substituted;
    fn count_tokens(&mut self, text: &str) -> f64;
    fn random(&mut self) -> f64;
}

pub struct ScanInput<'a> {
    /// 要掃的訊息，新到舊
    pub chat: &'a [String],
    /// 這次的提示預算（上下文上限－保留輸出）；沒有上限是 `f64::INFINITY`
    pub max_context: f64,
    pub global_scan: &'a GlobalScan,
    /// 生成類型（桌面版一律 `normal`）
    pub trigger: &'a str,
    pub timed: WiTimed,
    pub settings: &'a WiSettings,
    /// 桌面版：角色共線共用快照的靜態條目（方案三之 3）。預算溢出時照樣留在觸發結果裡（照網頁版設
    /// `overflowed`、停掉後面的條目與遞迴），不當 `ignoreBudget`；對拍案例傳空集合。
    pub pinned: &'a BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Example {
    pub position: &'static str,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DepthGroup {
    pub depth: f64,
    pub role: f64,
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WiResult {
    pub before: String,
    pub after: String,
    pub examples: Vec<Example>,
    pub depth: Vec<DepthGroup>,
    pub an_top: Vec<String>,
    pub an_bottom: Vec<String>,
    /// outlet 名稱 → 內容，依 JS 物件鍵順序（陣列索引形的名稱在前，其餘照第一次出現）
    #[serde(serialize_with = "ordered_pairs")]
    pub outlets: Vec<(String, Vec<String>)>,
    /// 掃完之後的計時狀態
    pub timed: WiTimed,
    /// 觸發的條目 ID（依加入順序）
    pub activated: Vec<String>,
    /// 桌面版：私密觸發的條目（觸發要靠私密片段才成立）
    #[serde(skip)]
    pub private_ids: BTreeSet<String>,
    /// 桌面版：內文代換時讀到私密來源的條目
    #[serde(skip)]
    pub private_content: BTreeSet<String>,
    /// 桌面版：觸發條目依放置前的排序（`compare_order` 穩定排序）列出 (ID, 代換後內文)，內文空的也列；
    /// 各位置分組的先後由這份順序推得（與上面的字串欄位同一套規則）
    #[serde(skip)]
    pub placed: Vec<(String, String)>,
}

impl WiResult {
    fn empty(timed: WiTimed) -> Self {
        Self {
            before: String::new(),
            after: String::new(),
            examples: Vec::new(),
            depth: Vec::new(),
            an_top: Vec::new(),
            an_bottom: Vec::new(),
            outlets: Vec::new(),
            timed,
            activated: Vec::new(),
            private_ids: BTreeSet::new(),
            private_content: BTreeSet::new(),
            placed: Vec::new(),
        }
    }
}

/// outlets 依序寫成 `[[名稱, [內容…]], …]`（對拍要比順序，JSON 物件在 Rust 讀回來會丟掉順序）。
fn ordered_pairs<S: serde::Serializer>(
    pairs: &[(String, Vec<String>)],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(pairs.iter().map(|(name, values)| (name, values)))
}

struct Buffer<'a> {
    depth: Vec<Option<String>>,
    global: &'a GlobalScan,
    settings: &'a WiSettings,
    recurse: Vec<String>,
    /// 遞迴緩衝裡公開的部分（每輪只收公開觸發、內文也公開的條目）
    recurse_public: Vec<String>,
    regexes: RefCell<HashMap<String, Option<Regex>>>,
    /// 這次掃描裡撞過回溯上限的正則鍵：之後直接算不中（避免惡意卡讓後端一再卡住）
    exceeded: RefCell<HashSet<String>>,
}

impl<'a> Buffer<'a> {
    fn new(messages: &[String], global: &'a GlobalScan, settings: &'a WiSettings) -> Self {
        let mut depth: Vec<Option<String>> = messages
            .iter()
            .take(MAX_SCAN_DEPTH as usize)
            .map(|message| (!message.is_empty()).then(|| js_trim(message).to_owned()))
            .collect();
        // JS 稀疏陣列的長度到最後一個有值的位置為止
        while depth.last().is_some_and(Option::is_none) {
            depth.pop();
        }
        Self {
            depth,
            global,
            settings,
            recurse: Vec::new(),
            recurse_public: Vec::new(),
            regexes: RefCell::new(HashMap::new()),
            exceeded: RefCell::new(HashSet::new()),
        }
    }

    fn text(&self, entry: &WiEntry, public: bool) -> String {
        let mut depth = entry.scan_depth.unwrap_or(self.settings.depth);
        if depth <= 0.0 {
            return String::new();
        }
        if depth > MAX_SCAN_DEPTH {
            depth = MAX_SCAN_DEPTH;
        }
        let count = (depth.trunc() as usize).min(self.depth.len());
        let mut result = MATCHER.to_owned();
        result.push_str(
            &self.depth[..count]
                .iter()
                .map(|item| item.as_deref().unwrap_or(""))
                .collect::<Vec<_>>()
                .join(JOINER),
        );
        let global = self.global;
        let extra = [
            (entry.match_persona_description, &global.persona_description),
            (
                entry.match_character_description,
                &global.character_description,
            ),
            (
                entry.match_character_personality,
                &global.character_personality,
            ),
            (
                entry.match_character_depth_prompt,
                &global.character_depth_prompt,
            ),
            (entry.match_scenario, &global.scenario),
            (entry.match_creator_notes, &global.creator_notes),
        ];
        for (enabled, field) in extra {
            let text = if public { &field.public } else { &field.full };
            if enabled && !text.is_empty() {
                result.push_str(JOINER);
                result.push_str(text);
            }
        }
        let recurse = if public {
            &self.recurse_public
        } else {
            &self.recurse
        };
        if !recurse.is_empty() {
            result.push_str(JOINER);
            result.push_str(&recurse.join(JOINER));
        }
        result
    }

    fn match_keys(&self, haystack: &str, needle: &str, entry: &WiEntry) -> bool {
        let regex = self
            .regexes
            .borrow_mut()
            .entry(needle.to_owned())
            .or_insert_with(|| parse_regex_from_string(needle))
            .clone();
        if let Some(regex) = regex {
            if self.exceeded.borrow().contains(needle) {
                return false;
            }
            return match regex_outcome(&regex, haystack) {
                RegexOutcome::Match => true,
                RegexOutcome::NoMatch => false,
                RegexOutcome::Exceeded => {
                    self.exceeded.borrow_mut().insert(needle.to_owned());
                    false
                }
            };
        }
        let case_sensitive = entry.case_sensitive.unwrap_or(self.settings.case_sensitive);
        let (hay, word) = match case_sensitive {
            true => (haystack.to_owned(), needle.to_owned()),
            false => (haystack.to_lowercase(), needle.to_lowercase()),
        };
        if entry
            .match_whole_words
            .unwrap_or(self.settings.match_whole_words)
        {
            if word.chars().any(is_js_whitespace) {
                return hay.contains(&word);
            }
            return whole_word(&hay, &word);
        }
        hay.contains(&word)
    }

    fn add_recurse(&mut self, text: String, public: String) {
        self.recurse.push(text);
        if !public.is_empty() {
            self.recurse_public.push(public);
        }
    }

    /// 群組計分：命中的主鍵數，AND_ANY 加次要鍵、AND_ALL 次要鍵全中才加（用原始鍵，不代換）。
    fn score(&self, entry: &WiEntry) -> f64 {
        let text = self.text(entry, false);
        let Some(primary) = entry.key.as_ref().filter(|keys| !keys.is_empty()) else {
            return 0.0;
        };
        let count = |keys: &[String]| {
            keys.iter()
                .filter(|key| self.match_keys(&text, key, entry))
                .count() as f64
        };
        let primary_score = count(primary);
        let secondary_score = count(&entry.keysecondary);
        if !entry.keysecondary.is_empty() {
            if entry.selective_logic == logic::AND_ANY {
                return primary_score + secondary_score;
            }
            if entry.selective_logic == logic::AND_ALL {
                return match secondary_score == entry.keysecondary.len() as f64 {
                    true => primary_score + secondary_score,
                    false => primary_score,
                };
            }
        }
        primary_score
    }
}

/// `(?:^|\W)(詞)(?:$|\W)`（不帶 u：`\W` 只看 ASCII）。逐一檢查每個出現位置（含重疊）。
fn whole_word(hay: &str, word: &str) -> bool {
    let mut from = 0;
    while from <= hay.len() {
        let Some(found) = hay[from..].find(word) else {
            return false;
        };
        let at = from + found;
        let before_ok = hay[..at]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_js_word(ch));
        let after_ok = hay[at + word.len()..]
            .chars()
            .next()
            .is_none_or(|ch| !is_js_word(ch));
        if before_ok && after_ok {
            return true;
        }
        from = at + hay[at..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// 一個關鍵字條目的比對證據：掃描時代換好的鍵（已 trim；代換結果是空字串記 None）與是否讀到私密來源。
#[derive(Default)]
struct Evidence {
    primary: Option<(String, bool)>,
    secondary: Vec<(Option<String>, bool)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScanState {
    None,
    Initial,
    Recursion,
}

fn is_sticky(timed: &TimedEffects, entry: &WiEntry) -> bool {
    timed.is_active(TimedType::Sticky, &entry.id)
}

/// `checkWorldInfo`。`entries` 要先照 `sort_entries` 排好。
pub fn check_world_info(
    entries: &[WiEntry],
    input: ScanInput<'_>,
    hooks: &mut dyn ScanHooks,
) -> WiResult {
    let settings = input.settings;
    let mut buffer = Buffer::new(input.chat, input.global_scan, settings);
    let timed_entries = entries.to_vec();
    let mut timed = TimedEffects::new(input.chat.len(), &timed_entries, input.timed);
    timed.check();
    if entries.is_empty() {
        return WiResult::empty(timed.state);
    }
    let mut entries = entries.to_vec();

    let mut budget = js_round(settings.budget_percent * input.max_context / 100.0);
    if budget == 0.0 || budget.is_nan() {
        budget = 1.0;
    }
    if settings.budget_cap > 0.0 && budget > settings.budget_cap {
        budget = settings.budget_cap;
    }

    let mut levels: Vec<f64> = Vec::new();
    for entry in entries
        .iter()
        .filter(|entry| entry.delay_until_recursion.truthy())
    {
        let level = entry.delay_until_recursion.level();
        if !levels.contains(&level) {
            levels.push(level);
        }
    }
    stable_sort(&mut levels, &|a: &f64, b: &f64| a - b);
    let mut levels = std::collections::VecDeque::from(levels);
    let mut current_level = levels.pop_front().unwrap_or(0.0);

    let mut scan_state = ScanState::Initial;
    let mut overflowed = false;
    // allActivated：Map（id → 條目），保持加入順序
    let mut all_activated: Vec<(String, usize)> = Vec::new();
    let mut failed_probability: HashSet<usize> = HashSet::new();
    let mut all_activated_text = String::new();
    let mut private_of: HashMap<usize, bool> = HashMap::new();
    let mut content_private: HashSet<usize> = HashSet::new();

    let activated_has =
        |all: &[(String, usize)], id: &str| all.iter().any(|(candidate, _)| candidate == id);

    while scan_state != ScanState::None {
        let mut next_state = ScanState::None;
        let mut activated_now: Vec<usize> = Vec::new();
        let mut evidence: HashMap<usize, Evidence> = HashMap::new();
        let mut public_by_rule: HashSet<usize> = HashSet::new();

        for (index, entry) in entries.iter().enumerate() {
            if failed_probability.contains(&index) || activated_has(&all_activated, &entry.id) {
                continue;
            }
            if entry.disable {
                continue;
            }
            if !entry.triggers.is_empty() && !entry.triggers.iter().any(|t| t == input.trigger) {
                continue;
            }
            let sticky = is_sticky(&timed, entry);
            if timed.is_active(TimedType::Delay, &entry.id) {
                continue;
            }
            if timed.is_active(TimedType::Cooldown, &entry.id) && !sticky {
                continue;
            }
            let delayed = entry.delay_until_recursion.truthy();
            if scan_state != ScanState::Recursion && delayed && !sticky {
                continue;
            }
            if scan_state == ScanState::Recursion
                && delayed
                && entry.delay_until_recursion.level() > current_level
                && !sticky
            {
                continue;
            }
            if scan_state == ScanState::Recursion
                && settings.recursive
                && entry.exclude_recursion
                && !sticky
            {
                continue;
            }
            if entry.decorators.iter().any(|d| d == "@@activate") {
                activated_now.push(index);
                public_by_rule.insert(index);
                continue;
            }
            if entry.decorators.iter().any(|d| d == "@@dont_activate") {
                continue;
            }
            if entry.constant || sticky {
                activated_now.push(index);
                if entry.constant || !timed.sticky_confidential(&entry.id) {
                    public_by_rule.insert(index);
                }
                continue;
            }
            let Some(keys) = entry.key.as_ref().filter(|keys| !keys.is_empty()) else {
                continue;
            };

            let text = buffer.text(entry, false);
            let mut proof = Evidence::default();
            let mut primary = false;
            for key in keys {
                let substituted = hooks.substitute(key);
                if substituted.text.is_empty() {
                    continue;
                }
                let trimmed = js_trim(&substituted.text).to_owned();
                if buffer.match_keys(&text, &trimmed, entry) {
                    proof.primary = Some((trimmed, substituted.private));
                    primary = true;
                    break;
                }
            }
            if !primary {
                continue;
            }
            if !(entry.selective && !entry.keysecondary.is_empty()) {
                activated_now.push(index);
                evidence.insert(index, proof);
                continue;
            }
            let selective_logic = entry.selective_logic;
            let secondary_matched = {
                let mut any = false;
                let mut all = true;
                let mut decided = None;
                for key in &entry.keysecondary {
                    let substituted = hooks.substitute(key);
                    let trimmed = (!substituted.text.is_empty())
                        .then(|| js_trim(&substituted.text).to_owned());
                    let hit = trimmed
                        .as_ref()
                        .is_some_and(|key| buffer.match_keys(&text, key, entry));
                    proof.secondary.push((trimmed, substituted.private));
                    if hit {
                        any = true;
                    } else {
                        all = false;
                    }
                    if selective_logic == logic::AND_ANY && hit {
                        decided = Some(true);
                        break;
                    }
                    if selective_logic == logic::NOT_ALL && !hit {
                        decided = Some(true);
                        break;
                    }
                }
                decided.unwrap_or(
                    (selective_logic == logic::NOT_ANY && !any)
                        || (selective_logic == logic::AND_ALL && all),
                )
            };
            if secondary_matched {
                activated_now.push(index);
                evidence.insert(index, proof);
            }
        }

        // 觸發來源：用這一輪「緩衝去掉私密片段」的文字、掃描時代換好的鍵，照同一套規則再判一次
        for &index in &activated_now {
            let entry = &entries[index];
            let public = match evidence.get(&index) {
                None => public_by_rule.contains(&index),
                Some(proof) => publicly_matched(&buffer, entry, proof),
            };
            private_of.insert(index, !public);
        }

        // 機率與預算照這個順序：sticky 先，其餘照排好的順序
        let mut new_entries = activated_now.clone();
        if new_entries.len() > 1 {
            // `Number(sticky(b)) - Number(sticky(a)) || index(a) - index(b)`
            stable_sort(&mut new_entries, &|a: &usize, b: &usize| {
                let sticky = |index: usize| f64::from(u8::from(is_sticky(&timed, &entries[index])));
                let by_sticky = sticky(*b) - sticky(*a);
                if by_sticky != 0.0 {
                    by_sticky
                } else {
                    *a as f64 - *b as f64
                }
            });
        }

        let mut new_content = String::new();
        let text_tokens = match all_activated_text.is_empty() {
            true => 0.0,
            false => hooks.count_tokens(&all_activated_text),
        };
        filter_by_inclusion_groups(
            &mut new_entries,
            &all_activated,
            &entries,
            &buffer,
            &timed,
            hooks,
            settings,
        );

        let mut ignores_budget = new_entries
            .iter()
            .filter(|index| entries[**index].ignore_budget)
            .count();
        let mut pinned_left = new_entries
            .iter()
            .filter(|index| input.pinned.contains(&entries[**index].id))
            .count();
        for &index in &new_entries.clone() {
            let ignore = entries[index].ignore_budget;
            let pinned = input.pinned.contains(&entries[index].id);
            if ignore {
                ignores_budget -= 1;
            }
            if pinned {
                pinned_left -= 1;
            }
            if overflowed && !ignore && !pinned {
                if ignores_budget > 0 || pinned_left > 0 {
                    continue;
                }
                break;
            }
            let entry = &entries[index];
            let passes = !entry.use_probability
                || entry.probability == 100.0
                || is_sticky(&timed, entry)
                || hooks.random() * 100.0 <= entry.probability;
            if !passes {
                failed_probability.insert(index);
                continue;
            }
            let substituted = hooks.substitute(&entries[index].content);
            if substituted.private {
                content_private.insert(index);
            }
            entries[index].content = substituted.text;
            new_content.push_str(&entries[index].content);
            new_content.push('\n');
            if !ignore && text_tokens + hooks.count_tokens(&new_content) >= budget {
                overflowed = true;
                if !pinned {
                    continue;
                }
            }
            let id = entries[index].id.clone();
            match all_activated
                .iter_mut()
                .find(|(candidate, _)| *candidate == id)
            {
                Some(slot) => slot.1 = index,
                None => all_activated.push((id, index)),
            }
        }

        let successful: Vec<usize> = new_entries
            .iter()
            .copied()
            .filter(|index| !failed_probability.contains(index))
            .collect();
        let for_recursion: Vec<usize> = successful
            .into_iter()
            .filter(|index| !entries[*index].prevent_recursion)
            .collect();
        if settings.recursive && !overflowed && !for_recursion.is_empty() {
            next_state = ScanState::Recursion;
        }
        if next_state == ScanState::None && !levels.is_empty() {
            next_state = ScanState::Recursion;
            current_level = levels.pop_front().expect("checked");
        }
        scan_state = next_state;
        if scan_state != ScanState::None {
            let text = for_recursion
                .iter()
                .map(|index| entries[*index].content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let public = for_recursion
                .iter()
                .filter(|index| {
                    !private_of.get(index).copied().unwrap_or(true)
                        && !entries[**index].limited
                        && !content_private.contains(index)
                })
                .map(|index| entries[*index].content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if !text.is_empty() {
                all_activated_text = format!("{text}\n{all_activated_text}");
                buffer.add_recurse(text, public);
            }
        }
    }

    let mut result = WiResult::empty(WiTimed::default());
    let mut before: Vec<String> = Vec::new();
    let mut after: Vec<String> = Vec::new();
    let mut placed: Vec<usize> = all_activated.iter().map(|(_, index)| *index).collect();
    stable_sort(&mut placed, &|a: &usize, b: &usize| {
        compare_order(&entries[*a], &entries[*b])
    });
    for &index in &placed {
        let entry = &entries[index];
        let content = entry.content.clone();
        result.placed.push((entry.id.clone(), content.clone()));
        if content.is_empty() {
            continue;
        }
        let pos = entry.position;
        if pos == position::BEFORE {
            before.insert(0, content);
        } else if pos == position::AFTER {
            after.insert(0, content);
        } else if pos == position::EM_TOP {
            result.examples.insert(
                0,
                Example {
                    position: "before",
                    content,
                },
            );
        } else if pos == position::EM_BOTTOM {
            result.examples.insert(
                0,
                Example {
                    position: "after",
                    content,
                },
            );
        } else if pos == position::AN_TOP {
            result.an_top.insert(0, content);
        } else if pos == position::AN_BOTTOM {
            result.an_bottom.insert(0, content);
        } else if pos == position::AT_DEPTH {
            let depth = entry.depth;
            let role = entry.role;
            match result
                .depth
                .iter_mut()
                .find(|group| group.depth == depth && group.role == role)
            {
                Some(group) => group.entries.insert(0, content),
                None => result.depth.push(DepthGroup {
                    depth: entry.depth,
                    role,
                    entries: vec![content],
                }),
            }
        } else if pos == position::OUTLET && !entry.outlet_name.is_empty() {
            match result
                .outlets
                .iter_mut()
                .find(|(name, _)| *name == entry.outlet_name)
            {
                Some((_, values)) => values.push(content),
                None => result
                    .outlets
                    .push((entry.outlet_name.clone(), vec![content])),
            }
        }
    }
    let order = js_key_order(result.outlets.iter().map(|(name, _)| name.clone()));
    let mut outlets = std::mem::take(&mut result.outlets);
    for name in order {
        let at = outlets
            .iter()
            .position(|(candidate, _)| *candidate == name)
            .expect("same names");
        result.outlets.push(outlets.remove(at));
    }
    result.before = before.join("\n");
    result.after = after.join("\n");
    let activated: Vec<(&WiEntry, bool)> = all_activated
        .iter()
        .map(|(_, index)| {
            (
                &entries[*index],
                private_of.get(index).copied().unwrap_or(false),
            )
        })
        .collect();
    timed.set(&activated);
    result.activated = all_activated.iter().map(|(id, _)| id.clone()).collect();
    for (id, index) in &all_activated {
        if private_of.get(index).copied().unwrap_or(false) {
            result.private_ids.insert(id.clone());
        }
        if content_private.contains(index) {
            result.private_content.insert(id.clone());
        }
    }
    result.timed = timed.state;
    result
}

/// 只用公開片段能不能證明這條觸發：主鍵與次要鍵都要成立；證據不足（鍵沒在掃描時代換過、或讀到私密來源）算不成立。
fn publicly_matched(buffer: &Buffer<'_>, entry: &WiEntry, proof: &Evidence) -> bool {
    let text = buffer.text(entry, true);
    let Some((primary, private)) = &proof.primary else {
        return false;
    };
    if *private || !buffer.match_keys(&text, primary, entry) {
        return false;
    }
    if !(entry.selective && !entry.keysecondary.is_empty()) {
        return true;
    }
    if proof.secondary.iter().any(|(_, private)| *private) {
        return false;
    }
    let hits: Vec<bool> = proof
        .secondary
        .iter()
        .map(|(key, _)| {
            key.as_ref()
                .is_some_and(|key| buffer.match_keys(&text, key, entry))
        })
        .collect();
    let complete = hits.len() == entry.keysecondary.len();
    let logic = entry.selective_logic;
    if logic == logic::AND_ANY {
        hits.iter().any(|hit| *hit)
    } else if logic == logic::NOT_ALL {
        hits.iter().any(|hit| !hit)
    } else if logic == logic::NOT_ANY {
        complete && !hits.iter().any(|hit| *hit)
    } else if logic == logic::AND_ALL {
        complete && hits.iter().all(|hit| *hit)
    } else {
        false
    }
}

/// `group.split(/,\s*/)`
fn split_groups(group: &str) -> Vec<String> {
    let mut parts = group.split(',');
    let mut result = vec![parts.next().unwrap_or("").to_owned()];
    result.extend(parts.map(|part| part.trim_start_matches(is_js_whitespace).to_owned()));
    result
}

/// `filterByInclusionGroups`；同一條在多個群組被移除時找不到就不刪（D31）。
#[allow(clippy::too_many_arguments)]
fn filter_by_inclusion_groups(
    new_entries: &mut Vec<usize>,
    all_activated: &[(String, usize)],
    entries: &[WiEntry],
    buffer: &Buffer<'_>,
    timed: &TimedEffects<'_>,
    hooks: &mut dyn ScanHooks,
    settings: &WiSettings,
) {
    let mut grouped: Vec<(String, Vec<usize>)> = Vec::new();
    for &item in new_entries.iter() {
        if entries[item].group.is_empty() {
            continue;
        }
        for name in split_groups(&entries[item].group)
            .into_iter()
            .filter(|name| !name.is_empty())
        {
            match grouped.iter_mut().find(|(key, _)| *key == name) {
                Some((_, members)) => members.push(item),
                None => grouped.push((name, vec![item])),
            }
        }
    }
    if grouped.is_empty() {
        return;
    }
    let order = js_key_order(grouped.iter().map(|(key, _)| key.clone()));
    let mut grouped: Vec<(String, Vec<usize>)> = order
        .into_iter()
        .map(|key| {
            let members = grouped
                .iter()
                .find(|(candidate, _)| *candidate == key)
                .map(|(_, members)| members.clone())
                .unwrap_or_default();
            (key, members)
        })
        .collect();
    let remove = |new_entries: &mut Vec<usize>, item: usize| {
        if let Some(at) = new_entries.iter().position(|candidate| *candidate == item) {
            new_entries.remove(at);
        }
    };
    let remove_all_but = |new_entries: &mut Vec<usize>, group: &[usize], chosen: Option<usize>| {
        for &item in group {
            if Some(item) != chosen {
                remove(new_entries, item);
            }
        }
    };

    // 計時：有 sticky 的群組只留 sticky；冷卻與延遲中的移除
    let mut has_sticky: HashMap<String, bool> = HashMap::new();
    for (key, group) in &grouped {
        has_sticky.insert(key.clone(), false);
        let sticky: Vec<usize> = group
            .iter()
            .copied()
            .filter(|item| is_sticky(timed, &entries[*item]))
            .collect();
        if !sticky.is_empty() {
            for &item in group {
                if !sticky.contains(&item) {
                    remove(new_entries, item);
                }
            }
            has_sticky.insert(key.clone(), true);
        }
        for &item in group {
            if timed.is_active(TimedType::Cooldown, &entries[item].id) {
                remove(new_entries, item);
            }
        }
        for &item in group {
            if timed.is_active(TimedType::Delay, &entries[item].id) {
                remove(new_entries, item);
            }
        }
    }
    // 群組計分
    for (key, group) in grouped.iter_mut() {
        if !settings.use_group_scoring
            && !group
                .iter()
                .any(|item| entries[*item].use_group_scoring == Some(true))
        {
            continue;
        }
        if has_sticky[key.as_str()] {
            continue;
        }
        let mut scores: Vec<f64> = group
            .iter()
            .map(|item| buffer.score(&entries[*item]))
            .collect();
        let max_score = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mut index = 0;
        while index < group.len() {
            let item = group[index];
            if !entries[item]
                .use_group_scoring
                .unwrap_or(settings.use_group_scoring)
            {
                index += 1;
                continue;
            }
            if scores[index] < max_score {
                remove(new_entries, item);
                group.remove(index);
                scores.remove(index);
                continue;
            }
            index += 1;
        }
    }
    for (key, group) in &grouped {
        if has_sticky[key.as_str()] {
            continue;
        }
        if all_activated
            .iter()
            .any(|(_, item)| entries[*item].group == *key)
        {
            remove_all_but(new_entries, group, None);
            continue;
        }
        if group.len() <= 1 {
            continue;
        }
        let mut prios: Vec<usize> = group
            .iter()
            .copied()
            .filter(|item| entries[*item].group_override)
            .collect();
        stable_sort(&mut prios, &|a: &usize, b: &usize| {
            compare_order(&entries[*a], &entries[*b])
        });
        if let Some(first) = prios.first() {
            remove_all_but(new_entries, group, Some(*first));
            continue;
        }
        let weight = |item: usize| entries[item].group_weight;
        let total: f64 = group.iter().map(|item| weight(*item)).sum();
        let roll = hooks.random() * total;
        let mut current = 0.0;
        let mut winner = None;
        for &item in group {
            current += weight(item);
            if roll <= current {
                winner = Some(item);
                break;
            }
        }
        if let Some(winner) = winner {
            remove_all_but(new_entries, group, Some(winner));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_info::entry::from_world_file;
    use crate::world_info::settings::ST_WI_SETTINGS;

    #[test]
    fn a_key_that_hit_the_backtrack_limit_is_not_tried_again() {
        let chat = vec![format!("{}b", "a".repeat(30))];
        let global = GlobalScan::default();
        let buffer = Buffer::new(&chat, &global, &ST_WI_SETTINGS);
        let entry = from_world_file(&serde_json::Map::new());
        let key = "/(a|a)+\\1$/";
        let text = buffer.text(&entry, false);
        assert!(!buffer.match_keys(&text, key, &entry));
        assert!(buffer.exceeded.borrow().contains(key));
        let started = std::time::Instant::now();
        assert!(!buffer.match_keys(&text, key, &entry));
        assert!(started.elapsed() < std::time::Duration::from_millis(5));
    }
}
