//! 世界書正則鍵：ST `parseRegexFromString`（`/樣式/旗標`）＋ JS 正則 → fancy-regex 轉譯。
//! 轉譯是逐字元的 JS 正則分詞，不是字串取代：已跳脫的字元與字元類別內各有規則，並記住前一個記號能不能接量詞
//! （JS 會報 SyntaxError 的寫法轉譯失敗＝當一般字串）。對照表與已知差異見
//! .ai/plans/worldbook-st-trigger-parity.md 三之 1、七；每一條都在 regex-cases.json。

use fancy_regex::{Regex, RegexBuilder};

const WORD: &str = r"A-Za-z0-9_";
const DIGIT: &str = r"0-9";
/// JS `\s` 的集合（有無 `u` 都相同）
const SPACE: &str = r"\t\n\x0B\x0C\r\x20\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}";
/// JS 的行終止字元（`.` 不吃、`m` 下的 `^`／`$` 認）
const LINE_TERMINATORS: &str = r"\n\r\x{2028}\x{2029}";
/// 回溯上限：超過算不命中（網頁版不擋 ReDoS，桌面版卡住的是後端執行緒）
const BACKTRACK_LIMIT: usize = 1_000_000;

#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    i: bool,
    m: bool,
    s: bool,
    u: bool,
    y: bool,
}

fn parse_flags(text: &str) -> Option<Flags> {
    let mut flags = Flags::default();
    let mut seen = String::new();
    for ch in text.chars() {
        if seen.contains(ch) {
            return None; // 重複旗標：new RegExp 會丟錯
        }
        seen.push(ch);
        match ch {
            'g' => {}
            'i' => flags.i = true,
            'm' => flags.m = true,
            's' => flags.s = true,
            'u' => flags.u = true,
            'y' => flags.y = true,
            _ => return None,
        }
    }
    Some(flags)
}

/// `^\/([\w\W]+?)\/([gimsuy]*)$`：樣式取最短、後面只剩旗標的那個 `/`。
fn split_literal(input: &str) -> Option<(&str, &str)> {
    let body = input.strip_prefix('/')?;
    let mut chars = body.char_indices();
    chars.next()?; // 樣式至少一個字元
    for (at, ch) in chars {
        if ch == '/' && body[at + 1..].chars().all(|flag| "gimsuy".contains(flag)) {
            return Some((&body[..at], &body[at + 1..]));
        }
    }
    None
}

/// ST `parseRegexFromString`；回 `None` 表示這個鍵當一般字串比對。
pub fn parse_regex_from_string(input: &str) -> Option<Regex> {
    let (pattern, flags) = split_literal(input)?;
    // 樣式裡沒跳脫的 `/`（含開頭）就不是正則
    let chars: Vec<char> = pattern.chars().collect();
    if chars
        .iter()
        .enumerate()
        .any(|(index, ch)| *ch == '/' && (index == 0 || chars[index - 1] != '\\'))
    {
        return None;
    }
    let pattern = pattern.replacen("\\/", "/", 1);
    let flags = parse_flags(flags)?;
    let translated = translate(&pattern, flags).ok()?;
    RegexBuilder::new(&translated)
        .backtrack_limit(BACKTRACK_LIMIT)
        .build()
        .ok()
}

/// `regex.test(haystack)` 的結果；回溯超限另外回報（呼叫端記住、同一次掃描不再試）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegexOutcome {
    Match,
    NoMatch,
    Exceeded,
}

pub fn regex_outcome(regex: &Regex, haystack: &str) -> RegexOutcome {
    match regex.is_match(haystack) {
        Ok(true) => RegexOutcome::Match,
        Ok(false) => RegexOutcome::NoMatch,
        Err(_) => RegexOutcome::Exceeded,
    }
}

/// `regex.test(haystack)`；回溯超限算不中（對拍測試用）。
#[cfg(test)]
pub fn regex_test(regex: &Regex, haystack: &str) -> bool {
    regex_outcome(regex, haystack) == RegexOutcome::Match
}

/// 前一個記號能不能接量詞。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prev {
    /// 開頭、`(`、`|` 之後：沒有東西可以重複
    Nothing,
    /// 可以接量詞的原子
    Atom,
    /// 剛接過量詞：只能再接一個懶惰的 `?`
    Quantified,
    /// 不帶 u 的前看（輸出從這個位置開始，外面包了一層 `(?:…)`）：可以接量詞，但 fancy-regex 不收重複的斷言，
    /// 量詞改寫掉（見 `quantifier`）
    Lookahead(usize),
    /// 改寫掉的量詞之後：懶惰的 `?` 也一起吞掉
    QuantifiedAway,
    /// 斷言（`^`、`$`、`\b`、`\B`、後看；帶 u 的前看）或懶惰量詞之後：不能接量詞
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Capture,
    Lookahead,
    Lookbehind,
}

struct Translator {
    chars: Vec<char>,
    at: usize,
    flags: Flags,
    /// 捕捉群組的名字（依出現順序，沒名字是 None）
    captures: Vec<Option<String>>,
    out: String,
    prev: Prev,
    /// 開著的群組與它在輸出裡的起點
    groups: Vec<(Group, usize)>,
}

type Step = Result<(), ()>;

fn literal(ch: char) -> String {
    format!("\\x{{{:X}}}", ch as u32)
}

fn ascii_class(set: &str, negated: bool) -> String {
    // (?-i:…)：JS 不帶 u 的 i 不會讓 \w 吃到 ſ、K，Rust 的 (?i) 會
    format!("(?-i:[{}{set}])", if negated { "^" } else { "" })
}

/// 反向參照：群組還沒參與比對（或在參照後面）時 JS 當空字串比中，fancy-regex 的 `\N` 不中，
/// 所以寫成條件式「有捕捉才比，否則空」。
fn backreference(number: usize) -> String {
    format!("(?({number})\\{number})")
}

/// 樣式裡的捕捉群組（依出現順序；具名的記名字），決定 `\數字`、`\k` 的意思。
fn scan_captures(chars: &[char]) -> Vec<Option<String>> {
    let (mut captures, mut in_class, mut index) = (Vec::new(), false, 0);
    while index < chars.len() {
        match chars[index] {
            '\\' => index += 1,
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '(' if !in_class => {
                if chars.get(index + 1) != Some(&'?') {
                    captures.push(None);
                } else if chars.get(index + 2) == Some(&'<')
                    && !matches!(chars.get(index + 3), Some('=') | Some('!'))
                {
                    let name: String = chars[index + 3..]
                        .iter()
                        .take_while(|ch| **ch != '>')
                        .collect();
                    captures.push(Some(name));
                }
            }
            _ => {}
        }
        index += 1;
    }
    captures
}

fn translate(pattern: &str, flags: Flags) -> Result<String, ()> {
    let chars: Vec<char> = pattern.chars().collect();
    let captures = scan_captures(&chars);
    let mut translator = Translator {
        chars,
        at: 0,
        flags,
        captures,
        out: String::new(),
        prev: Prev::Nothing,
        groups: Vec::new(),
    };
    while translator.at < translator.chars.len() {
        translator.token()?;
    }
    if !translator.groups.is_empty() {
        return Err(()); // 沒收尾的群組
    }
    let body = translator.out;
    let mut result = String::new();
    if flags.i {
        result.push_str("(?i)");
    }
    match flags.y {
        true => result.push_str(&format!("\\A(?:{body})")),
        false => result.push_str(&body),
    }
    Ok(result)
}

impl Translator {
    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.at + offset).copied()
    }

    fn hex(&self, from: usize, count: usize) -> Option<u32> {
        let digits: String = self.chars.get(from..from + count)?.iter().collect();
        (digits.len() == count && digits.chars().all(|ch| ch.is_ascii_hexdigit()))
            .then(|| u32::from_str_radix(&digits, 16).ok())
            .flatten()
    }

    /// `{n}`、`{n,}`、`{n,m}`：是量詞就回傳它的長度（字元數）。
    fn quantifier_len(&self) -> Option<usize> {
        let mut index = self.at + 1;
        let digits = |index: &mut usize| {
            let start = *index;
            while self.chars.get(*index).is_some_and(char::is_ascii_digit) {
                *index += 1;
            }
            *index > start
        };
        if !digits(&mut index) {
            return None;
        }
        if self.chars.get(index) == Some(&',') {
            index += 1;
            digits(&mut index);
        }
        (self.chars.get(index) == Some(&'}')).then_some(index + 1 - self.at)
    }

    /// 輸出一段原子並記成可接量詞。
    fn atom(&mut self, text: &str, len: usize) -> Step {
        self.out.push_str(text);
        self.at += len;
        self.prev = Prev::Atom;
        Ok(())
    }

    fn assertion(&mut self, text: &str, len: usize) -> Step {
        self.out.push_str(text);
        self.at += len;
        self.prev = Prev::Fixed;
        Ok(())
    }

    /// 量詞（`*`、`+`、`?`、`{…}`）；接在不能重複的東西後面＝SyntaxError。
    fn quantifier(&mut self, text: &str, len: usize) -> Step {
        let lazy = text == "?" && matches!(self.prev, Prev::Quantified | Prev::QuantifiedAway);
        if lazy {
            if self.prev == Prev::Quantified {
                self.out.push('?');
            }
            self.prev = Prev::Fixed;
            self.at += len;
            return Ok(());
        }
        match self.prev {
            Prev::Atom => {
                self.out.push_str(text);
                self.prev = Prev::Quantified;
            }
            Prev::Lookahead(start) => {
                // 重複零寬的前看（JS RepeatMatcher）：下限 0 時那一輪空比對被丟掉，整個等於空（裡面的捕捉不留）；
                // 下限 ≥ 1 時等於比一次
                let min: usize = match text {
                    "*" | "?" => 0,
                    "+" => 1,
                    braces => braces[1..]
                        .split([',', '}'])
                        .next()
                        .and_then(|digits| digits.parse().ok())
                        .unwrap_or(usize::MAX),
                };
                if min == 0 {
                    // 前看那一支前面放 (?!)：永遠走不進去（回溯也帶不出裡面的捕捉），但群組編號照留
                    let inner = self.out[start + 3..self.out.len() - 1].to_owned();
                    self.out.truncate(start);
                    self.out.push_str(&format!("(?:|(?!){inner})"));
                }
                self.prev = Prev::QuantifiedAway;
            }
            _ => return Err(()),
        }
        self.at += len;
        Ok(())
    }

    fn token(&mut self) -> Step {
        let ch = self.chars[self.at];
        match ch {
            '\\' => self.escape(),
            '[' => self.class(),
            '.' => {
                let text = match self.flags.s {
                    true => "(?s:.)".to_owned(),
                    false => format!("[^{LINE_TERMINATORS}]"),
                };
                self.atom(&text, 1)
            }
            '^' => {
                let text = match self.flags.m {
                    true => format!("(?:\\A|(?<=[{LINE_TERMINATORS}]))"),
                    false => "\\A".to_owned(),
                };
                self.assertion(&text, 1)
            }
            '$' => {
                let text = match self.flags.m {
                    true => format!("(?:\\z|(?=[{LINE_TERMINATORS}]))"),
                    false => "\\z".to_owned(),
                };
                self.assertion(&text, 1)
            }
            '(' => self.open_group(),
            ')' => {
                let (group, start) = self.groups.pop().ok_or(())?;
                self.out.push(')');
                if group == Group::Lookahead && !self.flags.u {
                    self.out.push(')');
                }
                self.at += 1;
                // Annex B：不帶 u 的前看可以接量詞；後看一律不行
                self.prev = match group {
                    Group::Capture => Prev::Atom,
                    Group::Lookahead if !self.flags.u => Prev::Lookahead(start),
                    _ => Prev::Fixed,
                };
                Ok(())
            }
            '|' => {
                self.out.push('|');
                self.at += 1;
                self.prev = Prev::Nothing;
                Ok(())
            }
            '*' | '+' | '?' => self.quantifier(&ch.to_string(), 1),
            '{' => match self.quantifier_len() {
                Some(len) => {
                    let text: String = self.chars[self.at..self.at + len].iter().collect();
                    // `{min,max}` 的 max 小於 min 是 SyntaxError（一般量詞與前看改寫都先擋）
                    let bound = |digits: &str| digits.parse::<u128>().unwrap_or(u128::MAX);
                    let inner = &text[1..text.len() - 1];
                    if let Some((min, max)) = inner.split_once(',') {
                        if !max.is_empty() && bound(max) < bound(min) {
                            return Err(());
                        }
                    }
                    self.quantifier(&text, len)
                }
                None if self.flags.u => Err(()),
                None => self.atom(&literal('{'), 1),
            },
            '}' | ']' if self.flags.u => Err(()),
            other => self.atom(&literal(other), 1),
        }
    }

    fn open_group(&mut self) -> Step {
        if self.peek(1) != Some('?') {
            self.groups.push((Group::Capture, self.out.len()));
            self.out.push('(');
            self.at += 1;
            self.prev = Prev::Nothing;
            return Ok(());
        }
        let rest: String = self.chars[self.at..].iter().take(4).collect();
        let (prefix, group) = if rest.starts_with("(?:") {
            ("(?:", Group::Capture)
        } else if rest.starts_with("(?=") {
            ("(?=", Group::Lookahead)
        } else if rest.starts_with("(?!") {
            ("(?!", Group::Lookahead)
        } else if rest.starts_with("(?<=") {
            ("(?<=", Group::Lookbehind)
        } else if rest.starts_with("(?<!") {
            ("(?<!", Group::Lookbehind)
        } else if rest.starts_with("(?<") {
            let close = self.chars[self.at + 3..]
                .iter()
                .position(|ch| *ch == '>')
                .ok_or(())?;
            let name: String = self.chars[self.at + 3..self.at + 3 + close]
                .iter()
                .collect();
            self.groups.push((Group::Capture, self.out.len()));
            self.out.push_str(&format!("(?P<{name}>"));
            self.at += 4 + close;
            self.prev = Prev::Nothing;
            return Ok(());
        } else {
            return Err(());
        };
        // 不帶 u 的前看可以接量詞（Annex B）：多包一層非捕捉群組，量詞改寫時好整段換掉
        self.groups.push((group, self.out.len()));
        if group == Group::Lookahead && !self.flags.u {
            self.out.push_str("(?:");
        }
        self.out.push_str(prefix);
        self.at += prefix.chars().count();
        self.prev = Prev::Nothing;
        Ok(())
    }

    fn escape(&mut self) -> Step {
        let Some(next) = self.peek(1) else {
            return Err(()); // 結尾的反斜線
        };
        let u = self.flags.u;
        match next {
            'd' => self.atom(&ascii_class(DIGIT, false), 2),
            'D' => self.atom(&ascii_class(DIGIT, true), 2),
            'w' => self.atom(&ascii_class(WORD, false), 2),
            'W' => self.atom(&ascii_class(WORD, true), 2),
            's' => self.atom(&format!("[{SPACE}]"), 2),
            'S' => self.atom(&format!("[^{SPACE}]"), 2),
            'b' => self.assertion(
                &format!("(?-i:(?<=[{WORD}])(?![{WORD}])|(?<![{WORD}])(?=[{WORD}]))"),
                2,
            ),
            'B' => self.assertion(
                &format!("(?-i:(?<=[{WORD}])(?=[{WORD}])|(?<![{WORD}])(?![{WORD}]))"),
                2,
            ),
            '1'..='9' => self.decimal_escape(),
            '0' => {
                if self.peek(2).is_some_and(|ch| ch.is_ascii_digit()) {
                    if u {
                        return Err(());
                    }
                    let (ch, len) = self.octal_at(self.at)?;
                    return self.atom(&literal(ch), len);
                }
                self.atom(&literal('\0'), 2)
            }
            'k' if u || self.captures.iter().any(Option::is_some) => {
                if self.peek(2) != Some('<') {
                    return Err(());
                }
                let close = self.chars[self.at + 3..]
                    .iter()
                    .position(|ch| *ch == '>')
                    .ok_or(())?;
                let name: String = self.chars[self.at + 3..self.at + 3 + close]
                    .iter()
                    .collect();
                let number = self
                    .captures
                    .iter()
                    .position(|capture| capture.as_deref() == Some(name.as_str()))
                    .ok_or(())?
                    + 1;
                self.atom(&backreference(number), 4 + close)
            }
            'p' | 'P' if u => {
                if self.peek(2) != Some('{') {
                    return Err(());
                }
                let close = self.chars[self.at + 3..]
                    .iter()
                    .position(|ch| *ch == '}')
                    .ok_or(())?;
                let body: String = self.chars[self.at + 3..self.at + 3 + close]
                    .iter()
                    .collect();
                self.atom(&format!("\\{next}{{{body}}}"), 4 + close)
            }
            '-' if u => Err(()),
            'c' => match self.peek(2) {
                Some(letter) if letter.is_ascii_alphabetic() => {
                    self.atom(&literal(char::from_u32(letter as u32 % 32).ok_or(())?), 3)
                }
                _ if u => Err(()),
                // Annex B：`\c` 後面不是字母就是字面的反斜線（c 留給下一個記號）
                _ => self.atom(&literal('\\'), 1),
            },
            _ => {
                let (ch, len) = self.char_escape(next)?;
                self.atom(&literal(ch), len)
            }
        }
    }

    /// 代表單一字元的跳脫（類別內外共用；`\c` 各自處理）：回傳字元與整段長度。
    fn char_escape(&self, next: char) -> Result<(char, usize), ()> {
        let u = self.flags.u;
        let control = |code: u32| char::from_u32(code).ok_or(());
        Ok(match next {
            't' => ('\t', 2),
            'n' => ('\n', 2),
            'v' => ('\u{0B}', 2),
            'f' => ('\u{0C}', 2),
            'r' => ('\r', 2),
            'x' => match self.hex(self.at + 2, 2) {
                Some(code) => (control(code)?, 4),
                None if u => return Err(()),
                None => ('x', 2),
            },
            'u' => {
                if u && self.peek(2) == Some('{') {
                    let close = self.chars[self.at + 3..]
                        .iter()
                        .position(|ch| *ch == '}')
                        .ok_or(())?;
                    let digits: String = self.chars[self.at + 3..self.at + 3 + close]
                        .iter()
                        .collect();
                    let code = u32::from_str_radix(&digits, 16).map_err(|_| ())?;
                    return Ok((control(code)?, 4 + close));
                }
                match self.hex(self.at + 2, 4) {
                    Some(high @ 0xD800..=0xDBFF) => {
                        // 代理對：😀 合成一個 code point；落單的代理 Rust 表示不了（已知差異）
                        let low = (self.chars.get(self.at + 6) == Some(&'\\')
                            && self.chars.get(self.at + 7) == Some(&'u'))
                        .then(|| self.hex(self.at + 8, 4))
                        .flatten()
                        .filter(|low| (0xDC00..=0xDFFF).contains(low))
                        .ok_or(())?;
                        let code = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                        (control(code)?, 12)
                    }
                    Some(code) => (control(code)?, 6),
                    None if u => return Err(()),
                    None => ('u', 2),
                }
            }
            '^' | '$' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
            | '/' => (next, 2),
            '-' => ('-', 2),
            _ if u => return Err(()),
            other => (other, 2),
        })
    }

    /// `\1`… 類別外：群組數夠就是反向參照，否則（無 u）是舊式八進位或字面數字。
    fn decimal_escape(&mut self) -> Step {
        let start = self.at + 1;
        let mut end = start;
        while self.chars.get(end).is_some_and(char::is_ascii_digit) {
            end += 1;
        }
        let digits: String = self.chars[start..end].iter().collect();
        let number: usize = digits.parse().unwrap_or(usize::MAX);
        if number <= self.captures.len() {
            return self.atom(&backreference(number), end - self.at);
        }
        if self.flags.u {
            return Err(());
        }
        let (ch, len) = self.octal_at(self.at)?;
        self.atom(&literal(ch), len)
    }

    /// Annex B 舊式八進位（`\8`、`\9` 是字面數字）；只吃八進位那幾位，其餘當一般字元。
    fn octal_at(&self, at: usize) -> Result<(char, usize), ()> {
        let first = self.chars.get(at + 1).copied().ok_or(())?;
        if matches!(first, '8' | '9') {
            return Ok((first, 2));
        }
        let max_digits = if first <= '3' { 3 } else { 2 };
        let mut value = 0u32;
        let mut count = 0;
        while count < max_digits {
            match self.chars.get(at + 1 + count) {
                Some(ch @ '0'..='7') => {
                    value = value * 8 + ch.to_digit(8).expect("octal");
                    count += 1;
                }
                _ => break,
            }
        }
        Ok((char::from_u32(value).ok_or(())?, 1 + count))
    }

    /// 字元類別：`[…]`、`[^…]`；`[]` 永遠不中、`[^]` 任一字元。
    fn class(&mut self) -> Step {
        let start = self.at;
        self.at += 1;
        let negated = self.peek(0) == Some('^');
        if negated {
            self.at += 1;
        }
        let mut items = String::new();
        let mut empty = true;
        loop {
            let Some(ch) = self.peek(0) else {
                return Err(()); // 沒有收尾的 ]
            };
            if ch == ']' {
                self.at += 1;
                break;
            }
            empty = false;
            let first = self.class_atom()?;
            // 範圍：兩端都是單一字元才算；有一端是 \d 之類（無 u）就把 - 當字面
            if self.peek(0) == Some('-') && self.peek(1).is_some_and(|next| next != ']') {
                self.at += 1;
                let last = self.class_atom()?;
                match (&first, &last) {
                    (ClassAtom::Char(low), ClassAtom::Char(high)) => {
                        if low > high {
                            return Err(());
                        }
                        items.push_str(&format!("{}-{}", literal(*low), literal(*high)));
                    }
                    _ if self.flags.u => return Err(()),
                    _ => {
                        items.push_str(&first.render());
                        items.push_str(&literal('-'));
                        items.push_str(&last.render());
                    }
                }
            } else {
                items.push_str(&first.render());
            }
        }
        let text = match (empty, negated) {
            (true, false) => "(?!)".to_owned(),
            (true, true) => "(?s:.)".to_owned(),
            (false, negated) => format!("[{}{items}]", if negated { "^" } else { "" }),
        };
        let len = self.at - start;
        self.at = start;
        self.atom(&text, len)
    }

    /// 類別內的一個元素（Annex B 類別內規則）。
    fn class_atom(&mut self) -> Result<ClassAtom, ()> {
        let ch = self.chars[self.at];
        if ch != '\\' {
            self.at += 1;
            return Ok(ClassAtom::Char(ch));
        }
        let next = self.peek(1).ok_or(())?;
        let set = match next {
            'd' => Some(ClassAtom::Set(DIGIT.to_owned(), false)),
            'D' => Some(ClassAtom::Set(DIGIT.to_owned(), true)),
            'w' => Some(ClassAtom::Set(WORD.to_owned(), false)),
            'W' => Some(ClassAtom::Set(WORD.to_owned(), true)),
            's' => Some(ClassAtom::Set(SPACE.to_owned(), false)),
            'S' => Some(ClassAtom::Set(SPACE.to_owned(), true)),
            _ => None,
        };
        if let Some(set) = set {
            self.at += 2;
            return Ok(set);
        }
        let u = self.flags.u;
        let (value, len) = match next {
            'b' => ('\u{08}', 2),
            '0' if !self.peek(2).is_some_and(|ch| ch.is_ascii_digit()) => ('\0', 2),
            '0'..='9' if u => return Err(()),
            '0'..='9' => self.octal_at(self.at)?,
            'B' if u => return Err(()),
            'B' => ('B', 2),
            'k' if !u => ('k', 2),
            'p' | 'P' if !u => (next, 2),
            // 類別內的 \c：字母一律；不帶 u 時數字與 _ 也算（ClassControlLetter）；其餘是字面的反斜線
            'c' => match self.peek(2) {
                Some(letter) if letter.is_ascii_alphabetic() => {
                    (char::from_u32(letter as u32 % 32).ok_or(())?, 3)
                }
                Some(digit) if !u && (digit.is_ascii_digit() || digit == '_') => {
                    (char::from_u32(digit as u32 % 32).ok_or(())?, 3)
                }
                _ if u => return Err(()),
                _ => ('\\', 1),
            },
            _ => self.char_escape(next)?,
        };
        self.at += len;
        Ok(ClassAtom::Char(value))
    }
}

enum ClassAtom {
    Char(char),
    /// 明列範圍與是否取補集（補集寫成巢狀類別，聯集在外層）
    Set(String, bool),
}

impl ClassAtom {
    fn render(&self) -> String {
        match self {
            ClassAtom::Char(ch) => literal(*ch),
            ClassAtom::Set(set, false) => set.clone(),
            ClassAtom::Set(set, true) => format!("[^{set}]"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backtrack_limit_is_reported_and_counts_as_a_miss() {
        // 反向參照迫使走回溯引擎；(a|a)+ 的指數回溯撞上限
        let regex = parse_regex_from_string("/(a|a)+\\1$/").expect("valid");
        let haystack = format!("{}b", "a".repeat(30));
        assert!(matches!(
            regex.is_match(&haystack),
            Err(fancy_regex::Error::RuntimeError(
                fancy_regex::RuntimeError::BacktrackLimitExceeded
            ))
        ));
        assert_eq!(regex_outcome(&regex, &haystack), RegexOutcome::Exceeded);
        assert!(!regex_test(&regex, &haystack));
    }

    #[test]
    fn a_match_beyond_the_backtrack_limit_counts_as_a_miss() {
        // 短的字串會中（第二個分支）；長的字串第一個分支先把回溯額度用完，本來會中也算不中
        let regex = parse_regex_from_string("/(a|a)+\\1c|z/").expect("valid");
        assert!(regex_test(&regex, "aaabz"));
        let haystack = format!("{}bz", "a".repeat(30));
        assert_eq!(regex_outcome(&regex, &haystack), RegexOutcome::Exceeded);
        assert!(!regex_test(&regex, &haystack));
    }

    #[test]
    fn split_takes_the_shortest_pattern() {
        assert_eq!(split_literal("/a/g"), Some(("a", "g")));
        assert_eq!(split_literal("/a/b/"), Some(("a/b", "")));
        assert_eq!(split_literal("//"), None);
    }
}
