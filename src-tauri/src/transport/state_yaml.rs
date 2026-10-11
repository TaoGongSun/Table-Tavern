//! 原卡狀態區塊（`<Status_block>` YAML、狀態圍欄、狀態 `<details>`）解析成欄位。
//! 認得 `- ` 清單項：清單存成 `0/1/…` 索引分支（同 `mechanism::tree::json_to_node`），
//! 並記下清單容器路徑供套用時整支替換。其餘寫法沿用 `parse_indented_fields` 的寬鬆規則：
//! 全形冒號也算分隔、行首 `#*+>` 略過、沒冒號的行丟掉、根層孤立 `- 鍵: 值` 當具名鍵。

/// 一個狀態區塊解析出的結果。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ParsedFields {
    /// 葉子路徑與值（照出現順序）
    pub fields: Vec<(Vec<String>, String)>,
    /// 清單容器路徑（含空清單 `鍵: []`、`0:`／`1:` 連號回寫的容器）
    pub lists: Vec<Vec<String>>,
    /// 具名容器路徑（值空或 `{}` 的鍵、底下是具名鍵的分支）
    pub maps: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Scalar(String),
    /// 值空或 `{}` 而底下沒有內容的鍵
    Empty,
    Map(Vec<(String, Node)>),
    List(Vec<Node>),
}

enum Entry {
    Key(String, Node),
    /// 清單項；None＝空純量項、`- {}` 之類略過不佔索引的項
    Item(Option<Node>),
}

#[derive(Clone, Copy)]
enum Accept {
    All,
    /// 縮排比這欄深的行
    Deeper(usize),
    /// 縮排比這欄深的行，或同欄的 `- ` 行（PyYAML 預設寫法：清單項與父鍵同欄）
    DeeperOrDashAt(usize),
}

struct Line {
    indent: usize,
    text: String,
}

struct Parser {
    lines: Vec<Line>,
    pos: usize,
}

const BLOCK_SCALARS: [&str; 6] = ["|", "|-", "|+", ">", ">-", ">+"];

/// 解析一個狀態區塊。
pub fn parse_state_fields(block: &str) -> ParsedFields {
    let mut parser = Parser {
        lines: block.lines().filter_map(split_indent).collect(),
        pos: 0,
    };
    let entries = parser.entries(Accept::All);
    let mut parsed = ParsedFields::default();
    // 根層沒有容器：孤立的 `- 鍵: 值` 照舊剝掉 `- ` 當具名鍵，沒冒號的純量項丟掉
    for (key, node) in flatten(entries) {
        emit(node, vec![key], &mut parsed);
    }
    parsed
}

fn split_indent(raw: &str) -> Option<Line> {
    let mut indent = 0;
    let mut offset = raw.len();
    for (index, character) in raw.char_indices() {
        match character {
            ' ' => indent += 1,
            '\t' => indent += 4,
            _ => {
                offset = index;
                break;
            }
        }
    }
    let text = raw[offset..].trim_end();
    (!text.is_empty()).then(|| Line {
        indent,
        text: text.to_owned(),
    })
}

/// `- ` 開頭（或整行只有 `-`）就回傳內容相對行首的位移與內容。
fn dash_rest(text: &str) -> Option<(usize, &str)> {
    if text == "-" {
        return Some((1, ""));
    }
    let after = text.strip_prefix("- ")?;
    let rest = after.trim_start();
    Some((text.len() - rest.len(), rest))
}

fn strip_markers(text: &str) -> &str {
    text.trim_start_matches(['#', '*', '+', '>']).trim_start()
}

/// 第一個 `:`／`：` 切成鍵與值（值剝成對引號；第三項＝值原本帶引號）。
fn split_key(text: &str) -> Option<(&str, &str, bool)> {
    let (index, separator) = text
        .char_indices()
        .find(|(_, character)| matches!(character, ':' | '：'))?;
    let key = text[..index].trim();
    let raw = text[index + separator.len_utf8()..].trim();
    let value = strip_quotes(raw);
    Some((key, value, value.len() != raw.len()))
}

fn has_key(text: &str) -> bool {
    split_key(strip_markers(text)).is_some_and(|(key, _, _)| !key.is_empty())
}

fn strip_quotes(value: &str) -> &str {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn is_empty_map(value: &str) -> bool {
    matches!(value, "{}" | "{ }")
}

fn is_empty_list(value: &str) -> bool {
    matches!(value, "[]" | "[ ]")
}

/// 清單項第一行是純量還是結構：引號開頭找配對的收尾引號，收尾後只剩空白是純量、
/// 接冒號是帶引號的鍵（回傳去掉引號的改寫行）；不帶引號的有冒號就是鍵。
enum ItemHead {
    /// 值與「原本帶引號」：帶引號的 `"{}"`、`"[]"` 是字串，不是空容器
    Scalar(String, bool),
    Structure(Option<String>),
}

fn classify_item(rest: &str) -> ItemHead {
    if dash_rest(rest).is_some() {
        return ItemHead::Structure(None);
    }
    let content = strip_markers(rest);
    if let Some(quote) = content.chars().next().filter(|c| matches!(c, '"' | '\'')) {
        if let Some(close) = content[1..].find(quote) {
            let inner = &content[1..1 + close];
            let after = content[1 + close + 1..].trim_start();
            if after.is_empty() {
                return ItemHead::Scalar(inner.to_owned(), true);
            }
            if after.starts_with([':', '：']) {
                return ItemHead::Structure(Some(format!("{inner}{after}")));
            }
        }
    }
    if has_key(content) {
        return ItemHead::Structure(None);
    }
    ItemHead::Scalar(content.to_owned(), false)
}

impl Parser {
    fn accepts(&self, accept: Accept) -> bool {
        let Some(line) = self.lines.get(self.pos) else {
            return false;
        };
        match accept {
            Accept::All => true,
            Accept::Deeper(column) => line.indent > column,
            Accept::DeeperOrDashAt(column) => {
                line.indent > column || (line.indent == column && dash_rest(&line.text).is_some())
            }
        }
    }

    fn skip_deeper(&mut self, column: usize) -> Vec<String> {
        let mut skipped = Vec::new();
        while self
            .lines
            .get(self.pos)
            .is_some_and(|line| line.indent > column)
        {
            skipped.push(self.lines[self.pos].text.clone());
            self.pos += 1;
        }
        skipped
    }

    /// 收一層的項目。區塊純量（`|`、`>`）的續行收成值、不當鍵解析。
    fn entries(&mut self, accept: Accept) -> Vec<Entry> {
        let mut out = Vec::new();
        while self.accepts(accept) {
            let indent = self.lines[self.pos].indent;
            let text = self.lines[self.pos].text.clone();
            if let Some((offset, rest)) = dash_rest(&text) {
                out.push(Entry::Item(self.item(indent, offset, rest)));
                continue;
            }
            self.pos += 1;
            let Some((key, value, quoted)) = split_key(strip_markers(&text)) else {
                continue;
            };
            if key.is_empty() {
                continue;
            }
            let node = if quoted && !value.is_empty() {
                Node::Scalar(value.to_owned())
            } else if value.is_empty() || is_empty_map(value) {
                build(self.entries(Accept::DeeperOrDashAt(indent)))
            } else if is_empty_list(value) {
                Node::List(Vec::new())
            } else if BLOCK_SCALARS.contains(&value) {
                // 底下沒有續行就當沒寫值，不誤刪既有值
                let lines = self.skip_deeper(indent);
                if lines.is_empty() {
                    continue;
                }
                Node::Scalar(lines.join("\n"))
            } else {
                Node::Scalar(value.to_owned())
            };
            out.push(Entry::Key(key.to_owned(), node));
        }
        out
    }

    /// 一個清單項：`- ` 後的內容當該項第一行（欄位＝內容所在欄），整行只有 `-` 就看下一行起。
    fn item(&mut self, dash_indent: usize, offset: usize, rest: &str) -> Option<Node> {
        if rest.is_empty() {
            self.pos += 1;
            return build_item(self.entries(Accept::Deeper(dash_indent)));
        }
        // 區塊純量先判（`>` 不能被當行首記號剝掉，續行裡的冒號也不能讓它變成標題項）
        if BLOCK_SCALARS.contains(&rest) {
            self.pos += 1;
            let lines = self.skip_deeper(dash_indent);
            return (!lines.is_empty()).then(|| Node::Scalar(lines.join("\n")));
        }
        match classify_item(rest) {
            ItemHead::Scalar(value, false) if self.titles_children(dash_indent) => {
                // 無冒號的標題項底下還有 `- ` 或 `鍵: 值`：當標題鍵（等同 `- 林远:`）
                // 底下比 `-` 深的行都歸它（不必比標題文字更深）
                let title = value.trim_matches('*').trim().to_owned();
                self.pos += 1;
                let children = build(self.entries(Accept::Deeper(dash_indent)));
                Some(Node::Map(vec![(title, children)]))
            }
            ItemHead::Scalar(value, quoted) => {
                self.pos += 1;
                // 純量項底下更深的純續行不產生鍵
                self.skip_deeper(dash_indent);
                if value.is_empty() || (!quoted && is_empty_map(&value)) {
                    None
                } else if !quoted && is_empty_list(&value) {
                    Some(Node::List(Vec::new()))
                } else {
                    Some(Node::Scalar(value))
                }
            }
            ItemHead::Structure(rewritten) => {
                self.lines[self.pos] = Line {
                    indent: dash_indent + offset,
                    text: rewritten.unwrap_or_else(|| rest.to_owned()),
                };
                build_item(self.entries(Accept::Deeper(dash_indent)))
            }
        }
    }

    /// 目前這行（清單項）底下更深的行裡有 `- ` 或 `鍵: 值`。
    fn titles_children(&self, dash_indent: usize) -> bool {
        self.lines[self.pos + 1..]
            .iter()
            .take_while(|line| line.indent > dash_indent)
            .any(|line| dash_rest(&line.text).is_some() || has_key(&line.text))
    }
}

fn build_item(entries: Vec<Entry>) -> Option<Node> {
    match build(entries) {
        Node::Empty => None,
        node => Some(node),
    }
}

/// 一層項目組成節點：全是鍵＝具名分支；全是清單項＝清單，但每項都是「單一鍵：純量、
/// 鍵名互不重複」時當具名鍵（等同舊寫法）；兩種混用照舊攤平成具名鍵。
fn build(entries: Vec<Entry>) -> Node {
    if entries.is_empty() {
        return Node::Empty;
    }
    if !entries.iter().all(|entry| matches!(entry, Entry::Item(_))) {
        return Node::Map(flatten(entries));
    }
    let items: Vec<Node> = entries
        .into_iter()
        .filter_map(|entry| match entry {
            Entry::Item(item) => item,
            Entry::Key(..) => None,
        })
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let dict_like = !items.is_empty()
        && items.iter().all(|item| match item {
            Node::Map(pairs) => {
                pairs.len() == 1
                    && matches!(pairs[0].1, Node::Scalar(_))
                    && seen.insert(pairs[0].0.clone())
            }
            _ => false,
        });
    if dict_like {
        return Node::Map(
            items
                .into_iter()
                .flat_map(|item| match item {
                    Node::Map(pairs) => pairs,
                    _ => Vec::new(),
                })
                .collect(),
        );
    }
    Node::List(items)
}

/// 攤平成具名鍵：鍵照收，清單項是具名分支就把它的鍵併進來，純量項與巢狀清單丟掉。
fn flatten(entries: Vec<Entry>) -> Vec<(String, Node)> {
    entries
        .into_iter()
        .flat_map(|entry| match entry {
            Entry::Key(key, node) => vec![(key, node)],
            Entry::Item(Some(Node::Map(pairs))) => pairs,
            Entry::Item(_) => Vec::new(),
        })
        .collect()
}

fn emit(node: Node, path: Vec<String>, out: &mut ParsedFields) {
    match node {
        Node::Scalar(value) => out.fields.push((path, value)),
        Node::Empty => out.maps.push(path),
        Node::Map(pairs) => {
            let keys: std::collections::BTreeSet<&str> =
                pairs.iter().map(|(key, _)| key.as_str()).collect();
            let indexed = !keys.is_empty()
                && (0..keys.len()).all(|index| keys.contains(index.to_string().as_str()));
            if indexed {
                out.lists.push(path.clone());
            } else {
                out.maps.push(path.clone());
            }
            for (key, child) in pairs {
                let mut child_path = path.clone();
                child_path.push(key);
                emit(child, child_path, out);
            }
        }
        Node::List(items) => {
            out.lists.push(path.clone());
            for (index, item) in items.into_iter().enumerate() {
                let mut child_path = path.clone();
                child_path.push(index.to_string());
                emit(item, child_path, out);
            }
        }
    }
}
