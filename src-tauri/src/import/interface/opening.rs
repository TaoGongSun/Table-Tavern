//! 開場白美化腳本：卡片作者預期只用來把開場白裡的一行字換成選單或說明的顯示腳本。這種腳本不規定
//! 模型每回合的回覆格式，GM 回合指示不因它讓路給「卡片自帶介面」。只認得出的形狀才算，
//! 判不出的一律回 false（維持原判定：當成會作用在回覆上的腳本）。

use super::InterfaceScript;

/// 判定（全部成立才算開場白美化）：
/// 1. find_regex 是純字面：`/…/flags` 外殼的 flags 只能是 g i m s u y（i 時大小寫不分比對）；
///    去掉首 `^`、尾 `$` 與首尾任意個 `\s*`／`\s+` 後，不得剩任何未跳脫的元字元 `. ^ $ * + ? ( ) [ ] { } |`；
///    跳脫只認 `\` 後接非英數字（`\.`、`\/` 之類，取字面），`\d`、`\n`、`\u…` 等一律判不出；去完是空字串也判不出。
/// 2. 字面不像格式標籤或佔位符：含 `< > { } [ ] 【 】` 任一個就不算（`<Status>`、`{{…}}`、`【開局】` 這類
///    可能同時出現在開場白與每回合格式裡）。
/// 3. 換成的內容不是前端：含 `<script`，或含 HTML 標籤且長度達 1000 字元，就不算。
/// 4. 字面等於這張卡某則開場白（first_mes 或 alternate_greetings）裡完整的一行（前後空白不計）——
///    只是開場白裡的一個詞（例如代名詞替換）不算。
pub(super) fn is_opening_decoration(script: &InterfaceScript, greetings: &[&str]) -> bool {
    let Some((literal, ignore_case)) = pure_literal(&script.find_regex) else {
        return false;
    };
    if literal
        .chars()
        .any(|c| matches!(c, '<' | '>' | '{' | '}' | '[' | ']' | '【' | '】'))
    {
        return false;
    }
    if looks_like_frontend(&script.replace_string) {
        return false;
    }
    let fold = |text: &str| match ignore_case {
        true => text.to_lowercase(),
        false => text.to_owned(),
    };
    let literal = fold(&literal);
    greetings
        .iter()
        .flat_map(|greeting| greeting.lines())
        .any(|line| fold(line.trim()) == literal)
}

/// 純字面的本文與「是否不分大小寫」；判不出回 None。
fn pure_literal(find_regex: &str) -> Option<(String, bool)> {
    let (body, flags) = match find_regex
        .strip_prefix('/')
        .and_then(|rest| rest.rfind('/').map(|end| (&rest[..end], &rest[end + 1..])))
    {
        Some((body, flags)) => (body, flags),
        None => (find_regex, ""),
    };
    if !flags.chars().all(|flag| "gimsuy".contains(flag)) {
        return None;
    }
    // 切成記號：跳脫（true, 字元）或一般字元（false, 字元）
    let mut tokens = Vec::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => tokens.push((true, chars.next()?)),
            other => tokens.push((false, other)),
        }
    }
    let mut tokens = tokens.as_slice();
    if let [(false, '^'), rest @ ..] = tokens {
        tokens = rest;
    }
    if let [rest @ .., (false, '$')] = tokens {
        tokens = rest;
    }
    while let [(true, 's'), (false, '*' | '+'), rest @ ..] = tokens {
        tokens = rest;
    }
    while let [rest @ .., (true, 's'), (false, '*' | '+')] = tokens {
        tokens = rest;
    }
    let mut literal = String::new();
    for &(escaped, c) in tokens {
        match escaped {
            true if c.is_alphanumeric() => return None,
            false if ".^$*+?()[]{}|".contains(c) => return None,
            _ => literal.push(c),
        }
    }
    let literal = literal.trim().to_owned();
    (!literal.is_empty()).then(|| (literal, flags.contains('i')))
}

fn looks_like_frontend(replace: &str) -> bool {
    let lower = replace.to_ascii_lowercase();
    if lower.contains("<script") {
        return true;
    }
    let has_tag = lower.match_indices('<').any(|(index, _)| {
        lower[index + 1..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '/' || c == '!')
    });
    has_tag && replace.chars().count() >= 1000
}
