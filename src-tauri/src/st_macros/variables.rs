//! ST 聊天變數（local＝這段對話、global＝跨對話），照網頁版 `variables.ts`（ST `variables.js`）：
//! 讀出來是數字字串就轉數字、加法遇到非數字改成字串串接、JSON 陣列就 push、帶 index 存成 JSON。
//! 完整模式下每次寫入記成操作序列（[`VarOp`]），落地時在最新的表上重放（方案三之 8）。

use super::js_value::{array_index, is_blank, number_to_string, put_entry, text_limit, JsValue};

/// 帶索引寫入時陣列長度的實用上限；超過當寫入失敗（JS 吞掉例外、表不變）。
pub const MAX_ARRAY_LENGTH: usize = 1_000_000;

/// JS 丟例外（`throw`）：呼叫端把巨集原樣留著。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thrown;

/// 一個範圍的變數表（JS 物件：鍵依插入順序，輸出照 JS 鍵順序）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VarScope {
    values: Vec<(String, JsValue)>,
}

impl VarScope {
    /// 從 JSON 物件文字讀入（卡片變數層的 `vars`）；不是物件回錯。
    pub fn from_json_text(text: &str) -> Result<Self, String> {
        match JsValue::parse_json(text) {
            Some(JsValue::Object(values)) => Ok(Self { values }),
            Some(_) => Err("變數表不是 JSON 物件".to_owned()),
            None => Err("變數表不是合法 JSON".to_owned()),
        }
    }

    /// `JSON.stringify(values)`
    pub fn to_json_text(&self) -> String {
        JsValue::Object(self.values.clone())
            .stringify()
            .expect("物件一定能序列化")
    }

    /// 原始值（沒有這個鍵＝`None`）。
    pub fn raw(&self, name: &str) -> Option<&JsValue> {
        self.values
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }

    fn put(&mut self, name: &str, value: JsValue) {
        put_entry(&mut self.values, name, value);
    }

    pub fn has(&self, name: &str) -> bool {
        self.raw(name)
            .is_some_and(|value| !matches!(value, JsValue::Undefined))
    }

    pub fn get(&self, name: &str, index: Option<&str>) -> JsValue {
        let mut value = self.raw(name).cloned().unwrap_or(JsValue::Undefined);
        if let Some(index) = index {
            if let Some(parsed) = value.parse_coerced() {
                if parsed != JsValue::Null {
                    value = parsed.property(&property_key(index));
                    if value.is_object_like() {
                        value = JsValue::Str(value.stringify().expect("物件能序列化"));
                    }
                }
            }
        }
        let blank = matches!(&value, JsValue::Str(text) if is_blank(text));
        let number = value.to_number();
        if blank || number.is_nan() {
            if value.truthy() {
                value
            } else {
                JsValue::str("")
            }
        } else {
            JsValue::Num(number)
        }
    }

    pub fn set(
        &mut self,
        name: &str,
        value: JsValue,
        index: Option<&str>,
    ) -> Result<JsValue, Thrown> {
        self.set_tracked(name, value, index).map(|(value, _)| value)
    }

    /// `set`，另外回報有沒有真的寫進去（帶索引時解析或指定失敗，JS 吞掉例外、表不變）。
    fn set_tracked(
        &mut self,
        name: &str,
        value: JsValue,
        index: Option<&str>,
    ) -> Result<(JsValue, bool), Thrown> {
        if name.is_empty() {
            return Err(Thrown);
        }
        let Some(index) = index else {
            self.put(name, value.clone());
            return Ok((value, true));
        };
        // JSON.parse(values[name] ?? "null")；解析或指定失敗就算了（ST 也是吞掉）
        let source = match self.raw(name) {
            None | Some(JsValue::Undefined) | Some(JsValue::Null) => Some(JsValue::Null),
            Some(existing) => existing.parse_coerced(),
        };
        let written = source
            .and_then(|parsed| assign_index(parsed, index, value.clone()))
            .and_then(|updated| updated.stringify())
            .filter(|text| text.len() <= text_limit());
        let wrote = written.is_some();
        if let Some(text) = written {
            self.put(name, JsValue::Str(text));
        }
        Ok((value, wrote))
    }

    pub fn del(&mut self, name: &str) {
        self.values.retain(|(key, _)| key != name);
    }

    pub fn add(&mut self, name: &str, value: JsValue) -> Result<JsValue, Thrown> {
        self.add_tracked(name, value).map(|(value, _)| value)
    }

    /// `add`，另外回報有沒有真的寫進去（兩邊相加得 NaN 時不寫）。
    fn add_tracked(&mut self, name: &str, value: JsValue) -> Result<(JsValue, bool), Thrown> {
        let got = self.get(name, None);
        let current = if got.truthy() { got } else { JsValue::Num(0.0) };
        if let Some(JsValue::Array(mut items)) = current.parse_coerced() {
            items.push(value);
            let list = JsValue::Array(items);
            let text = list.stringify().expect("陣列能序列化");
            if text.len() > text_limit() {
                return Err(Thrown);
            }
            self.set(name, JsValue::Str(text), None)?;
            return Ok((list, true));
        }
        let increment = value.to_number();
        let base = current.to_number();
        if increment.is_nan() || base.is_nan() {
            let head = if current.truthy() {
                current.to_js_string()
            } else {
                String::new()
            };
            // 超過字串實用上限：當丟例外（先算長度，不先配置）
            let tail = value.to_js_string();
            if head.len() + tail.len() > text_limit() {
                return Err(Thrown);
            }
            let joined = head + &tail;
            let text = JsValue::Str(joined);
            self.set(name, text.clone(), None)?;
            return Ok((text, true));
        }
        let next = base + increment;
        if next.is_nan() {
            return Ok((JsValue::str(""), false));
        }
        self.set(name, JsValue::Num(next), None)?;
        Ok((JsValue::Num(next), true))
    }
}

/// `Number.isNaN(Number(index)) ? index : Number(index)` 當屬性鍵。
fn property_key(index: &str) -> String {
    let number = JsValue::str(index).to_number();
    if number.is_nan() {
        index.to_owned()
    } else {
        number_to_string(number)
    }
}

/// `parsed[index] = value`（null 先依索引是不是數字換成 `{}`／`[]`）；JS 會丟例外的情形回 `None`。
fn assign_index(parsed: JsValue, index: &str, value: JsValue) -> Option<JsValue> {
    let numeric = !JsValue::str(index).to_number().is_nan();
    let parsed = match parsed {
        JsValue::Null if numeric => JsValue::Array(Vec::new()),
        JsValue::Null => JsValue::Object(Vec::new()),
        other => other,
    };
    let key = property_key(index);
    match parsed {
        JsValue::Object(mut entries) => {
            put_entry(&mut entries, &key, value);
            Some(JsValue::Object(entries))
        }
        JsValue::Array(mut items) => {
            if let Some(at) = array_index(&key) {
                // 實用上限（比 ST 好：JS 要到字串上限才失敗，途中可吃掉數百 MB）
                if at >= MAX_ARRAY_LENGTH {
                    return None;
                }
                if at >= items.len() {
                    items.resize(at + 1, JsValue::Undefined);
                }
                items[at] = value;
            } else if key == "length" {
                let length = value.to_number();
                let valid = length >= 0.0 && length.fract() == 0.0 && length < 4_294_967_296.0;
                if !valid || length > MAX_ARRAY_LENGTH as f64 {
                    return None;
                }
                items.resize(length as usize, JsValue::Undefined);
            }
            // 其他鍵是陣列的非索引屬性，JSON.stringify 不輸出
            Some(JsValue::Array(items))
        }
        // 嚴格模式下對原始值設屬性丟 TypeError
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Local,
    Global,
}

/// 一次變數寫入（`inc`／`dec` 記成加 ±1、`-=` 記成加負數）。
#[derive(Debug, Clone, PartialEq)]
pub enum VarOpKind {
    Set {
        value: JsValue,
        index: Option<String>,
    },
    Add {
        value: JsValue,
    },
    Delete,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarOp {
    pub scope: ScopeKind,
    pub name: String,
    pub kind: VarOpKind,
}

/// 一次求值看得到的變數：兩個範圍＋這次求值的寫入紀錄。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Variables {
    pub local: VarScope,
    pub global: VarScope,
    /// 依發生順序；中性模式在隔離副本上記，結束就丟掉
    pub ops: Vec<VarOp>,
}

impl Variables {
    pub fn new(local: VarScope, global: VarScope) -> Self {
        Self {
            local,
            global,
            ops: Vec::new(),
        }
    }

    pub fn scope(&self, kind: ScopeKind) -> &VarScope {
        match kind {
            ScopeKind::Local => &self.local,
            ScopeKind::Global => &self.global,
        }
    }

    fn scope_mut(&mut self, kind: ScopeKind) -> &mut VarScope {
        match kind {
            ScopeKind::Local => &mut self.local,
            ScopeKind::Global => &mut self.global,
        }
    }

    pub fn set(
        &mut self,
        kind: ScopeKind,
        name: &str,
        value: JsValue,
        index: Option<&str>,
    ) -> Result<JsValue, Thrown> {
        let (result, wrote) = self
            .scope_mut(kind)
            .set_tracked(name, value.clone(), index)?;
        if !wrote {
            return Ok(result);
        }
        self.ops.push(VarOp {
            scope: kind,
            name: name.to_owned(),
            kind: VarOpKind::Set {
                value,
                index: index.map(str::to_owned),
            },
        });
        Ok(result)
    }

    pub fn add(&mut self, kind: ScopeKind, name: &str, value: JsValue) -> Result<JsValue, Thrown> {
        let (result, wrote) = self.scope_mut(kind).add_tracked(name, value.clone())?;
        if !wrote {
            return Ok(result);
        }
        self.ops.push(VarOp {
            scope: kind,
            name: name.to_owned(),
            kind: VarOpKind::Add { value },
        });
        Ok(result)
    }

    pub fn del(&mut self, kind: ScopeKind, name: &str) {
        self.scope_mut(kind).del(name);
        self.ops.push(VarOp {
            scope: kind,
            name: name.to_owned(),
            kind: VarOpKind::Delete,
        });
    }

    /// 在這份表上依序重放操作序列（落地時用最新的表重放；不再記錄）。只能打在還沒套用過這批 ops 的表上：
    /// `add` 等不是冪等的，重放兩次會加兩次。重放只重做寫入本身，不重判 `||=`、`??=`、`{{if}}` 分支等當初的條件。
    pub fn replay(&mut self, ops: &[VarOp]) {
        for op in ops {
            let scope = self.scope_mut(op.scope);
            // 原本成功的寫入在新表上也可能丟例外（例如名字空白不會發生）；照 JS 一樣當沒寫
            let _ = match &op.kind {
                VarOpKind::Set { value, index } => {
                    scope.set(&op.name, value.clone(), index.as_deref())
                }
                VarOpKind::Add { value } => scope.add(&op.name, value.clone()),
                VarOpKind::Delete => {
                    scope.del(&op.name);
                    Ok(JsValue::Undefined)
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(text: &str) -> VarScope {
        VarScope::from_json_text(text).unwrap()
    }

    #[test]
    fn get_converts_numbers_like_st() {
        let vars = scope(r#"{"a":" 007 ","b":"  ","c":"x","d":true,"e":null,"f":[3],"g":{"k":1}}"#);
        assert_eq!(vars.get("a", None), JsValue::Num(7.0));
        assert_eq!(vars.get("b", None), JsValue::str("  "));
        assert_eq!(vars.get("c", None), JsValue::str("x"));
        assert_eq!(vars.get("d", None), JsValue::Num(1.0));
        assert_eq!(vars.get("e", None), JsValue::Num(0.0));
        assert_eq!(vars.get("f", None), JsValue::Num(3.0));
        assert_eq!(vars.get("g", None).normalize(), r#"{"k":1}"#);
        assert_eq!(vars.get("missing", None), JsValue::str(""));
    }

    #[test]
    fn indexed_set_and_get() {
        let mut vars = scope("{}");
        vars.set("list", JsValue::str("x"), Some("2")).unwrap();
        assert_eq!(vars.to_json_text(), r#"{"list":"[null,null,\"x\"]"}"#);
        vars.set("map", JsValue::str("1"), Some("甲")).unwrap();
        assert_eq!(vars.get("map", Some("甲")), JsValue::Num(1.0));
        vars.set("n", JsValue::str("5"), None).unwrap();
        vars.set("n", JsValue::str("x"), Some("0")).unwrap();
        assert_eq!(vars.raw("n"), Some(&JsValue::str("5")));
        assert_eq!(vars.set("", JsValue::str("x"), None), Err(Thrown));
    }

    #[test]
    fn add_concatenates_or_pushes() {
        let mut vars = scope(r#"{"s":"旅","l":"[1]"}"#);
        vars.add("s", JsValue::str("人")).unwrap();
        assert_eq!(vars.raw("s"), Some(&JsValue::str("旅人")));
        vars.add("l", JsValue::Num(2.0)).unwrap();
        assert_eq!(vars.raw("l"), Some(&JsValue::str("[1,2]")));
        assert_eq!(
            vars.add("n", JsValue::str("2.5")).unwrap(),
            JsValue::Num(2.5)
        );
    }

    #[test]
    fn replay_reproduces_the_same_table() {
        let mut first = Variables::new(scope(r#"{"hp":"10"}"#), scope("{}"));
        first
            .add(ScopeKind::Local, "hp", JsValue::Num(-3.0))
            .unwrap();
        first
            .set(ScopeKind::Global, "g", JsValue::str("1"), None)
            .unwrap();
        first.del(ScopeKind::Local, "gone");
        let mut latest = Variables::new(scope(r#"{"hp":"20","other":1}"#), scope("{}"));
        latest.replay(&first.ops);
        assert_eq!(latest.local.to_json_text(), r#"{"hp":17,"other":1}"#);
        assert_eq!(latest.global.to_json_text(), r#"{"g":"1"}"#);
    }

    #[test]
    fn failed_writes_are_not_recorded() {
        let mut vars = Variables::new(scope(r#"{"x":"bad json","n":"Infinity"}"#), scope("{}"));
        vars.set(ScopeKind::Local, "x", JsValue::str("v"), Some("0"))
            .unwrap();
        assert_eq!(vars.local.raw("x"), Some(&JsValue::str("bad json")));
        // Infinity + -Infinity＝NaN：不寫
        assert_eq!(
            vars.add(ScopeKind::Local, "n", JsValue::Num(f64::NEG_INFINITY))
                .unwrap(),
            JsValue::str("")
        );
        // 超過陣列長度上限
        vars.set(ScopeKind::Local, "big", JsValue::str("v"), Some("1000000"))
            .unwrap();
        assert!(!vars.local.has("big"));
        assert!(vars.ops.is_empty());
        vars.set(ScopeKind::Local, "big", JsValue::str("v"), Some("999999"))
            .unwrap();
        assert_eq!(vars.ops.len(), 1);
    }
}
