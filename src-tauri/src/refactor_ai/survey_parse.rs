use super::parse_common::parse_blocks;
use super::types::{
    RefactorEntryVerdict, RefactorRecommendOutcome, RefactorSpanRoute, RefactorSplitGroup,
    RefactorSurveyOutcome, RefactorSurveyPerson,
};

pub fn parse_recommend(raw: &str) -> Option<RefactorRecommendOutcome> {
    let mut recommend = None;
    let mut evidence = String::new();
    for line in raw.lines() {
        let trimmed = line.trim().trim_start_matches('-').trim();
        let lower = trimmed.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("recommend:") {
            let value = rest.trim();
            if value.starts_with("interface") {
                recommend = Some("interface".to_owned());
            } else if value.starts_with("characters") {
                recommend = Some("characters".to_owned());
            }
        } else if lower.starts_with("evidence:") {
            evidence = trimmed["evidence:".len()..].trim().to_owned();
        }
    }
    Some(RefactorRecommendOutcome {
        recommend: recommend?,
        evidence,
        run_id: String::new(),
        fingerprint: String::new(),
        raw: raw.to_owned(),
    })
}

fn parse_uid_line(line: &str) -> Option<u64> {
    let trimmed = line.trim().trim_start_matches('-').trim();
    let head = trimmed.get(..4)?;
    if !head.eq_ignore_ascii_case("uid=") {
        return None;
    }
    trimmed[4..].trim().split_whitespace().next()?.parse().ok()
}

/// 判斷欄位值是不是「肯定」（yes／true／是開頭，大小寫不拘）；INTERFACE 的 playable 與 PERSONS
/// 的 player 共用同一套寬鬆判斷。
fn is_affirmative(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("yes") || lower.starts_with("true") || lower.starts_with('是')
}

/// INTERFACE 區塊行：`- uid=12 playable: yes`。抽不到合法 uid 整行略過；playable 欄缺席或
/// 值不是 yes/true/是 一律當 no（沒把握就 no 的保守基準落在解析端再兜一層）。
fn parse_interface_line(line: &str) -> Option<(u64, bool)> {
    let uid = parse_uid_line(line)?;
    let lower = line.to_ascii_lowercase();
    let playable = lower
        .find("playable:")
        .is_some_and(|pos| is_affirmative(&lower[pos + "playable:".len()..]));
    Some((uid, playable))
}

/// 依宣告順序找一組欄位鍵在字串裡的位置（大小寫不拘）；欄位之間必須維持宣告順序，缺席的欄位
/// 就跳過不找、不影響後面欄位的搜尋起點。回傳陣列與 `keys` 一一對應，缺席回 None。搭配
/// `field_value` 切出每欄的值——PERSONS／ENTRIES／SPLITS／GROUPS 等固定欄序、部分欄可選的
/// 區塊行共用這套抽取邏輯。
fn locate_fields(lower: &str, keys: &[&str]) -> Vec<Option<usize>> {
    let mut positions = vec![None; keys.len()];
    let mut search_from = 0usize;
    for (index, key) in keys.iter().enumerate() {
        if let Some(relative) = lower.get(search_from..).and_then(|rest| rest.find(key)) {
            let pos = search_from + relative;
            positions[index] = Some(pos);
            search_from = pos + key.len();
        }
    }
    positions
}

/// 配 `locate_fields` 使用：取第 `index` 個欄位的值（欄名之後到下一個「有出現」欄位之前，trim
/// 過）；該欄缺席回 None。`text` 必須是取得 `lower`／`positions` 的同一段原文（byte 位置才會
/// 對得上——`to_ascii_lowercase` 不改變位元組長度與邊界，位置可以直接套用）。
fn field_value<'a>(
    text: &'a str,
    positions: &[Option<usize>],
    keys: &[&str],
    index: usize,
) -> Option<&'a str> {
    let start = positions[index]? + keys[index].len();
    let end = positions[index + 1..]
        .iter()
        .flatten()
        .next()
        .copied()
        .unwrap_or(text.len());
    text.get(start..end).map(str::trim)
}

const PERSON_FIELD_KEYS: [&str; 6] = ["name:", "uids:", "player:", "mode:", "spans:", "private:"];

/// 從盤點 PERSONS 區塊裡的一行抽出人名、來源 uid 清單、疑似玩家旗標、mode／spans／private；
/// 固定欄位順序 name→uids→player→mode→spans→private（跟提示詞範本一致），後四欄選配、缺席
/// 就是空值——舊格式行（只有 name／uids／player）照樣解析成功。抽不到名字或一個合法 uid 都
/// 沒有的行整行略過——garbage in 無聲跳過，不 panic。
fn parse_person_line(line: &str) -> Option<RefactorSurveyPerson> {
    let trimmed = line.trim().trim_start_matches('-').trim();
    let lower = trimmed.to_ascii_lowercase();
    let positions = locate_fields(&lower, &PERSON_FIELD_KEYS);
    let name = field_value(trimmed, &positions, &PERSON_FIELD_KEYS, 0)?;
    if name.is_empty() {
        return None;
    }
    let uids = parse_uid_list(field_value(trimmed, &positions, &PERSON_FIELD_KEYS, 1)?);
    if uids.is_empty() {
        return None;
    }
    let is_player =
        field_value(trimmed, &positions, &PERSON_FIELD_KEYS, 2).is_some_and(is_affirmative);
    let mode = field_value(trimmed, &positions, &PERSON_FIELD_KEYS, 3)
        .map(str::to_ascii_lowercase)
        .filter(|mode| mode.as_str() == "clean" || mode.as_str() == "tangled")
        .unwrap_or_default();
    let spans = field_value(trimmed, &positions, &PERSON_FIELD_KEYS, 4)
        .map(parse_span_list)
        .unwrap_or_default();
    let private_spans = field_value(trimmed, &positions, &PERSON_FIELD_KEYS, 5)
        .map(parse_span_list)
        .unwrap_or_default();
    Some(RefactorSurveyPerson {
        name: name.to_owned(),
        uids,
        is_player,
        mode,
        spans,
        private_spans,
    })
}

fn parse_uid_list(text: &str) -> Vec<String> {
    text.split([',', '、', '，'])
        .map(str::trim)
        .filter(|text| text.parse::<u64>().is_ok())
        .map(str::to_owned)
        .collect()
}

/// `uid#sN` 格式檢查：uid 與段號都必須是合法數字，中間用 `#s` 分隔。
fn is_valid_span_ref(text: &str) -> bool {
    let Some((uid, span_id)) = text.split_once("#s") else {
        return false;
    };
    !span_id.is_empty()
        && uid.parse::<u64>().is_ok()
        && span_id.chars().all(|ch| ch.is_ascii_digit())
}

fn parse_span_list(text: &str) -> Vec<String> {
    text.split([',', '、', '，'])
        .map(str::trim)
        .filter(|token| is_valid_span_ref(token))
        .map(str::to_owned)
        .collect()
}

/// 把一段文字從第一個空白處切開：回傳（第一個 token，去掉前導空白的其餘部分）。SPLITS 的 route
/// 欄先抽出關鍵字本身，剩下的部分再找 route 專屬的附欄（rule／name／title／id／note）。
fn split_first_token(text: &str) -> (&str, &str) {
    let text = text.trim_start();
    match text.find(char::is_whitespace) {
        Some(index) => (&text[..index], text[index..].trim_start()),
        None => (text, ""),
    }
}

/// 在一段文字裡找 `key`（大小寫不拘），回傳鍵之後到這段文字結尾、trim 過的內容；找不到或抽出來
/// 是空字串都回 None。用在「這欄一定是這段文字裡最後一個已知欄位」的情境（SPLITS route 的附欄）。
fn find_trailing_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let lower = text.to_ascii_lowercase();
    let pos = lower.find(key)?;
    let value = text[pos + key.len()..].trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn parse_rule_field(text: &str) -> Option<u8> {
    find_trailing_field(text, "rule:")?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

const ENTRY_ACTIONS: [&str; 4] = ["carry", "absorb", "drop", "split"];
const ENTRY_FIELD_KEYS: [&str; 3] = ["action:", "rule:", "reason:"];

/// ENTRIES 區塊行：`- uid=5 action: drop rule: 2 reason: ...`。uid 沿用 `parse_uid_line`
/// （跟 INTERFACE 同一種 `uid=` 寫法）；action 不在封閉字彙整行略過；rule／reason 都選填，
/// rule 就算 action 不是 drop 也照抽——drop 缺 rule 照收，交給後續稽核包退回，這裡不擋。
fn parse_entry_line(line: &str) -> Option<RefactorEntryVerdict> {
    let uid = parse_uid_line(line)?;
    let trimmed = line.trim().trim_start_matches('-').trim();
    let lower = trimmed.to_ascii_lowercase();
    let positions = locate_fields(&lower, &ENTRY_FIELD_KEYS);
    let action = field_value(trimmed, &positions, &ENTRY_FIELD_KEYS, 0)?.to_ascii_lowercase();
    if !ENTRY_ACTIONS.contains(&action.as_str()) {
        return None;
    }
    let rule = field_value(trimmed, &positions, &ENTRY_FIELD_KEYS, 1)
        .and_then(|value| value.split_whitespace().next())
        .and_then(|token| token.parse::<u8>().ok());
    let reason = field_value(trimmed, &positions, &ENTRY_FIELD_KEYS, 2)
        .unwrap_or_default()
        .to_owned();
    Some(RefactorEntryVerdict {
        uid: uid.to_string(),
        action,
        rule,
        reason,
    })
}

const SPLIT_ROUTES: [&str; 7] = [
    "statusbar",
    "gm",
    "drop",
    "person",
    "entry",
    "group",
    "unabsorbed",
];
const SPLIT_FIELD_KEYS: [&str; 2] = ["span:", "route:"];

/// SPLITS 區塊行：`- span: 7#s1 route: statusbar` 之類；route 後視關鍵字附對應欄位（drop→
/// rule、person→name、entry→title、group→id、unabsorbed→note，statusbar／gm 無附欄）。span
/// 格式不合法、route 不在封閉字彙、或 person／entry／group 缺對應附欄，整行略過（那個 span
/// 就此沒有路由，留給後續稽核包的「拆組守恆」兜底併回照搬）。
fn parse_split_line(line: &str) -> Option<RefactorSpanRoute> {
    let trimmed = line.trim().trim_start_matches('-').trim();
    let lower = trimmed.to_ascii_lowercase();
    let positions = locate_fields(&lower, &SPLIT_FIELD_KEYS);
    let span = field_value(trimmed, &positions, &SPLIT_FIELD_KEYS, 0)?;
    if !is_valid_span_ref(span) {
        return None;
    }
    let rest = field_value(trimmed, &positions, &SPLIT_FIELD_KEYS, 1)?;
    let (route, remainder) = split_first_token(rest);
    let route = route.to_ascii_lowercase();
    if !SPLIT_ROUTES.contains(&route.as_str()) {
        return None;
    }
    let mut result = RefactorSpanRoute {
        span: span.to_owned(),
        route: route.clone(),
        rule: None,
        name: String::new(),
        title: String::new(),
        group: String::new(),
        note: String::new(),
    };
    match route.as_str() {
        "drop" => result.rule = parse_rule_field(remainder),
        "person" => result.name = find_trailing_field(remainder, "name:")?.to_owned(),
        "entry" => result.title = find_trailing_field(remainder, "title:")?.to_owned(),
        "group" => result.group = find_trailing_field(remainder, "id:")?.to_owned(),
        "unabsorbed" => {
            result.note = find_trailing_field(remainder, "note:")
                .unwrap_or_default()
                .to_owned()
        }
        _ => {}
    }
    Some(result)
}

const GROUP_FIELD_KEYS: [&str; 4] = ["id:", "title:", "kind:", "spans:"];

/// GROUPS 區塊行：`- id: g1 title: 格式與行為 kind: mechanism spans: 16#s2,16#s5,18#s1`。
/// 固定欄位順序 id→title→kind→spans；id／title 空、kind 不是 setting/mechanism、或一個合法
/// span 引用都沒有的行整行略過。
fn parse_group_line(line: &str) -> Option<RefactorSplitGroup> {
    let trimmed = line.trim().trim_start_matches('-').trim();
    let lower = trimmed.to_ascii_lowercase();
    let positions = locate_fields(&lower, &GROUP_FIELD_KEYS);
    let id = field_value(trimmed, &positions, &GROUP_FIELD_KEYS, 0)?;
    if id.is_empty() {
        return None;
    }
    let title = field_value(trimmed, &positions, &GROUP_FIELD_KEYS, 1)?;
    if title.is_empty() {
        return None;
    }
    let kind = field_value(trimmed, &positions, &GROUP_FIELD_KEYS, 2)?.to_ascii_lowercase();
    if kind.as_str() != "setting" && kind.as_str() != "mechanism" {
        return None;
    }
    let spans = parse_span_list(field_value(trimmed, &positions, &GROUP_FIELD_KEYS, 3)?);
    if spans.is_empty() {
        return None;
    }
    Some(RefactorSplitGroup {
        id: id.to_owned(),
        title: title.to_owned(),
        kind,
        spans,
    })
}

/// FIELDS 區塊行：`- 好感度`，去掉開頭 `-` 與空白就是欄位名；空行略過。
fn parse_field_line(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_start_matches('-').trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

pub fn parse_survey(raw: &str) -> RefactorSurveyOutcome {
    let blocks = parse_blocks(
        raw,
        &[
            "MODE",
            "PERSONS",
            "INTERFACE",
            "ENTRIES",
            "SPLITS",
            "GROUPS",
            "FIELDS",
        ],
    );
    let mut persons = Vec::new();
    let mut interface_uids = Vec::new();
    let mut playable_interface_uids = Vec::new();
    let mut verdicts = Vec::new();
    let mut splits = Vec::new();
    let mut groups = Vec::new();
    let mut fields = Vec::new();
    let mut mode = String::new();
    for block in &blocks {
        match block.marker {
            // MODE 回聲：值在標記同行冒號後（`## MODE: interface`——值不可單獨成行，
            // 會被上面的掃描認成 INTERFACE 標記）；容錯也看區塊首行。合法值才收，
            // 其餘留空（呼叫端核對不過＝整份拒收）。
            "MODE" => {
                let candidate = if block.value.trim().is_empty() {
                    block.lines.iter().map(|line| line.trim()).find(|line| !line.is_empty()).unwrap_or("").to_owned()
                } else {
                    block.value.trim().to_owned()
                };
                let lower = candidate.to_ascii_lowercase();
                if lower == "interface" || lower == "characters" {
                    mode = lower;
                }
            }
            "PERSONS" => persons.extend(
                block
                    .lines
                    .iter()
                    .filter_map(|line| parse_person_line(line)),
            ),
            "INTERFACE" => {
                for (uid, playable) in block
                    .lines
                    .iter()
                    .filter_map(|line| parse_interface_line(line))
                {
                    interface_uids.push(uid.to_string());
                    if playable {
                        playable_interface_uids.push(uid.to_string());
                    }
                }
            }
            "ENTRIES" => {
                verdicts.extend(block.lines.iter().filter_map(|line| parse_entry_line(line)))
            }
            "SPLITS" => splits.extend(block.lines.iter().filter_map(|line| parse_split_line(line))),
            "GROUPS" => groups.extend(block.lines.iter().filter_map(|line| parse_group_line(line))),
            "FIELDS" => fields.extend(block.lines.iter().filter_map(|line| parse_field_line(line))),
            _ => {}
        }
    }
    RefactorSurveyOutcome {
        persons,
        interface_uids,
        playable_interface_uids,
        verdicts,
        splits,
        groups,
        fields,
        mode,
        raw: raw.to_owned(),
    }
}

/// MODE 行為正規化（refactor-mode-split）：回聲字串核過只保證「整份沒跑錯模式」；模式對了
/// 但判官違規吐出不該有的區塊（interface 模式吐 PERSONS）由這裡清掉——人物認領作廢後交回
/// 其餘判定：span／uid 另有合法 route 或 verdict 照原判（drop 進淘汰、statusbar／group 進
/// 產物、absorb 進規則），無下落者由涵蓋稽核與餘段兜底自動補 carry。內容全有下落、不拆卡。
/// characters 側的 INTERFACE／statusbar 已由 rule 5 淘汰與前端 pool 過濾涵蓋，這裡不動。
pub fn normalize_survey_for_mode(outcome: &mut RefactorSurveyOutcome) {
    if outcome.mode == "interface" {
        outcome.persons.clear();
    }
}
