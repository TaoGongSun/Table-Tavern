//! ST 巨集求值，照網頁版 `macro-engine.ts`（ST 06bde939 MacroEngine／MacroCstWalker／MacroRegistry）：
//! 參數先求值（內層先）、成對的 `{{x}}…{{/x}}` 把中間當最後一個參數、不認得的巨集原樣留著（內層已換）、
//! 參數個數或型別不合也原樣留著；前處理換舊式 `<USER>` 等標記，後處理還原 `\{`、處理 `{{trim}}`。

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use regex::Regex;

use super::js_value::{text_limit, trim, utf16_len, JsValue};
use super::moment::Clock;
use super::parser::{parse_document, Abort, MacroNode, Range, MAX_DEPTH};
use super::variables::{ScopeKind, Thrown, Variables};

pub const ELSE_MARKER: &str = "\u{0}\u{1F}ELSE\u{1F}\u{0}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgType {
    String,
    Integer,
    Number,
}

#[derive(Debug, Clone, Copy)]
pub struct ArgSpec {
    pub types: &'static [ArgType],
    pub optional: bool,
}

pub type Handler = fn(&Call) -> Result<JsValue, Thrown>;

pub struct MacroDef {
    pub name: String,
    pub aliases: Vec<String>,
    pub args: Vec<ArgSpec>,
    pub list: bool,
    pub delay_arg_resolution: bool,
    pub handler: Handler,
}

/// 文字與它是不是讀自私密來源（`private_md`、私密觸發或限定條目）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SourceText {
    pub text: String,
    pub private: bool,
}

impl SourceText {
    pub fn public(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            private: false,
        }
    }

    #[cfg(test)]
    pub fn private(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            private: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatLine {
    pub is_user: bool,
    pub text: String,
    /// 送出時間（epoch 毫秒）；`{{idle_duration}}` 用
    pub sent_at: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardField {
    CharPrompt,
    CharInstruction,
    Description,
    Personality,
    Scenario,
    Persona,
    MesExamplesRaw,
    CharDepthPrompt,
    CreatorNotes,
    FirstMessage,
    Version,
}

/// 卡欄位巨集的來源（用到才代換，見 substitute.rs）。
pub trait CharacterSource {
    fn field(&self, field: CardField) -> SourceText;
    /// 全部替代開場白（網頁版一次代換整串）
    fn alternate_greetings(&self) -> &[SourceText];
}

/// `[0,1)` 亂數來源（網頁版 `env.random`）。
pub trait RandomSource {
    fn next_random(&self) -> f64;
}

impl<F: FnMut() -> f64> RandomSource for RefCell<F> {
    fn next_random(&self) -> f64 {
        (self.borrow_mut())()
    }
}

#[derive(Debug, Clone)]
pub struct Names {
    pub user: String,
    pub char: String,
    pub group: String,
    pub group_not_muted: String,
    pub not_char: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Limits {
    pub max_context: f64,
    pub max_response: f64,
}

/// 一次求值看得到的世界（網頁版 `MacroEnv`）＋私密來源旗標。
pub struct MacroEnv<'a> {
    pub content_hash: f64,
    pub names: Names,
    /// `None`＝卡欄位巨集不代入（ST baseChatReplace）
    pub character: Option<&'a dyn CharacterSource>,
    pub model: &'a str,
    /// `{{original}}`：只有第一次給原文，之後空字串；`None`＝沒有原文（巨集原樣留著）
    pub original: Option<(&'a SourceText, Cell<bool>)>,
    pub chat: &'a [ChatLine],
    pub variables: &'a RefCell<Variables>,
    pub input: &'a str,
    pub generation_type: &'a str,
    pub limits: Limits,
    pub clock: &'a dyn Clock,
    pub chat_id: &'a str,
    pub random: &'a dyn RandomSource,
    pub outlets: &'a BTreeMap<String, SourceText>,
    /// 有給就記下讀過的 outlet 名稱
    pub outlet_reads: Option<&'a RefCell<std::collections::BTreeSet<String>>>,
    pub is_mobile: bool,
    /// 這次求值讀到了私密來源
    pub private: Cell<bool>,
}

impl MacroEnv<'_> {
    pub fn random(&self) -> f64 {
        self.random.next_random()
    }

    pub fn mark_private(&self, private: bool) {
        if private {
            self.private.set(true);
        }
    }

    pub fn field(&self, field: CardField) -> String {
        match self.character {
            Some(source) => {
                let value = source.field(field);
                self.mark_private(value.private);
                value.text
            }
            None => String::new(),
        }
    }
}

pub struct Call<'c, 'e> {
    pub name: &'c str,
    pub unnamed: Vec<String>,
    pub list: Option<Vec<String>>,
    pub flags: &'c [char],
    pub is_scoped: bool,
    /// UTF-16 位置（`{{pick}}` 的種子用）
    pub global_offset: usize,
    pub env: &'c MacroEnv<'e>,
    depth: usize,
}

impl Call<'_, '_> {
    /// 在同一個環境下對一段文字完整求值（含前後處理），給 if 這類延後求值的巨集用。
    pub fn resolve(&self, text: &str) -> String {
        evaluate_at(text, self.env, self.global_offset, self.depth + 1)
    }

    pub fn arg(&self, index: usize) -> Option<&str> {
        self.unnamed.get(index).map(String::as_str)
    }

    pub fn variables(&self) -> std::cell::RefMut<'_, Variables> {
        self.env.variables.borrow_mut()
    }
}

#[derive(Default)]
pub struct MacroRegistry {
    defs: HashMap<String, usize>,
    all: Vec<MacroDef>,
}

impl MacroRegistry {
    pub fn register(&mut self, def: MacroDef) {
        let index = self.all.len();
        self.defs.insert(def.name.to_lowercase(), index);
        for alias in &def.aliases {
            self.defs.insert(alias.to_lowercase(), index);
        }
        self.all.push(def);
    }

    pub fn get(&self, name: &str) -> Option<&MacroDef> {
        self.defs
            .get(&trim(name).to_lowercase())
            .map(|index| &self.all[*index])
    }
}

pub fn arg_bounds(args: &[ArgSpec]) -> (usize, usize) {
    let min = args
        .iter()
        .position(|spec| spec.optional)
        .unwrap_or(args.len());
    (min, args.len())
}

pub fn is_false_boolean(value: &str) -> bool {
    matches!(trim(value).to_lowercase().as_str(), "off" | "false" | "0")
}

/// `/^-?\d+$/`
pub fn is_integer_text(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn value_of_type(value: &str, kind: ArgType) -> bool {
    let trimmed = trim(value);
    match kind {
        ArgType::String => true,
        ArgType::Integer => is_integer_text(trimmed),
        ArgType::Number => JsValue::str(trimmed).to_number().is_finite(),
    }
}

/// 行首的 `[ \t]*` 長度
fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

/// 成對巨集裡的內容：去頭尾空白，並以第一行非空白的縮排為準整段退縮排。
pub fn trim_scoped_content(content: &str) -> String {
    if content.is_empty() {
        return String::new();
    }
    let lines: Vec<&str> = content.split('\n').collect();
    let base = lines
        .iter()
        .find(|line| !trim(line).is_empty())
        .map_or(0, |line| indent_of(line));
    if base == 0 {
        return trim(content).to_owned();
    }
    let joined = lines
        .iter()
        .map(|line| {
            if indent_of(line) >= base {
                &line[base..]
            } else {
                line.trim_start_matches(crate::world_info::js_semantics::is_js_whitespace)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    trim(&joined).to_owned()
}

struct Item {
    node: MacroNode,
    keep_raw: bool,
    /// 成對內容的範圍與收尾巨集的結尾
    scoped: Option<(Range, usize)>,
}

static TIME_UTC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i-u:\{\{time_(UTC[+-][0-9]+)\}\})").expect("time_UTC"));
static LEGACY_MARKERS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        ("<USER>", "{{user}}"),
        ("<BOT>", "{{char}}"),
        ("<CHAR>", "{{char}}"),
        ("<GROUP>", "{{group}}"),
        ("<CHARIFNOTGROUP>", "{{charIfNotGroup}}"),
    ]
    .into_iter()
    .map(|(marker, macro_text)| {
        (
            Regex::new(&format!("(?i-u:{marker})")).expect("legacy marker"),
            macro_text,
        )
    })
    .collect()
});
static ESCAPED_BRACE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\([{}])").expect("escaped brace"));
static TRIM_MARK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i-u:(?:\r?\n)*\{\{trim\}\}(?:\r?\n)*)").expect("trim mark"));

/// 一段輸入完整求值（前處理、剖析求值、後處理）；剖析或求值放棄時原文照回。
pub fn evaluate(input: &str, env: &MacroEnv) -> String {
    evaluate_at(input, env, 0, 0)
}

fn evaluate_at(input: &str, env: &MacroEnv, offset: usize, depth: usize) -> String {
    if input.is_empty() {
        return String::new();
    }
    let mut pre = TIME_UTC
        .replace_all(input, |caps: &regex::Captures| {
            format!("{{{{time::{}}}}}", &caps[1])
        })
        .into_owned();
    for (marker, replacement) in LEGACY_MARKERS.iter() {
        pre = marker
            .replace_all(&pre, regex::NoExpand(replacement))
            .into_owned();
    }
    let Ok(result) = evaluate_text(&pre, offset, env, depth) else {
        return input.to_owned();
    };
    let result = ESCAPED_BRACE.replace_all(&result, "$1");
    let result = TRIM_MARK.replace_all(&result, "");
    result.split(ELSE_MARKER).collect()
}

/// 只含「靜態巨集」（方案三之 3 的穩定定義）：`{{user}}`、`{{newline}}`、`{{trim}}`、`{{noop}}` 與註解，
/// `allow_char` 再加 `{{char}}`（卡片公開設定以該卡自己的名字代換）。帶參數、旗標、變數簡寫、跳脫大括號
/// （代換後會變成新的巨集）或剖析不了的一律不算。
pub fn is_static(text: &str, allow_char: bool) -> bool {
    if TIME_UTC.is_match(text) || ESCAPED_BRACE.is_match(text) {
        return false;
    }
    let mut pre = text.to_owned();
    for (marker, replacement) in LEGACY_MARKERS.iter() {
        pre = marker
            .replace_all(&pre, regex::NoExpand(replacement))
            .into_owned();
    }
    let Ok(nodes) = parse_document(&pre) else {
        return false;
    };
    nodes.iter().all(|node| {
        if node.variable.is_some() {
            return false;
        }
        let name = node.name.to_lowercase();
        if name == "//" {
            return true;
        }
        node.flags.is_empty()
            && node.args.is_empty()
            && (matches!(name.as_str(), "user" | "newline" | "trim" | "noop")
                || (allow_char && name == "char"))
    })
}

/// 一段文字當成獨立文件剖析求值（參數、成對內容都走這支）。
fn evaluate_text(text: &str, offset: usize, env: &MacroEnv, depth: usize) -> Result<String, Abort> {
    if text.is_empty() {
        return Ok(String::new());
    }
    if depth > MAX_DEPTH {
        return Err(Abort);
    }
    let items = pair_scopes(parse_document(text)?);
    let mut result = String::new();
    // 每段追加前先算合計長度：超過實用上限整段放棄、原文照回（防止一連串巨集把記憶體吃到 GB 級）
    let append = |result: &mut String, piece: &str| -> Result<(), Abort> {
        if result.len() + piece.len() > text_limit() {
            return Err(Abort);
        }
        result.push_str(piece);
        Ok(())
    };
    let mut cursor = 0;
    for item in items {
        append(&mut result, &text[cursor..item.node.start])?;
        if item.keep_raw {
            append(&mut result, &text[item.node.start..item.node.end])?;
            cursor = item.node.end;
        } else {
            let value = evaluate_node(
                &item.node,
                text,
                offset,
                env,
                item.scoped.map(|(range, _)| range),
                depth,
            )?;
            append(&mut result, &value)?;
            cursor = item
                .scoped
                .map_or(item.node.end, |(_, closing_end)| closing_end);
        }
    }
    append(&mut result, &text[cursor..])?;
    Ok(result)
}

fn info(node: &MacroNode) -> Option<(String, bool)> {
    if node.variable.is_some() || node.name.is_empty() {
        return None;
    }
    let closing =
        node.flags.contains(&'/') || (node.name == "//" && node.first_arg_char == Some('/'));
    Some((node.name.to_lowercase(), closing))
}

fn can_take_scope(node: &MacroNode) -> bool {
    let Some(def) = super::library::LIBRARY.get(&node.name) else {
        return true;
    };
    if def.list {
        return false;
    }
    let (min, max) = arg_bounds(&def.args);
    let count = node.args.len() + 1;
    count >= min && count <= max
}

/// MacroCstWalker `#processScopedMacros`：只配最外層，內層等內容重剖時再配。
fn pair_scopes(nodes: Vec<MacroNode>) -> Vec<Item> {
    let mut items: Vec<Item> = nodes
        .into_iter()
        .map(|node| Item {
            node,
            keep_raw: false,
            scoped: None,
        })
        .collect();
    struct Info {
        index: usize,
        name: String,
        closing: bool,
        matched: bool,
    }
    let mut infos: Vec<Info> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            info(&item.node).map(|(name, closing)| Info {
                index,
                name,
                closing,
                matched: false,
            })
        })
        .collect();
    let mut inside = vec![false; items.len()];
    let mut remove = vec![false; items.len()];
    for i in 0..infos.len() {
        if infos[i].closing || infos[i].matched || inside[infos[i].index] {
            continue;
        }
        let mut depth = 1;
        let mut close_at = None;
        for j in i + 1..infos.len() {
            if infos[j].name != infos[i].name || infos[j].matched {
                continue;
            }
            if infos[j].closing {
                depth -= 1;
                if depth == 0 {
                    close_at = Some(j);
                    break;
                }
            } else if can_take_scope(&items[infos[j].index].node) {
                depth += 1;
            }
        }
        let Some(close_at) = close_at else {
            continue;
        };
        infos[i].matched = true;
        infos[close_at].matched = true;
        let (open_index, close_index) = (infos[i].index, infos[close_at].index);
        if !can_take_scope(&items[open_index].node) {
            items[open_index].keep_raw = true;
            items[close_index].keep_raw = true;
            continue;
        }
        let range = Range {
            start: items[open_index].node.end,
            end: items[close_index].node.start,
        };
        items[open_index].scoped = Some((range, items[close_index].node.end));
        for k in open_index + 1..=close_index {
            if k < close_index {
                inside[k] = true;
            }
            remove[k] = true;
        }
    }
    for info in &infos {
        if info.closing && !info.matched {
            items[info.index].keep_raw = true;
        }
    }
    items
        .into_iter()
        .enumerate()
        .filter(|(index, _)| !remove[*index])
        .map(|(_, item)| item)
        .collect()
}

fn evaluate_node(
    node: &MacroNode,
    text: &str,
    offset: usize,
    env: &MacroEnv,
    scoped: Option<Range>,
    depth: usize,
) -> Result<String, Abort> {
    if node.variable.is_some() {
        return evaluate_variable(node, text, offset, env, depth);
    }
    let library = &*super::library::LIBRARY;
    let def = library.get(&node.name);
    let delay = def.is_some_and(|def| def.delay_arg_resolution);
    let at = |position: usize| offset + utf16_len(&text[..position]);
    let mut args: Vec<String> = Vec::new();
    let mut placed: Vec<(Range, String)> = Vec::new();
    for range in &node.args {
        let Some(range) = range else {
            args.push(String::new());
            continue;
        };
        let raw = &text[range.start..range.end];
        let value = if delay {
            raw.to_owned()
        } else {
            evaluate_text(raw, at(range.start), env, depth + 1)?
        };
        args.push(value.clone());
        placed.push((*range, value));
    }
    if let Some(scope) = scoped {
        if scope.start >= scope.end {
            args.push(String::new());
        } else {
            let raw = &text[scope.start..scope.end];
            let mut value = if delay {
                raw.to_owned()
            } else {
                evaluate_text(raw, at(scope.start), env, depth + 1)?
            };
            if !delay && !node.flags.contains(&'#') {
                value = trim_scoped_content(&value);
            }
            args.push(value.clone());
            placed.push((scope, value));
        }
    }
    placed.sort_by_key(|(range, _)| range.start);
    let mut raw_inner = String::new();
    let mut cursor = node.inner_start;
    for (range, value) in &placed {
        if range.start > cursor {
            raw_inner.push_str(&text[cursor..range.start]);
        }
        raw_inner.push_str(value);
        cursor = range.end;
    }
    if cursor < node.inner_end {
        raw_inner.push_str(&text[cursor..node.inner_end]);
    }
    let raw = format!("{{{{{raw_inner}}}}}");

    let Some(def) = def else {
        return Ok(raw);
    };
    let (min, max) = arg_bounds(&def.args);
    let valid = if def.list {
        args.len() >= min
    } else {
        args.len() >= min && args.len() <= max
    };
    if !valid {
        return Ok(raw);
    }
    let unnamed: Vec<String> = args[..args.len().min(max)].to_vec();
    for (spec, value) in def.args.iter().zip(&unnamed) {
        if !spec.types.iter().any(|kind| value_of_type(value, *kind)) {
            return Ok(raw);
        }
    }
    let list = def.list.then(|| {
        if args.len() > max {
            args[max..].to_vec()
        } else {
            Vec::new()
        }
    });
    let call = Call {
        name: &def.name,
        unnamed,
        list,
        flags: &node.flags,
        is_scoped: scoped.is_some(),
        global_offset: at(node.start),
        env,
        depth,
    };
    Ok(match (def.handler)(&call) {
        Ok(value) => value.normalize(),
        Err(Thrown) => raw,
    })
}

fn evaluate_variable(
    node: &MacroNode,
    text: &str,
    offset: usize,
    env: &MacroEnv,
    depth: usize,
) -> Result<String, Abort> {
    let variable = node.variable.as_ref().expect("變數節點");
    let kind = if variable.global {
        ScopeKind::Global
    } else {
        ScopeKind::Local
    };
    let name = variable.name.as_str();
    let mut cached: Option<String> = None;
    let mut value = |cached: &mut Option<String>| -> Result<String, Abort> {
        if cached.is_none() {
            *cached = Some(match variable.value {
                Some(range) => trim(&evaluate_text(
                    &text[range.start..range.end],
                    offset + utf16_len(&text[..range.start]),
                    env,
                    depth + 1,
                )?)
                .to_owned(),
                None => String::new(),
            });
        }
        Ok(cached.clone().expect("剛填好"))
    };
    let get = || env.variables.borrow().scope(kind).get(name, None);
    let has = || env.variables.borrow().scope(kind).has(name);
    let falsy = |current: &JsValue| !current.truthy() || is_false_boolean(&current.normalize());
    let numeric = |cached: &mut Option<String>,
                   value: &mut dyn FnMut(&mut Option<String>) -> Result<String, Abort>,
                   compare: fn(f64, f64) -> bool|
     -> Result<String, Abort> {
        let a = get().to_number();
        let b = JsValue::str(value(cached)?).to_number();
        Ok(if a.is_nan() || b.is_nan() {
            "false".to_owned()
        } else {
            compare(a, b).to_string()
        })
    };
    let set = |text: String| {
        let _ = env
            .variables
            .borrow_mut()
            .set(kind, name, JsValue::Str(text), None);
    };
    let add = |amount: JsValue| -> JsValue {
        env.variables
            .borrow_mut()
            .add(kind, name, amount)
            .unwrap_or(JsValue::Undefined)
    };
    Ok(match variable.operator {
        None => get().normalize(),
        Some("=") => {
            let v = value(&mut cached)?;
            set(v);
            String::new()
        }
        Some("++") => add(JsValue::Num(1.0)).normalize(),
        Some("--") => add(JsValue::Num(-1.0)).normalize(),
        Some("+=") => {
            let v = value(&mut cached)?;
            add(JsValue::Str(v));
            String::new()
        }
        Some("-=") => {
            let amount = JsValue::str(value(&mut cached)?).to_number();
            if !amount.is_nan() {
                add(JsValue::Num(-amount));
            }
            String::new()
        }
        Some("||") => {
            let current = get();
            if falsy(&current) {
                value(&mut cached)?
            } else {
                current.normalize()
            }
        }
        Some("??") => {
            if has() {
                get().normalize()
            } else {
                value(&mut cached)?
            }
        }
        Some("||=") => {
            let current = get();
            if !falsy(&current) {
                current.normalize()
            } else {
                let v = value(&mut cached)?;
                set(v.clone());
                v
            }
        }
        Some("??=") => {
            if has() {
                get().normalize()
            } else {
                let v = value(&mut cached)?;
                set(v.clone());
                v
            }
        }
        Some("==") => {
            let current = get().normalize();
            (current == value(&mut cached)?).to_string()
        }
        Some("!=") => {
            let current = get().normalize();
            (current != value(&mut cached)?).to_string()
        }
        Some(">") => numeric(&mut cached, &mut value, |a, b| a > b)?,
        Some(">=") => numeric(&mut cached, &mut value, |a, b| a >= b)?,
        Some("<") => numeric(&mut cached, &mut value, |a, b| a < b)?,
        Some("<=") => numeric(&mut cached, &mut value, |a, b| a <= b)?,
        Some(_) => String::new(),
    })
}
