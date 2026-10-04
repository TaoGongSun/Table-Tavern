//! 上游 MVU 的 set（MagVarUpdate 438f9ffc `update_variables.ts` L906-956，非 strictSet）：沒重構的 MVU 卡的
//! replace 走這裡。狀態樹的葉子只有字串、看不出原本的 JSON 型別，所以 set 不看葉子文字猜：一批指令維持一份完整的
//! 帶型別 stat_data（`TypedView`），照指令先後同步，舊值從這份取、新值照型別寫回；整批做完這份就是合回的底。
//! 沒有 stat_data（樹模式）才從葉子文字還原一份。非有限數（NaN／Infinity）在整批做完、序列化時才轉 null。
use crate::data::message_vars::{leaf_text, restore_leaf, Json};
use crate::data::StateNode;
use std::collections::{BTreeMap, BTreeSet};

/// 一批指令進行中的 JS 值：數字可以是 NaN／Infinity（上游記憶體裡就是這樣，下一條 set 看到的是 number）。
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Val {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Val>),
    Obj(Vec<(String, Val)>),
}

impl Val {
    pub(super) fn from_json(value: &Json) -> Val {
        match value {
            Json::Null => Val::Null,
            Json::Bool(flag) => Val::Bool(*flag),
            Json::Number(number) => Val::Num(number.as_f64().unwrap_or(f64::NAN)),
            Json::String(text) => Val::Str(text.clone()),
            Json::Array(items) => Val::Arr(items.iter().map(Val::from_json).collect()),
            Json::Object(entries) => Val::Obj(
                entries
                    .iter()
                    .map(|(key, child)| (key.clone(), Val::from_json(child)))
                    .collect(),
            ),
        }
    }

    pub(super) fn from_serde(value: &serde_json::Value) -> Val {
        crate::data::message_vars::parse_json(&value.to_string())
            .map(|json| Val::from_json(&json))
            .unwrap_or(Val::Null)
    }

    /// 序列化邊界：非有限數轉 null（同 JSON.stringify）。
    pub(super) fn to_json(&self) -> Json {
        match self {
            Val::Null => Json::Null,
            Val::Bool(flag) => Json::Bool(*flag),
            Val::Num(number) => number_json(*number),
            Val::Str(text) => Json::String(text.clone()),
            Val::Arr(items) => Json::Array(items.iter().map(Val::to_json).collect()),
            Val::Obj(entries) => Json::Object(
                entries
                    .iter()
                    .map(|(key, child)| (key.clone(), child.to_json()))
                    .collect(),
            ),
        }
    }

    /// 投影成狀態樹節點，規則同 `stat_to_tree`：物件成分支，其餘成葉子文字。
    pub(super) fn node(&self) -> StateNode {
        match self {
            Val::Obj(entries) => StateNode::Branch(
                entries
                    .iter()
                    .map(|(key, child)| (key.clone(), child.node()))
                    .collect(),
            ),
            other => StateNode::Leaf(leaf_text(&other.to_json())),
        }
    }
}

/// 數字寫回 JSON：整數照整數存（同 JSON 解析整數文字的結果），其餘照 f64；非有限數是 null。
fn number_json(value: f64) -> Json {
    if !value.is_finite() {
        return Json::Null;
    }
    if value.fract() == 0.0 {
        if (0.0..18_446_744_073_709_551_616.0).contains(&value) {
            return Json::Number((value as u64).into());
        }
        if (-9_223_372_036_854_775_808.0..0.0).contains(&value) {
            return Json::Number((value as i64).into());
        }
    }
    serde_json::Number::from_f64(value)
        .map(Json::Number)
        .unwrap_or(Json::Null)
}

/// set 的套用規則：`[值, 說明]` 只改第 0 項（舊值是數字且新值不是 null 才 `Number()`）；
/// 舊值是數字、新值是字串就 `Number()`；其餘照新值寫。不夾範圍、不拒收。
pub(super) fn upstream_set(old: &Val, new: Val) -> Val {
    if let Val::Arr(items) = old {
        if items.len() == 2 && matches!(items[1], Val::Str(_)) && !matches!(items[0], Val::Arr(_)) {
            let first = if matches!(items[0], Val::Num(_)) && new != Val::Null {
                Val::Num(js_to_number(&new))
            } else {
                new
            };
            return Val::Arr(vec![first, items[1].clone()]);
        }
    }
    match (old, new) {
        (Val::Num(_), Val::Str(text)) => Val::Num(js_number(&text)),
        (_, new) => new,
    }
}

/// JS `Number(value)`。
fn js_to_number(value: &Val) -> f64 {
    match value {
        Val::Null => 0.0,
        Val::Bool(flag) => f64::from(u8::from(*flag)),
        Val::Num(number) => *number,
        Val::Str(text) => js_number(text),
        // 陣列先轉字串（join）：空陣列是 ""＝0、單一元素看那個元素的字串、兩個以上一定有逗號＝NaN
        Val::Arr(items) => match items.as_slice() {
            [] => 0.0,
            [Val::Null] => 0.0,
            [only @ (Val::Num(_) | Val::Str(_) | Val::Arr(_))] => js_to_number(only),
            _ => f64::NAN,
        },
        Val::Obj(_) => f64::NAN,
    }
}

/// JS 字串轉數字的空白（StrWhiteSpaceChar：WhiteSpace＋LineTerminator，含 U+FEFF 與 Zs 類）。
fn js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// JS `Number(text)`：去掉 JS 空白、空字串是 0、0x／0o／0b 不限長度（照正確捨入）、帶號 Infinity，
/// 其餘照十進位字面值；都不是就是 NaN。
pub(crate) fn js_number(text: &str) -> f64 {
    let text = text.trim_matches(js_space);
    if text.is_empty() {
        return 0.0;
    }
    for (prefix, bits) in [("0x", 4), ("0o", 3), ("0b", 1)] {
        let lower = text.get(..2).map(str::to_ascii_lowercase);
        if lower.as_deref() == Some(prefix) {
            return radix_number(&text[2..], bits);
        }
    }
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    if unsigned == "Infinity" {
        return if text.starts_with('-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(index) => (&unsigned[..index], Some(&unsigned[index + 1..])),
        None => (unsigned, None),
    };
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    let mantissa_ok = match mantissa.split_once('.') {
        Some((whole, fraction)) => {
            (whole.is_empty() || digits(whole))
                && (fraction.is_empty() || digits(fraction))
                && !(whole.is_empty() && fraction.is_empty())
        }
        None => digits(mantissa),
    };
    let exponent_ok =
        exponent.is_none_or(|exp| digits(exp.strip_prefix(['+', '-']).unwrap_or(exp)));
    match mantissa_ok && exponent_ok {
        true => text.parse::<f64>().unwrap_or(f64::NAN),
        false => f64::NAN,
    }
}

/// 2 的冪進位（每位 `bits` 位元）的整數，任意長度、照 IEEE 就近偶數捨入。
fn radix_number(digits: &str, bits: u32) -> f64 {
    let radix = 1u32 << bits;
    let mut values = Vec::with_capacity(digits.len());
    for c in digits.chars() {
        match c.to_digit(radix) {
            Some(value) => values.push(value),
            None => return f64::NAN,
        }
    }
    if values.is_empty() {
        return f64::NAN;
    }
    // 攤成位元（高位在前），去掉前導 0
    let mut stream: Vec<bool> = values
        .iter()
        .flat_map(|value| (0..bits).rev().map(move |shift| value >> shift & 1 == 1))
        .collect();
    let leading = stream.iter().take_while(|bit| !**bit).count();
    stream.drain(..leading);
    if stream.len() <= 128 {
        let whole = stream
            .iter()
            .fold(0u128, |acc, bit| acc << 1 | u128::from(*bit));
        return whole as f64;
    }
    // 超過 128 位元：取前 127 位元，其後只要有 1 就補一個黏著位元，捨入結果不變，再乘回 2 的次方
    let mut head = stream[..127]
        .iter()
        .fold(0u128, |acc, bit| acc << 1 | u128::from(*bit));
    head = head << 1 | u128::from(stream[127..].iter().any(|bit| *bit));
    let shift = (stream.len() - 128) as i32;
    (head as f64) * 2f64.powi(shift)
}

/// 一批指令的完整帶型別視圖：整份 stat_data（樹模式沒有就照樹的葉子文字還原一份），照指令先後逐條同步——
/// set 照算出的型別寫；其他指令（delta、insert、remove、move、狀態欄平欄）照它對樹造成的前後差異寫
/// （型別還原規則同 `merge_tree_change`）。NaN／Infinity 在文件裡先存 null，另記位置，讀舊值時還原成數字。
pub(super) struct TypedView {
    /// 不是沒重構的 MVU 卡就不建視圖（原規則不走 set，也不改合回方式）
    active: bool,
    doc: Json,
    non_finite: BTreeSet<Vec<String>>,
    types: BTreeMap<String, String>,
    /// 這條指令是不是 set 寫的（是就不再照樹的差異同步，免得型別被文字還原蓋掉）
    set_written: bool,
}

impl TypedView {
    /// 沒重構的 MVU 卡才建視圖；`base`／`before`／`now` 見 `new`。
    pub(super) fn for_table(
        mechanism: &crate::data::Mechanism,
        base: Option<&Json>,
        before: &BTreeMap<String, StateNode>,
        now: &BTreeMap<String, StateNode>,
    ) -> Self {
        match mechanism.numeric_update {
            crate::data::NumericUpdate::Upstream => {
                Self::new(base, before, now, &mechanism.value_types)
            }
            crate::data::NumericUpdate::DeltaOnly => Self::inactive(),
        }
    }

    fn inactive() -> Self {
        Self {
            active: false,
            doc: Json::Null,
            non_finite: BTreeSet::new(),
            types: BTreeMap::new(),
            set_written: false,
        }
    }

    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    /// set 指令寫入：照算出的型別寫，這條指令不再照樹的差異同步。
    pub(super) fn set(&mut self, path: &[String], value: &Val) {
        self.put(path, value);
        self.set_written = true;
    }

    /// 直接照型別改寫一處（move 搬成功後補回原值的型別）。
    pub(super) fn overwrite(&mut self, path: &[String], value: &Val) {
        self.put(path, value);
    }

    /// 一條指令做完：不是 set 寫的就照它對樹造成的差異同步（只看它可能動到的頂層鍵）。
    pub(super) fn after_command(
        &mut self,
        keys: &[&String],
        before: &BTreeMap<String, StateNode>,
        now: &BTreeMap<String, StateNode>,
    ) {
        if !std::mem::take(&mut self.set_written) {
            self.sync(keys, before, now);
        }
    }

    /// `before` 是 `base` 投影出來的那棵樹（呼叫端拿到的原樹），`now` 是目前的樹：先把兩者的差異（例如
    /// 狀態欄平欄）同步進文件。沒有 `base` 就照 `now` 還原一份。
    fn new(
        base: Option<&Json>,
        before: &BTreeMap<String, StateNode>,
        now: &BTreeMap<String, StateNode>,
        types: &BTreeMap<String, String>,
    ) -> Self {
        let mut view = Self {
            active: true,
            doc: Json::empty_object(),
            non_finite: BTreeSet::new(),
            types: types.clone(),
            set_written: false,
        };
        match base {
            Some(base @ Json::Object(_)) => {
                view.doc = base.clone();
                view.sync_children(&mut Vec::new(), before, now);
            }
            _ => view.sync_children(&mut Vec::new(), &BTreeMap::new(), now),
        }
        view
    }

    /// 這條路徑目前的帶型別值（含批內 set 出來、還沒序列化的 NaN）。
    pub(super) fn get(&self, path: &[String]) -> Option<Val> {
        let mut node = &self.doc;
        for segment in path {
            node = node.get(segment)?;
        }
        let mut value = Val::from_json(node);
        for mark in &self.non_finite {
            if let Some(rest) = mark.strip_prefix(path) {
                set_non_finite(&mut value, rest);
            }
        }
        Some(value)
    }

    /// 整個位置換成 `value`（舊的子欄寫入與非有限數標記一併作廢）。
    fn put(&mut self, path: &[String], value: &Val) {
        self.non_finite.retain(|mark| !mark.starts_with(path));
        let mut marks = Vec::new();
        collect_non_finite(value, &mut path.to_vec(), &mut marks);
        self.non_finite.extend(marks);
        let Some((key, parents)) = path.split_last() else {
            self.doc = value.to_json();
            return;
        };
        let mut target = &mut self.doc;
        for segment in parents {
            if !matches!(target.get(segment), Some(Json::Object(_))) {
                target.insert(segment, Json::empty_object());
            }
            target = target.get_mut(segment).expect("剛補上的分支");
        }
        target.insert(key, value.to_json());
    }

    fn remove(&mut self, path: &[String]) {
        self.non_finite.retain(|mark| !mark.starts_with(path));
        let Some((key, parents)) = path.split_last() else {
            return;
        };
        let mut target = Some(&mut self.doc);
        for segment in parents {
            target = target.and_then(|node| node.get_mut(segment));
        }
        if let Some(parent) = target {
            parent.remove(key);
        }
    }

    /// `before`／`now` 是指令前後的整棵樹，只看它可能動到的頂層鍵。
    fn sync(
        &mut self,
        keys: &[&String],
        before: &BTreeMap<String, StateNode>,
        now: &BTreeMap<String, StateNode>,
    ) {
        for key in keys {
            let mut path = vec![(*key).clone()];
            self.sync_node(&mut path, before.get(*key), now.get(*key));
        }
    }

    fn sync_children(
        &mut self,
        path: &mut Vec<String>,
        before: &BTreeMap<String, StateNode>,
        now: &BTreeMap<String, StateNode>,
    ) {
        let keys: BTreeSet<&String> = before.keys().chain(now.keys()).collect();
        for key in keys {
            path.push(key.clone());
            self.sync_node(path, before.get(key), now.get(key));
            path.pop();
        }
    }

    fn sync_node(
        &mut self,
        path: &mut Vec<String>,
        before: Option<&StateNode>,
        now: Option<&StateNode>,
    ) {
        if before == now {
            return;
        }
        match (before, now) {
            (_, None) => self.remove(path),
            (Some(StateNode::Branch(old)), Some(StateNode::Branch(new)))
                if matches!(self.get(path), Some(Val::Obj(_))) =>
            {
                self.sync_children(path, old, new)
            }
            (_, Some(node)) => {
                let value = self.restore(path, node);
                // 同 merge_tree_change：舊值是 `[值, 說明]` 而新值不是陣列，只改第 0 項
                let value = match (self.get(path), value) {
                    (Some(Val::Arr(mut pair)), value)
                        if pair.len() == 2 && !matches!(value, Val::Arr(_)) =>
                    {
                        pair[0] = value;
                        Val::Arr(pair)
                    }
                    (_, value) => value,
                };
                self.put(path, &value);
            }
        }
    }

    fn restore(&self, path: &mut Vec<String>, node: &StateNode) -> Val {
        match node {
            StateNode::Leaf(text) => Val::from_json(&restore_leaf(
                text,
                self.types.get(&path.join(".")).map(String::as_str),
            )),
            StateNode::Branch(children) => Val::Obj(
                children
                    .iter()
                    .map(|(key, child)| {
                        path.push(key.clone());
                        let value = self.restore(path, child);
                        path.pop();
                        (key.clone(), value)
                    })
                    .collect(),
            ),
        }
    }

    /// 整批做完：序列化好的整份 stat_data（非有限數已是 null）。
    pub(super) fn finish(self) -> Json {
        self.doc
    }
}

fn collect_non_finite(value: &Val, path: &mut Vec<String>, marks: &mut Vec<Vec<String>>) {
    match value {
        Val::Num(number) if !number.is_finite() => marks.push(path.clone()),
        Val::Arr(items) => {
            for (index, item) in items.iter().enumerate() {
                path.push(index.to_string());
                collect_non_finite(item, path, marks);
                path.pop();
            }
        }
        Val::Obj(entries) => {
            for (key, child) in entries {
                path.push(key.clone());
                collect_non_finite(child, path, marks);
                path.pop();
            }
        }
        _ => {}
    }
}

fn set_non_finite(value: &mut Val, rest: &[String]) {
    let Some((first, tail)) = rest.split_first() else {
        *value = Val::Num(f64::NAN);
        return;
    };
    let child = match value {
        Val::Arr(items) => first
            .parse::<usize>()
            .ok()
            .and_then(|index| items.get_mut(index)),
        Val::Obj(entries) => entries
            .iter_mut()
            .find(|(key, _)| key == first)
            .map(|(_, child)| child),
        _ => None,
    };
    if let Some(child) = child {
        set_non_finite(child, tail);
    }
}
