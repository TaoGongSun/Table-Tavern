//! ST 內建巨集，照網頁版 `macro-library.ts`（ST 06bde939 macros/definitions/*-macros.js）。
//! 名單涵蓋 `src/shared/contracts/st-macros/st-macros.json`；桌面版沒有的東西（群組、instruct、擴充）
//! 照 ST 在單人 Chat Completion 下的值回（空字串或 false）。

use std::sync::LazyLock;

use regex::Regex;

use super::engine::{
    is_false_boolean, trim_scoped_content, ArgSpec, ArgType, Call, CardField, Handler, MacroDef,
    MacroRegistry, ELSE_MARKER,
};
use super::js_value::{number_to_string, string_hash, text_limit, trim, JsValue};
use super::moment::{format_local, format_utc_offset, humanize};
use super::moment_parse::time_diff;
use super::parser::parse_document;
use super::seedrandom::seedrandom;
use super::variables::{ScopeKind, Thrown};

/// `{{space}}`／`{{newline}}` 次數的實用上限；超過當丟例外（巨集原樣留著）。
const MAX_REPEAT: f64 = 1_000_000.0;

const STR: ArgSpec = ArgSpec {
    types: &[ArgType::String],
    optional: false,
};
const OPTIONAL_STR: ArgSpec = ArgSpec {
    types: &[ArgType::String],
    optional: true,
};
const OPTIONAL_INT: ArgSpec = ArgSpec {
    types: &[ArgType::Integer],
    optional: true,
};
const STR_OR_NUM: ArgSpec = ArgSpec {
    types: &[ArgType::String, ArgType::Number],
    optional: false,
};

fn text(value: impl Into<String>) -> Result<JsValue, Thrown> {
    Ok(JsValue::Str(value.into()))
}

/// 單一參數的舊寫法 `{{random a,b}}`：有 `::` 優先用它切，否則用逗號（`\,` 是字面逗號）。
fn single_arg_list(value: &str) -> Vec<String> {
    if value.contains("::") {
        return value
            .split("::")
            .map(|item| trim(item).to_owned())
            .collect();
    }
    const COMMA: &str = "\u{0}COMMA\u{0}";
    value
        .replace("\\,", COMMA)
        .split(',')
        .map(|item| trim(item).replace(COMMA, ","))
        .collect()
}

fn list_of(call: &Call) -> Vec<String> {
    let list = call.list.clone().unwrap_or_default();
    if list.len() == 1 {
        single_arg_list(&list[0])
    } else {
        list
    }
}

/// droll 子集：`XdY`、`dY`、`XdY±Z`（不分大小寫）。
fn roll_dice(formula: &str, call: &Call) -> String {
    let formula = trim(formula);
    let Some(d_at) = formula.find(['d', 'D']) else {
        return String::new();
    };
    let (count_text, rest) = (&formula[..d_at], &formula[d_at + 1..]);
    let (sides_text, modifier) = match rest.find(['+', '-']) {
        Some(at) => (&rest[..at], Some(&rest[at..])),
        None => (rest, None),
    };
    let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    let modifier_ok = modifier.is_none_or(|m| m.len() > 1 && digits(&m[1..]));
    if !digits(count_text) || sides_text.is_empty() || !digits(sides_text) || !modifier_ok {
        return String::new();
    }
    let number = |part: &str| JsValue::str(part).to_number();
    let count = if count_text.is_empty() {
        1.0
    } else {
        number(count_text)
    };
    let sides = number(sides_text);
    if count < 1.0 || sides < 1.0 || count > 1000.0 {
        return String::new();
    }
    let mut total = modifier.map_or(0.0, number);
    for _ in 0..count as usize {
        total += 1.0 + (call.env.random() * sides).floor();
    }
    number_to_string(total)
}

fn split_on_top_level_else(content: &str) -> Result<(String, Option<String>), Thrown> {
    let mut depth = 0i64;
    for node in parse_document(content).map_err(|_| Thrown)? {
        if node.variable.is_some() {
            continue;
        }
        let closing = node.flags.contains(&'/');
        if node.name == "if" && !closing && node.args.len() == 1 {
            depth += 1;
        } else if node.name == "if" && closing {
            depth -= 1;
        } else if node.name == "else" && depth == 0 {
            return Ok((
                content[..node.start].to_owned(),
                Some(content[node.end..].to_owned()),
            ));
        }
    }
    Ok((content.to_owned(), None))
}

/// `/^([.$])([a-zA-Z](?:[\w-]*\w)?)$/`
fn variable_shorthand(condition: &str) -> Option<(bool, &str)> {
    let global = match condition.as_bytes().first()? {
        b'.' => false,
        b'$' => true,
        _ => return None,
    };
    let name = &condition[1..];
    let bytes = name.as_bytes();
    let word = |byte: &u8| byte.is_ascii_alphanumeric() || *byte == b'_';
    let ok = bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && bytes.iter().all(|byte| word(byte) || *byte == b'-')
        && bytes.last().is_some_and(word);
    ok.then_some((global, name))
}

fn if_macro(call: &Call) -> Result<JsValue, Thrown> {
    let raw_condition = call.arg(0).unwrap_or("");
    let raw_content = call.arg(1).unwrap_or("");
    let leading =
        raw_condition.trim_start_matches(crate::world_info::js_semantics::is_js_whitespace);
    let inverted = leading.starts_with('!');
    let mut condition = if inverted {
        let after = &leading[1..];
        call.resolve(after.trim_start_matches(crate::world_info::js_semantics::is_js_whitespace))
    } else {
        call.resolve(raw_condition)
    };
    if let Some((global, name)) = variable_shorthand(&condition) {
        let getter = if global { "getglobalvar" } else { "getvar" };
        condition = call.resolve(&format!("{{{{{getter}::{name}}}}}"));
    } else if let Some(def) = LIBRARY.get(&condition) {
        if def.args.iter().all(|spec| spec.optional) && !trim(&condition).is_empty() {
            condition = call.resolve(&format!("{{{{{condition}}}}}"));
        }
    }
    let mut falsy = condition.is_empty() || is_false_boolean(&condition);
    if inverted {
        falsy = !falsy;
    }
    let (then, otherwise) = split_on_top_level_else(raw_content)?;
    let chosen = if falsy { otherwise } else { Some(then) };
    let Some(chosen) = chosen else {
        return text("");
    };
    let result = call.resolve(&chosen);
    text(if call.flags.contains(&'#') {
        result
    } else {
        trim_scoped_content(&result)
    })
}

fn last_index(call: &Call, filter: Option<bool>) -> Option<usize> {
    call.env
        .chat
        .iter()
        .rposition(|line| filter.is_none_or(|is_user| line.is_user == is_user))
}

fn line_text(call: &Call, index: Option<usize>) -> Result<JsValue, Thrown> {
    text(
        index
            .and_then(|index| call.env.chat.get(index))
            .map(|line| line.text.clone())
            .unwrap_or_default(),
    )
}

fn repeat(call: &Call, unit: &str) -> Result<JsValue, Thrown> {
    let count = call.arg(0).map_or(1.0, |arg| JsValue::str(arg).to_number());
    // 負數 JS 丟 RangeError；過大的次數以實用上限擋下（比 ST 好：JS 要到字串上限才失敗）
    if !(0.0..=MAX_REPEAT).contains(&count) || count * unit.len() as f64 > text_limit() as f64 {
        return Err(Thrown);
    }
    text(unit.repeat(count as usize))
}
static START_MARK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("(?i-u:<START>)").expect("<START>"));

/// 依 `<START>` 切段（網頁版 `parseMesExamples`）：每段前面補 `<START>\n`、尾端補換行。
pub fn parse_mes_examples(examples: &str) -> Vec<String> {
    if examples.is_empty() || examples == "<START>" {
        return Vec::new();
    }
    let joined = if examples.starts_with("<START>") {
        examples.to_owned()
    } else {
        format!("<START>\n{}", trim(examples))
    };
    START_MARK
        .split(&joined)
        .skip(1)
        .map(|block| format!("<START>\n{}\n", trim(block)))
        .collect()
}

fn field(name: CardField) -> Handler {
    match name {
        CardField::CharPrompt => |c| text(c.env.field(CardField::CharPrompt)),
        CardField::CharInstruction => |c| text(c.env.field(CardField::CharInstruction)),
        CardField::Description => |c| text(c.env.field(CardField::Description)),
        CardField::Personality => |c| text(c.env.field(CardField::Personality)),
        CardField::Scenario => |c| text(c.env.field(CardField::Scenario)),
        CardField::Persona => |c| text(c.env.field(CardField::Persona)),
        CardField::MesExamplesRaw => |c| text(c.env.field(CardField::MesExamplesRaw)),
        CardField::CharDepthPrompt => |c| text(c.env.field(CardField::CharDepthPrompt)),
        CardField::CreatorNotes => |c| text(c.env.field(CardField::CreatorNotes)),
        CardField::FirstMessage => |c| text(c.env.field(CardField::FirstMessage)),
        CardField::Version => |c| text(c.env.field(CardField::Version)),
    }
}

fn def(name: &str, aliases: &[&str], args: &[ArgSpec], handler: Handler) -> MacroDef {
    MacroDef {
        name: name.to_owned(),
        aliases: aliases.iter().map(|alias| (*alias).to_owned()).collect(),
        args: args.to_vec(),
        list: false,
        delay_arg_resolution: false,
        handler,
    }
}

fn list_def(name: &str, handler: Handler) -> MacroDef {
    MacroDef {
        list: true,
        ..def(name, &[], &[], handler)
    }
}

fn scope_of(call: &Call) -> ScopeKind {
    if call.name.to_lowercase().contains("globalvar") {
        ScopeKind::Global
    } else {
        ScopeKind::Local
    }
}

fn variable_macros(global: bool) -> Vec<MacroDef> {
    let name = |base: &str| -> String {
        if global {
            base.replacen("var", "globalvar", 1)
        } else {
            base.to_owned()
        }
    };
    let alias = name;
    vec![
        def(&name("setvar"), &[], &[STR, STR_OR_NUM], |c| {
            let (name, value) = (c.arg(0).unwrap_or(""), c.arg(1).unwrap_or(""));
            c.variables()
                .set(scope_of(c), name, JsValue::str(value), None)?;
            text("")
        }),
        def(&name("addvar"), &[], &[STR, STR_OR_NUM], |c| {
            let (name, value) = (c.arg(0).unwrap_or(""), c.arg(1).unwrap_or(""));
            c.variables().add(scope_of(c), name, JsValue::str(value))?;
            text("")
        }),
        def(&name("incvar"), &[], &[STR], |c| {
            let name = c.arg(0).unwrap_or("");
            c.variables().add(scope_of(c), name, JsValue::Num(1.0))
        }),
        def(&name("decvar"), &[], &[STR], |c| {
            let name = c.arg(0).unwrap_or("");
            c.variables().add(scope_of(c), name, JsValue::Num(-1.0))
        }),
        def(&name("getvar"), &[], &[STR], |c| {
            Ok(c.variables()
                .scope(scope_of(c))
                .get(c.arg(0).unwrap_or(""), None))
        }),
        MacroDef {
            aliases: vec![alias("varexists")],
            ..def(&name("hasvar"), &[], &[STR], |c| {
                let has = c.variables().scope(scope_of(c)).has(c.arg(0).unwrap_or(""));
                text(if has { "true" } else { "false" })
            })
        },
        MacroDef {
            aliases: vec![alias("flushvar")],
            ..def(&name("deletevar"), &[], &[STR], |c| {
                c.variables().del(scope_of(c), c.arg(0).unwrap_or(""));
                text("")
            })
        },
        MacroDef {
            aliases: vec![alias("setvarindex")],
            ..def(
                &name("setvarkey"),
                &[],
                &[STR, STR_OR_NUM, STR_OR_NUM],
                |c| {
                    let (name, index, value) = (
                        c.arg(0).unwrap_or(""),
                        c.arg(1).unwrap_or(""),
                        c.arg(2).unwrap_or(""),
                    );
                    c.variables()
                        .set(scope_of(c), name, JsValue::str(value), Some(index))?;
                    text("")
                },
            )
        },
        MacroDef {
            aliases: vec![alias("getvarindex")],
            ..def(&name("getvarkey"), &[], &[STR, STR_OR_NUM], |c| {
                let (name, index) = (c.arg(0).unwrap_or(""), c.arg(1).unwrap_or(""));
                Ok(c.variables().scope(scope_of(c)).get(name, Some(index)))
            })
        },
    ]
}

fn definitions() -> Vec<MacroDef> {
    let mut defs = vec![
        // ── 名字與卡欄位（env-macros.js）
        def("user", &[], &[], |c| text(c.env.names.user.clone())),
        def("char", &[], &[], |c| text(c.env.names.char.clone())),
        def("group", &["charIfNotGroup"], &[], |c| {
            text(c.env.names.group.clone())
        }),
        def("groupNotMuted", &[], &[], |c| {
            text(c.env.names.group_not_muted.clone())
        }),
        def("notChar", &[], &[], |c| text(c.env.names.not_char.clone())),
        def("charPrompt", &[], &[], field(CardField::CharPrompt)),
        def(
            "charInstruction",
            &[],
            &[],
            field(CardField::CharInstruction),
        ),
        def(
            "charDescription",
            &["description"],
            &[],
            field(CardField::Description),
        ),
        def(
            "charPersonality",
            &["personality"],
            &[],
            field(CardField::Personality),
        ),
        def(
            "charScenario",
            &["scenario"],
            &[],
            field(CardField::Scenario),
        ),
        def("persona", &[], &[], field(CardField::Persona)),
        def("mesExamplesRaw", &[], &[], field(CardField::MesExamplesRaw)),
        def("mesExamples", &[], &[], |c| {
            text(parse_mes_examples(&c.env.field(CardField::MesExamplesRaw)).join(""))
        }),
        def(
            "charDepthPrompt",
            &[],
            &[],
            field(CardField::CharDepthPrompt),
        ),
        def(
            "charCreatorNotes",
            &["creatorNotes"],
            &[],
            field(CardField::CreatorNotes),
        ),
        def("charFirstMessage", &["greeting"], &[OPTIONAL_INT], |c| {
            let index = c.arg(0).map_or(0.0, |arg| JsValue::str(arg).to_number());
            if index == 0.0 {
                return text(c.env.field(CardField::FirstMessage));
            }
            // 網頁版一取 alternateGreetings 就整串代換（副作用照發生），再取第 index-1 個
            let Some(source) = c.env.character else {
                return text("");
            };
            // 整串都求值過了：任何一則讀到私密來源就算（沒被取用的那幾則的副作用也已發生）
            let greetings = source.alternate_greetings();
            c.env
                .mark_private(greetings.iter().any(|greeting| greeting.private));
            let greeting = (index >= 1.0)
                .then(|| greetings.get(index as usize - 1))
                .flatten();
            text(
                greeting
                    .map(|greeting| greeting.text.clone())
                    .unwrap_or_default(),
            )
        }),
        def(
            "charVersion",
            &["version", "char_version"],
            &[],
            field(CardField::Version),
        ),
        def("model", &[], &[], |c| text(c.env.model)),
        def("original", &[], &[], |c| {
            // ST：沒有可代換的原文時呼叫會丟錯，巨集原樣保留
            let Some((original, used)) = &c.env.original else {
                return Err(Thrown);
            };
            c.env.mark_private(original.private);
            if used.replace(true) {
                text("")
            } else {
                text(original.text.clone())
            }
        }),
        def("isMobile", &[], &[], |c| text(c.env.is_mobile.to_string())),
        // ── 對話（chat-macros.js）；沒有 swipe
        def("lastMessage", &[], &[], |c| {
            line_text(c, last_index(c, None))
        }),
        def("lastMessageId", &[], &[], |c| {
            text(
                last_index(c, None)
                    .map(|i| i.to_string())
                    .unwrap_or_default(),
            )
        }),
        def("lastUserMessage", &[], &[], |c| {
            line_text(c, last_index(c, Some(true)))
        }),
        def("lastCharMessage", &[], &[], |c| {
            line_text(c, last_index(c, Some(false)))
        }),
        def("firstIncludedMessageId", &[], &[], |c| {
            text(if c.env.chat.is_empty() { "" } else { "0" })
        }),
        def("firstDisplayedMessageId", &[], &[], |c| {
            text(if c.env.chat.is_empty() { "" } else { "0" })
        }),
        def("lastSwipeId", &[], &[], |_| text("")),
        def("currentSwipeId", &[], &[], |_| text("")),
        def("allChatRange", &[], &[], |c| {
            let len = c.env.chat.len();
            text(if len == 0 {
                String::new()
            } else {
                format!("0-{}", len - 1)
            })
        }),
        // ── 工具（core-macros.js）
        def("space", &[], &[OPTIONAL_INT], |c| repeat(c, " ")),
        def("newline", &[], &[OPTIONAL_INT], |c| repeat(c, "\n")),
        def("noop", &[], &[], |_| text("")),
        def("trim", &[], &[OPTIONAL_STR], |c| {
            if c.is_scoped {
                text(c.arg(0).unwrap_or(""))
            } else {
                text("{{trim}}")
            }
        }),
        MacroDef {
            delay_arg_resolution: true,
            ..def("if", &[], &[STR, STR], if_macro)
        },
        def("else", &[], &[], |_| text(ELSE_MARKER)),
        def("input", &[], &[], |c| text(c.env.input)),
        def("maxPrompt", &["maxPromptTokens"], &[], |c| {
            text(number_to_string(
                c.env.limits.max_context - c.env.limits.max_response,
            ))
        }),
        def("maxContext", &["maxContextTokens"], &[], |c| {
            text(number_to_string(c.env.limits.max_context))
        }),
        def("maxResponse", &["maxResponseTokens"], &[], |c| {
            text(number_to_string(c.env.limits.max_response))
        }),
        def("reverse", &[], &[STR], |c| {
            text(c.arg(0).unwrap_or("").chars().rev().collect::<String>())
        }),
        def("//", &["comment"], &[STR], |_| text("")),
        def("roll", &[], &[STR], |c| {
            let formula = c.arg(0).unwrap_or("");
            let all_digits = !formula.is_empty() && formula.bytes().all(|b| b.is_ascii_digit());
            let formula = if all_digits {
                format!("1d{formula}")
            } else {
                formula.to_owned()
            };
            text(roll_dice(&formula, c))
        }),
        list_def("random", |c| {
            let list = list_of(c);
            if list.is_empty() {
                return text("");
            }
            let index = (c.env.random() * list.len() as f64).floor();
            text(list.get(index as usize).cloned().unwrap_or_default())
        }),
        list_def("pick", |c| {
            let list = list_of(c);
            if list.is_empty() {
                return text("");
            }
            // 照 ST：種子＝hash(對話 id 的 hash-整段內容 hash-位置)
            let seed = string_hash(
                &format!(
                    "{}-{}-{}",
                    number_to_string(string_hash(c.env.chat_id, 0)),
                    number_to_string(c.env.content_hash),
                    c.global_offset
                ),
                0,
            );
            let roll = seedrandom(&number_to_string(seed))();
            let index = (roll * list.len() as f64).floor() as usize;
            text(list.get(index).cloned().unwrap_or_default())
        }),
        def("banned", &[], &[STR], |_| text("")),
        def("outlet", &[], &[STR], |c| {
            let name = c.arg(0).unwrap_or("");
            match c.env.outlets.get(name).filter(|_| !name.is_empty()) {
                Some(outlet) => {
                    if let Some(reads) = c.env.outlet_reads {
                        reads.borrow_mut().insert(name.to_owned());
                    }
                    c.env.mark_private(outlet.private);
                    text(outlet.text.clone())
                }
                None => text(""),
            }
        }),
        // ── 時間（time-macros.js）
        def("time", &[], &[OPTIONAL_STR], |c| {
            let offset = c.arg(0).and_then(|arg| {
                let rest = arg.strip_prefix("UTC")?;
                let digits = rest.strip_prefix(['+', '-'])?;
                (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
                    .then(|| JsValue::str(rest).to_number())
            });
            text(match offset {
                Some(offset) => format_utc_offset(c.env.clock, offset, "LT"),
                None => format_local(c.env.clock, "LT"),
            })
        }),
        def("date", &[], &[], |c| text(format_local(c.env.clock, "LL"))),
        def("weekday", &[], &[], |c| {
            text(format_local(c.env.clock, "dddd"))
        }),
        def("isotime", &[], &[], |c| {
            text(format_local(c.env.clock, "HH:mm"))
        }),
        def("isodate", &[], &[], |c| {
            text(format_local(c.env.clock, "YYYY-MM-DD"))
        }),
        def("datetimeformat", &[], &[STR], |c| {
            text(format_local(c.env.clock, c.arg(0).unwrap_or("")))
        }),
        def("idleDuration", &["idle_duration"], &[], |c| {
            let mut take_next = false;
            for line in c.env.chat.iter().rev() {
                if line.is_user && take_next {
                    return text(match line.sent_at {
                        Some(sent_at) => humanize(c.env.clock.now_ms() - sent_at, false),
                        None => "just now".to_owned(),
                    });
                }
                take_next = true;
            }
            text("just now")
        }),
        def("timeDiff", &[], &[STR, STR], |c| {
            text(time_diff(
                c.env.clock,
                c.arg(0).unwrap_or(""),
                c.arg(1).unwrap_or(""),
            ))
        }),
        // ── 狀態（state-macros.js）
        def("lastGenerationType", &[], &[], |c| {
            text(c.env.generation_type)
        }),
        def("hasExtension", &[], &[STR], |_| text("false")),
    ];
    defs.extend(variable_macros(false));
    defs.extend(variable_macros(true));
    defs
}

pub static LIBRARY: LazyLock<MacroRegistry> = LazyLock::new(|| {
    let mut registry = MacroRegistry::default();
    for def in definitions() {
        registry.register(def);
    }
    registry
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mes_examples_split_on_start() {
        assert_eq!(
            parse_mes_examples("{{user}}: 嗨\n<start>\nB"),
            ["<START>\n{{user}}: 嗨\n", "<START>\nB\n"]
        );
        assert!(parse_mes_examples("<START>").is_empty());
    }

    #[test]
    fn variable_shorthand_needs_word_end() {
        assert_eq!(variable_shorthand(".hp"), Some((false, "hp")));
        assert_eq!(variable_shorthand("$a-b"), Some((true, "a-b")));
        assert_eq!(variable_shorthand(".a-"), None);
    }
}
