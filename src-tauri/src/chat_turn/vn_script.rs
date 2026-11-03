//! VN 劇本解析（vn-mode 包 1）：事件文字→格清單的唯一來源，寬容解析、認不得的都退旁白。
//! 格不另存：識別＝事件 ULID＋格序，規則見 `.ai/plans/vn-mode-pkg1.md` §1–§2。
use super::gm_reply::partial_display;
use crate::{data, transport};
use serde::{Deserialize, Serialize};

/// 每則事件最多幾個演出指令（超出的清掉，只在落檔版套）
const DIRECTIVE_CAP: usize = 2;
const MAX_OPTIONS: usize = 4;
/// 名字最長幾個字：超過就不當對白行（旁白裡的長句冒號）
const MAX_NAME_CHARS: usize = 24;
/// 名字裡出現就不是名字（句讀與引號）
const NOT_IN_NAME: &[char] = &[
    '，', '。', '、', '！', '？', '；', '「', '」', '『', '』', '“', '”', '"', ',', '!', '?', ';',
    '[', ']',
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Directive {
    Closeup,
    Shake,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum VnCell {
    Narration {
        text: String,
    },
    Dialogue {
        /// 對上名單的角色 id（玩家是 `PLAYER_SENTINEL`）；對不上＝沒有卡的配角，名牌照出、不擺立繪
        speaker_id: Option<String>,
        speaker_name: String,
        expression: String,
        /// `@變體` 原樣保留，解析在包 7
        variant: Option<String>,
        thought: bool,
        directive: Option<Directive>,
        text: String,
    },
    Bg {
        location: String,
        time: Option<String>,
    },
    Choices {
        options: Vec<String>,
    },
    Chapter,
    Ending {
        id: String,
    },
}

/// GM 這一拍停在哪種終止標記（停接力、球交回玩家）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VnStop {
    Choices,
    Chapter,
    Ending,
}

/// 文字出自哪條線：角色線只認自己的對白，別人的對白格改旁白（文字原樣）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum VnSource {
    Gm,
    Character { self_id: String },
}

/// 解析用的名單：角色 (id, 名字) 與玩家名。用「現在的名單」對名字（改名後舊對白對不到 id，已知限制）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct VnRoster {
    pub characters: Vec<(String, String)>,
    pub player: Option<String>,
}

impl VnRoster {
    pub(crate) fn new(metas: &[data::CharacterMeta], player: Option<&data::CharacterCard>) -> Self {
        Self {
            characters: metas
                .iter()
                .map(|meta| (meta.id.clone(), meta.name.clone()))
                .collect(),
            player: player.map(|card| card.name.clone()),
        }
    }

    pub(crate) fn load(root: &std::path::Path, world_id: &str) -> Result<Self, String> {
        let metas = data::list_characters(root, world_id).map_err(|error| error.to_string())?;
        let player = data::read_player_card(root, world_id).map_err(|error| error.to_string())?;
        Ok(Self::new(&metas, player.as_ref()))
    }

    fn id_of(&self, name: &str) -> Option<String> {
        self.characters
            .iter()
            .find(|(_, known)| known == name)
            .map(|(id, _)| id.clone())
            .or_else(|| {
                (self.player.as_deref() == Some(name))
                    .then(|| transport::PLAYER_SENTINEL.to_owned())
            })
    }
}

/// 串流入口的結果：最後一格可能還在長。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct VnText {
    pub cells: Vec<VnCell>,
    pub tail_tentative: bool,
}

enum Marker {
    Bg {
        location: String,
        time: Option<String>,
    },
    Choices,
    Chapter,
    Ending(String),
}

/// 整行、行首的 `[...]` 標記，關鍵字英文不分大小寫；認不得（或缺必要內容）回 None。
fn marker(line: &str) -> Option<Marker> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?.trim();
    let (keyword, rest) = match inner.split_once([':', '：']) {
        Some((keyword, rest)) => (keyword.trim(), Some(rest.trim())),
        None => (inner, None),
    };
    match (keyword.to_ascii_lowercase().as_str(), rest) {
        ("choices", None) => Some(Marker::Choices),
        ("chapter", None) => Some(Marker::Chapter),
        ("ending", Some(id)) if !id.is_empty() => Some(Marker::Ending(id.to_owned())),
        ("bg", Some(rest)) => {
            let (location, time) = match rest.split_once(['|', '｜']) {
                Some((location, time)) => (location.trim(), Some(time.trim())),
                None => (rest, None),
            };
            (!location.is_empty()).then(|| Marker::Bg {
                location: location.to_owned(),
                time: time.filter(|time| !time.is_empty()).map(str::to_owned),
            })
        }
        _ => None,
    }
}

/// `- 選項` 行（容許 `-` 後不空格）。
fn option_line(line: &str) -> Option<&str> {
    line.strip_prefix('-').map(str::trim)
}

fn strip_quotes(text: &str) -> &str {
    for (open, close) in [("「", "」"), ("“", "”"), ("\"", "\""), ("『", "』")] {
        if let Some(inner) = text
            .strip_prefix(open)
            .and_then(|rest| rest.strip_suffix(close))
        {
            return inner.trim();
        }
    }
    text
}

fn directive(word: &str) -> Option<Directive> {
    match word.trim().to_ascii_lowercase().as_str() {
        "closeup" => Some(Directive::Closeup),
        "shake" => Some(Directive::Shake),
        _ => None,
    }
}

/// 對白行 `名字 (表情[ @變體][, 指令])[ thought]: 台詞`；括號內容整組可省、冒號全半形皆可。
/// 名單外的名字要有括號才算（「她說：「走吧」」「I thought: …」這類旁白不當對白）。
fn dialogue(line: &str, roster: &VnRoster) -> Option<VnCell> {
    let colon = line.find([':', '：'])?;
    let mut head = line[..colon].trim();
    let body = line[colon..].trim_start_matches([':', '：']).trim();
    let thought = head
        .get(head.len().saturating_sub(7)..)
        .is_some_and(|tail| tail.eq_ignore_ascii_case("thought"));
    if thought {
        head = head[..head.len() - 7].trim_end();
    }
    let mut paren = None;
    if let Some(inner) = head.strip_suffix([')', '）']) {
        let open = inner.rfind(['(', '（'])?;
        paren = Some(&inner[open..]);
        head = inner[..open].trim_end();
    }
    let name = head;
    let paren = paren.map(|text| text.trim_start_matches(['(', '（']));
    let text = strip_quotes(body);
    if name.is_empty()
        || text.is_empty()
        || name.chars().count() > MAX_NAME_CHARS
        || name.contains(NOT_IN_NAME)
    {
        return None;
    }
    let speaker_id = roster.id_of(name);
    if speaker_id.is_none() && paren.is_none() {
        return None;
    }
    let mut parts = paren.unwrap_or_default().split([',', '，']);
    let first = parts.next().unwrap_or_default();
    let (expression, variant) = match first.split_once('@') {
        Some((expression, variant)) => (expression, Some(variant.trim())),
        None => (first, None),
    };
    // 括號第一格就是指令（`(closeup)`）也收
    let directive = directive(expression).or_else(|| parts.find_map(directive));
    let expression = expression.trim().to_ascii_lowercase();
    Some(VnCell::Dialogue {
        speaker_id,
        speaker_name: name.to_owned(),
        expression: match transport::VN_EXPRESSIONS.contains(&expression.as_str()) {
            true => expression,
            false => "neutral".to_owned(),
        },
        variant: variant
            .filter(|variant| !variant.is_empty())
            .map(str::to_owned),
        thought,
        directive,
        text: text.to_owned(),
    })
}

/// 語法層：逐行切格，不套任何刪格規則（串流版直接用）。
fn parse_cells(text: &str, source: &VnSource, roster: &VnRoster) -> Vec<VnCell> {
    let mut cells = Vec::new();
    let mut narration: Vec<&str> = Vec::new();
    let mut choices: Option<Vec<String>> = None;
    let flush = |narration: &mut Vec<&str>, cells: &mut Vec<VnCell>| {
        if !narration.is_empty() {
            cells.push(VnCell::Narration {
                text: narration.join("\n"),
            });
            narration.clear();
        }
    };
    for line in text.lines() {
        let line = line.trim();
        if let Some(options) = &mut choices {
            if line.is_empty() {
                continue;
            }
            if let Some(option) = option_line(line) {
                if !option.is_empty() && options.len() < MAX_OPTIONS {
                    options.push(option.to_owned());
                }
                continue;
            }
            cells.push(VnCell::Choices {
                options: std::mem::take(options),
            });
            choices = None;
        }
        if line.is_empty() {
            flush(&mut narration, &mut cells);
            continue;
        }
        if let Some(marker) = marker(line) {
            flush(&mut narration, &mut cells);
            match marker {
                Marker::Bg { location, time } => cells.push(VnCell::Bg { location, time }),
                Marker::Choices => choices = Some(Vec::new()),
                Marker::Chapter => cells.push(VnCell::Chapter),
                Marker::Ending(id) => cells.push(VnCell::Ending { id }),
            }
            continue;
        }
        match dialogue(line, roster) {
            Some(VnCell::Dialogue { speaker_id, .. }) if matches!(source, VnSource::Character { self_id } if speaker_id.as_ref() != Some(self_id)) => {
                narration.push(line)
            }
            Some(cell) => {
                flush(&mut narration, &mut cells);
                cells.push(cell);
            }
            None => narration.push(line),
        }
    }
    if let Some(options) = choices {
        cells.push(VnCell::Choices { options });
    }
    flush(&mut narration, &mut cells);
    cells
}

/// 終止標記的優先序：`[ending]` > `[chapter]` > `[choices]`。
fn terminal_rank(cell: &VnCell) -> Option<u8> {
    match cell {
        VnCell::Choices { .. } => Some(1),
        VnCell::Chapter => Some(2),
        VnCell::Ending { .. } => Some(3),
        _ => None,
    }
}

/// 包 1 的壓制資格：`[chapter]`／`[ending]` 不驗證、不取得資格（包 5 接上驗證），選項恆有。
pub(crate) fn pkg1_accepted(cell: &VnCell) -> bool {
    matches!(cell, VnCell::Choices { .. })
}

/// 取得壓制資格的最高終止格生效時，低優先的終止格剝掉。
pub(crate) fn apply_terminal_priority(
    cells: Vec<VnCell>,
    accepted: impl Fn(&VnCell) -> bool,
) -> Vec<VnCell> {
    let Some(top) = cells
        .iter()
        .filter(|cell| accepted(cell))
        .filter_map(terminal_rank)
        .max()
    else {
        return cells;
    };
    cells
        .into_iter()
        .filter(|cell| terminal_rank(cell).is_none_or(|rank| rank >= top))
        .collect()
}

/// 落檔版：選項少於 2 個丟掉、多塊取最後一塊、指令每事件上限、終止優先序。
pub(crate) fn parse_event(text: &str, source: &VnSource, roster: &VnRoster) -> Vec<VnCell> {
    let mut cells = parse_cells(text, source, roster);
    cells.retain(|cell| !matches!(cell, VnCell::Choices { options } if options.len() < 2));
    if let Some(last) = cells
        .iter()
        .rposition(|cell| matches!(cell, VnCell::Choices { .. }))
    {
        let mut index = 0;
        cells.retain(|cell| {
            index += 1;
            index - 1 == last || !matches!(cell, VnCell::Choices { .. })
        });
    }
    let mut directives = 0;
    for cell in &mut cells {
        if let VnCell::Dialogue {
            directive: directive @ Some(_),
            ..
        } = cell
        {
            directives += 1;
            if directives > DIRECTIVE_CAP {
                *directive = None;
            }
        }
    }
    apply_terminal_priority(cells, pkg1_accepted)
}

/// 停的理由：取得壓制資格的最高終止格；都沒有資格時取出現的最高者。三種任一出現都停接力。
pub(crate) fn stop_reason(cells: &[VnCell], accepted: impl Fn(&VnCell) -> bool) -> Option<VnStop> {
    let best = |only_accepted: bool| {
        cells
            .iter()
            .filter(|cell| !only_accepted || accepted(cell))
            .filter_map(|cell| terminal_rank(cell).map(|rank| (rank, cell)))
            .max_by_key(|(rank, _)| *rank)
            .map(|(_, cell)| match cell {
                VnCell::Choices { .. } => VnStop::Choices,
                VnCell::Chapter => VnStop::Chapter,
                _ => VnStop::Ending,
            })
    };
    best(true).or_else(|| best(false))
}

/// VN 桌 GM 回合的停點：有終止標記就不點名（`next` 改 None，不是 `__PLAYER__`）並回理由。
pub(crate) fn gm_stop(
    display: &str,
    roster: &VnRoster,
    next: Option<String>,
) -> (Option<String>, Option<VnStop>) {
    let cells = parse_event(display, &VnSource::Gm, roster);
    match stop_reason(&cells, pkg1_accepted) {
        Some(stop) => (None, Some(stop)),
        None => (next, None),
    }
}

/// 角色線過濾：`[bg]`／`[choices]`（連其下選項行）／`[chapter]`／`[ending]` 整行剝掉；
/// 別人的對白行文字原樣留著，解析時（`VnSource::Character`）改成旁白格。
pub(crate) fn filter_character_reply(text: &str) -> String {
    let mut kept = Vec::new();
    let mut in_choices = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if in_choices && (trimmed.is_empty() || option_line(trimmed).is_some()) {
            continue;
        }
        in_choices = false;
        match marker(trimmed) {
            Some(Marker::Choices) => in_choices = true,
            Some(_) => {}
            None => kept.push(line),
        }
    }
    kept.join("\n").trim().to_owned()
}

/// VN 桌角色回覆收尾（四處共用：串流入口、落檔、lane 抹寫、回聲對帳）：本輪前綴＝空字串（不剝「名字:」，
/// 那是對白行本身）→控制區塊與未閉合尾巴→角色線過濾。
pub(crate) fn finish_vn_character_reply(reply: &str, aborted: bool) -> transport::CharacterReply {
    let text = filter_character_reply(&transport::finish_character_reply(reply, "", aborted).text);
    transport::CharacterReply {
        raw: (reply != text).then(|| reply.to_owned()),
        text,
        aborted,
    }
}

/// 串流入口：先照來源做顯示文清理（GM 同中止旁白的剝法；角色同 `finish_vn_character_reply`），
/// 再解析；不套刪格規則，最後一格標暫定。
pub(crate) fn parse_text(text: &str, source: &VnSource, roster: &VnRoster) -> VnText {
    let cleaned = match source {
        VnSource::Gm => partial_display(text),
        VnSource::Character { .. } => finish_vn_character_reply(text, true).text,
    };
    let cells = parse_cells(&cleaned, source, roster);
    VnText {
        tail_tentative: !cells.is_empty(),
        cells,
    }
}

/// 只換掉文字裡最後一塊 `[choices]`（標記行到最後一個選項行），其餘逐字不動；沒有區塊回 None。
pub(crate) fn replace_choices(text: &str, options: &[String]) -> Option<String> {
    let lines: Vec<&str> = text.split('\n').collect();
    let start = lines
        .iter()
        .rposition(|line| matches!(marker(line.trim()), Some(Marker::Choices)))?;
    let mut end = start + 1;
    let mut scan = end;
    while scan < lines.len() {
        let trimmed = lines[scan].trim();
        if option_line(trimmed).is_some() {
            end = scan + 1;
        } else if !trimmed.is_empty() {
            break;
        }
        scan += 1;
    }
    let block = std::iter::once("[choices]".to_owned())
        .chain(options.iter().map(|option| format!("- {option}")))
        .collect::<Vec<_>>()
        .join("\n");
    let mut out: Vec<String> = lines[..start]
        .iter()
        .map(|line| (*line).to_owned())
        .collect();
    out.push(block);
    out.extend(lines[end..].iter().map(|line| (*line).to_owned()));
    Some(out.join("\n"))
}

#[cfg(test)]
#[path = "vn_script_tests.rs"]
mod tests;
