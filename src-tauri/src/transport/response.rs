use super::messages::{ChatMessage, message};

use super::assemble::{PLAYER_SENTINEL};



/// 從換場摘要回覆的第一行取幕名：以「標題：」或「Title:」開頭（大小寫寬鬆）才算，
/// 取到就把該行（含後面的空行）從摘要文字中拿掉；解析不到就回傳 None、原文整段當摘要。
pub fn extract_scene_title(reply: &str) -> (Option<String>, String) {
    let mut lines = reply.splitn(2, '\n');
    let first_line = lines.next().unwrap_or("").trim();
    let remainder = lines.next().unwrap_or("");

    let title = first_line
        .strip_prefix("標題：")
        .or_else(|| first_line.strip_prefix("標題:"))
        .map(str::to_owned)
        .or_else(|| {
            let lower = first_line.to_lowercase();
            lower
                .strip_prefix("title:")
                .map(|_| first_line[6..].to_owned())
        })
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty());

    match title {
        Some(name) => (Some(name), remainder.trim_start_matches('\n').to_owned()),
        None => (None, reply.to_owned()),
    }
}

/// 導演指示：插入旁白（附加在 GM 上下文最後）；有名單時尾端固定要一行「下一位：」點名，
/// 旁白與點名一次呼叫完成（包 5 拍板）。
pub fn narrate_instruction(
    lang: &str,
    roster: &[String],
    player_name: Option<&str>,
) -> ChatMessage {
    let mut instruction = if lang == "en" {
        "(Director instruction) Insert a narration: describe scene changes, the world's response, or plot progress. \
         Use any length the story needs. You may portray supporting characters without character cards, but do not speak for listed characters or the player. \
         After the narration, start a new line and output one ```state fence. Inside it, write one `key: value` field per line. \
         Always output `time`, `place`, and `present`; write values in the player's language, separate multiple present characters with `、`, and repeat unchanged values too. \
         Do not add any explanation outside the narration and fence."
            .to_owned()
    } else {
        "（導演指示）請插入一段旁白：描述場景變化、世界反應或劇情推進，\
         篇幅不設限，依劇情需要自由發揮。沒有角色卡的配角由你扮演，可以出場與說話；\
         不要替「登場角色」名單上的角色或玩家說話。旁白本文結束後另起一行，輸出一個 ```state 圍欄，\
         圍欄內一行一欄、格式為「鍵: 值」，固定輸出 time、place、present 三個鍵；\
         值用玩家的語言寫，present 的多人用頓號分隔，沒有變化的欄位也照抄目前值。\
         圍欄以外不要有多餘說明。"
            .to_owned()
    };
    if !roster.is_empty() {
        let call = if lang == "en" {
            format!(
                " After the fence, end with exactly one final line `Next: <name>`, choosing the next speaker from: {}. \
                 If it is the player{player}'s turn to act, write `Next: {PLAYER_SENTINEL}` instead.",
                roster.join("、"),
                player = player_name.map_or(String::new(), |name| format!(" ({name})")),
            )
        } else {
            format!(
                "圍欄之後最後再另起一行，固定輸出「下一位：〈名字〉」，\
                 從名單中選出下一位最適合發言的角色：{}。\
                 若現在應該輪到玩家{player}行動，就寫「下一位：{PLAYER_SENTINEL}」。",
                roster.join("、"),
                // 沒玩家卡時是空字串，句子與加玩家卡前逐字相同
                player = player_name.map_or(String::new(), |name| format!("（{name}）")),
            )
        };
        instruction.push_str(&call);
    }
    message("user", instruction)
}

/// 卡片自帶介面的桌：卡片自己規定了輸出格式，我們不再要求旁白＋state 圍欄，
/// 否則兩套指令打架、模型會照我們的寫，卡片的介面就永遠對不上。
pub fn card_format_instruction(lang: &str, entry_title: Option<&str>) -> ChatMessage {
    let instruction = if lang == "en" {
        let format_source = entry_title
            .map(|title| format!(" (see the worldbook entry \"{title}\")"))
            .unwrap_or_default();
        format!(
            "(Director instruction) This table uses the interface that ships with the card, and the card already defines the reply format{format_source}. \
             Follow that specification exactly for this turn: same tags, same block order, same counts, same required fields, with the content advancing the story. \
             Do not rewrite it as ordinary narration, and do not output anything outside that format."
        )
    } else {
        let format_source = entry_title
            .map(|title| format!("（見世界書「{title}」）"))
            .unwrap_or_default();
        format!(
            "（導演指示）這桌使用卡片自帶的介面，卡片已經規定了回覆的輸出格式{format_source}。\
             請完全依照那份規定產生本回合的回覆：標籤、區塊順序、數量與必填欄位都照規定，內容依劇情推進。\
             不要改寫成一般旁白，也不要輸出規定格式以外的任何說明或狀態欄。"
        )
    };
    message("user", instruction)
}

/// 從旁白剝出尾端的「下一位：」點名行：回傳（點名原文, 剝除後的顯示文字）。
/// 與 extract_state_block 同族：只認整行、行首標記，掃到多行取最後一行；沒有就原樣返回。
pub fn extract_next_speaker(reply: &str) -> (Option<String>, String) {
    let hit = reply.lines().rev().find_map(|line| {
        let trimmed = line.trim();
        let rest = ["下一位", "next"].iter().find_map(|marker| {
            if trimmed.len() < marker.len() || !trimmed.is_char_boundary(marker.len()) {
                return None;
            }
            let (head, tail) = trimmed.split_at(marker.len());
            if head.to_ascii_lowercase() != *marker {
                return None;
            }
            let tail = tail.trim_start();
            tail.strip_prefix('：').or_else(|| tail.strip_prefix(':'))
        })?;
        let name = rest.trim();
        if name.is_empty() {
            return None;
        }
        Some((line.to_owned(), name.to_owned()))
    });
    match hit {
        Some((line, name)) => {
            let display = reply.replacen(&line, "", 1).trim_end().to_owned();
            (Some(name), display)
        }
        None => (None, reply.to_owned()),
    }
}

struct StateTag {
    start: usize,
    content_start: usize,
    content_end: usize,
    end: usize,
    /// UpdateVariable 只剝不收；狀態標籤才收欄位。
    collect: bool,
}

/// 掃出下一個狀態標籤。標籤名走前綴比對——`<StatusData>`、`<Status_block>` 各家自己取名，
/// 開頭是 status 就算；開閉標籤要同名才配對，免得吃掉後面不相干的內容。
fn find_state_tag(display: &str) -> Option<StateTag> {
    let lower = display.to_ascii_lowercase();
    let mut cursor = 0;
    while let Some(offset) = lower[cursor..].find('<') {
        let start = cursor + offset;
        cursor = start + 1;
        let Some(name_end) = lower[cursor..]
            .find(|character: char| {
                character == '>' || character == '/' || character.is_whitespace()
            })
            .map(|index| cursor + index)
        else {
            break;
        };
        let name = &lower[cursor..name_end];
        let collect = name.starts_with("status");
        if !collect && name != "updatevariable" {
            continue;
        }
        let Some(open_end) = lower[name_end..].find('>').map(|index| name_end + index) else {
            break;
        };
        let closing = format!("</{name}>");
        let Some(close_start) = lower[open_end + 1..]
            .find(&closing)
            .map(|index| open_end + 1 + index)
        else {
            continue;
        };
        return Some(StateTag {
            start,
            content_start: open_end + 1,
            content_end: close_start,
            end: close_start + closing.len(),
            collect,
        });
    }
    None
}

/// extract_state_block 的回傳：欄位對、原始 `<UpdateVariable>` 內容（供 mechanism 解析）、
/// 剝除後的顯示文字。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateBlock {
    pub fields: Vec<(Vec<String>, String)>,
    pub updates: Vec<String>,
    pub display: String,
}

/// 從 GM 回覆剝出狀態區塊。
/// 標籤比對一律走 to_ascii_lowercase——full lowercase 會改變某些字母的長度（如土耳其文 İ），
/// 算出的位移拿回原字串切片就會切在非字元邊界上 panic。
pub fn extract_state_block(reply: &str) -> StateBlock {
    let mut display = reply.to_owned();
    let mut blocks = Vec::new();
    let mut updates = Vec::new();
    let mut removed = false;

    let mut details_cursor = 0;
    while let Some(offset) = display[details_cursor..]
        .to_ascii_lowercase()
        .find("<details")
    {
        let start = details_cursor + offset;
        let Some(open_end) = display[start..].find('>').map(|index| start + index) else {
            break;
        };
        let lower = display.to_ascii_lowercase();
        let Some(end_start) = lower[open_end + 1..]
            .find("</details>")
            .map(|index| open_end + 1 + index)
        else {
            break;
        };
        let inner = &display[open_end + 1..end_start];
        let inner_lower = inner.to_ascii_lowercase();
        let Some(summary_start) = inner_lower.find("<summary") else {
            details_cursor = end_start + "</details>".len();
            continue;
        };
        let Some(summary_open_end) = inner[summary_start..]
            .find('>')
            .map(|index| summary_start + index)
        else {
            details_cursor = end_start + "</details>".len();
            continue;
        };
        let Some(summary_end) = inner_lower[summary_open_end + 1..]
            .find("</summary>")
            .map(|index| summary_open_end + 1 + index)
        else {
            details_cursor = end_start + "</details>".len();
            continue;
        };
        let summary = &inner[summary_open_end + 1..summary_end];
        let summary_lower = summary.to_ascii_lowercase();
        if !(summary_lower.contains("状态")
            || summary_lower.contains("狀態")
            || summary_lower.contains("status"))
        {
            details_cursor = end_start + "</details>".len();
            continue;
        }
        blocks.push(inner[summary_end + "</summary>".len()..].to_owned());
        display.replace_range(start..end_start + "</details>".len(), "");
        details_cursor = start;
        removed = true;
    }

    while let Some(tag) = find_state_tag(&display) {
        let content = display[tag.content_start..tag.content_end].to_owned();
        if tag.collect {
            blocks.push(content);
        } else {
            // UpdateVariable 是 MVU 的 JSON patch，原始內容交給 mechanism::parse_updates 解析。
            updates.push(content);
        }
        display.replace_range(tag.start..tag.end, "");
        removed = true;
    }

    // 鎮北王府那類把正文包在 <maintext> 裡：拆掉外殼留正文，標籤不裸露在畫面上。
    loop {
        let lower = display.to_ascii_lowercase();
        let Some(start) = lower.find("<maintext>") else {
            break;
        };
        let content_start = start + "<maintext>".len();
        if let Some(close_start) = lower[content_start..]
            .find("</maintext>")
            .map(|index| content_start + index)
        {
            display.replace_range(close_start..close_start + "</maintext>".len(), "");
        }
        display.replace_range(start..content_start, "");
        removed = true;
    }

    let mut fences = Vec::new();
    let mut cursor = 0;
    while let Some(open_offset) = display[cursor..].find("```") {
        let start = cursor + open_offset;
        let opening_end = start + 3;
        let Some(close_offset) = display[opening_end..].find("```") else {
            break;
        };
        let end_start = opening_end + close_offset;
        let header_end = display[opening_end..end_start]
            .find('\n')
            .map(|index| opening_end + index);
        let Some(header_end) = header_end else {
            cursor = end_start + 3;
            continue;
        };
        let info = display[opening_end..header_end].trim();
        let info_lower = info.to_ascii_lowercase();
        let is_state = matches!(
            info_lower.as_str(),
            "state" | "status" | "状态栏" | "狀態欄"
        );
        let is_trailing_plain = info.is_empty() && display[end_start + 3..].trim().is_empty();
        if is_state || is_trailing_plain {
            fences.push((
                start,
                end_start + 3,
                display[header_end + 1..end_start].to_owned(),
            ));
        }
        cursor = end_start + 3;
    }
    let fence_blocks: Vec<_> = fences.iter().map(|(_, _, block)| block.clone()).collect();
    for (start, end, _) in fences.into_iter().rev() {
        display.replace_range(start..end, "");
        removed = true;
    }
    blocks.extend(fence_blocks);

    if !removed {
        return StateBlock {
            fields: Vec::new(),
            updates: Vec::new(),
            display: reply.to_owned(),
        };
    }

    let fields = blocks
        .iter()
        .flat_map(|block| parse_indented_fields(block))
        .filter_map(|(mut path, value)| {
            let value = value?;
            if path.len() == 1 {
                path[0] = match path[0].to_ascii_lowercase().as_str() {
                    "time" | "時間" | "时间" => "time".to_owned(),
                    "place" | "location" | "地點" | "地点" => "place".to_owned(),
                    "present" | "在場" | "在场" | "在場人物" | "在场人物" => {
                        "present".to_owned()
                    }
                    _ => path[0].clone(),
                };
            }
            Some((path, value))
        })
        .collect();
    StateBlock {
        fields,
        updates,
        display: display.trim_end().to_owned(),
    }
}

/// 將縮排區塊解析成路徑和值；壞行略過，空值與空字典只標記分支而不終止解析。
pub fn parse_indented_fields(block: &str) -> Vec<(Vec<String>, Option<String>)> {
    let mut fields = Vec::new();
    let mut stack = Vec::<(usize, String)>::new();
    for line in block.lines() {
        let mut indent = 0;
        let mut offset = 0;
        for (index, character) in line.char_indices() {
            match character {
                ' ' => indent += 1,
                '\t' => indent += 4,
                _ => {
                    offset = index;
                    break;
                }
            }
            offset = index + character.len_utf8();
        }
        let mut line = &line[offset..];
        if let Some(stripped) = line.strip_prefix("- ") {
            line = stripped;
            indent += 2;
        }
        line = line.trim_start_matches(['#', '*', '+', '>']).trim_start();
        if line.is_empty() {
            continue;
        }
        let Some((index, separator)) = line
            .char_indices()
            .find(|(_, character)| matches!(character, ':' | '：'))
        else {
            continue;
        };
        let key = line[..index].trim();
        if key.is_empty() {
            continue;
        }
        let mut value = line[index + separator.len_utf8()..].trim();
        if value.len() >= 2
            && ((value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\'')))
        {
            value = &value[1..value.len() - 1];
        }
        while stack
            .last()
            .is_some_and(|(parent_indent, _)| *parent_indent >= indent)
        {
            stack.pop();
        }
        let mut path: Vec<String> = stack.iter().map(|(_, parent)| parent.clone()).collect();
        path.push(key.to_owned());
        if value.is_empty() || matches!(value, "{}" | "{ }") {
            fields.push((path, None));
            stack.push((indent, key.to_owned()));
            continue;
        }
        fields.push((path, Some(value.to_owned())));
    }
    fields
}

/// 從 GM 點名回覆解析出角色名（或玩家代號）。
/// 先試整句精確比對；模型多話時退回「回覆中最先出現的候選名」。
/// GM 是語言模型，輪到玩家時可能吐代號、也可能吐「玩家」或玩家卡上的名字，全部對回代號；
/// NPC 名字排在前面，撞名時算 NPC。
pub fn pick_speaker(reply: &str, roster: &[String], player_name: Option<&str>) -> Option<String> {
    let mut player_words = vec![PLAYER_SENTINEL, "玩家", "Player"];
    player_words.extend(player_name);
    let mut candidates: Vec<&str> = roster.iter().map(String::as_str).collect();
    candidates.extend(&player_words);

    let as_speaker = |name: &str| {
        if player_words.contains(&name) {
            PLAYER_SENTINEL.to_owned()
        } else {
            name.to_owned()
        }
    };
    let trimmed = reply.trim().trim_matches(|character: char| {
        character.is_whitespace() || "「」『』。．：:，,！!？?".contains(character)
    });
    if let Some(hit) = candidates.iter().find(|name| trimmed == **name) {
        return Some(as_speaker(hit));
    }
    candidates
        .iter()
        .filter_map(|name| reply.find(*name).map(|position| (position, *name)))
        .min_by_key(|(position, _)| *position)
        .map(|(_, name)| as_speaker(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::data::{self, AppConfig, CharacterCard, DataResult, FieldKind, FieldRule, InjectLevel, Mechanism, StateNode, TableState, Tier, TranscriptEvent, TranscriptKind, Visibility, WorldbookEntry};
    #[allow(unused_imports)]
    use crate::mechanism;
    #[allow(unused_imports)]
    use std::collections::{BTreeMap, BTreeSet};
    #[allow(unused_imports)]
    use super::super::test_support::{card, event, worldbook_entry};
    #[allow(unused_imports)]
    use super::super::messages::*;
    #[allow(unused_imports)]
    use super::super::context::*;
    #[allow(unused_imports)]
    use super::super::assemble::*;
    #[allow(unused_imports)]
    use super::super::state_view::*;
    #[allow(unused_imports)]
    use super::super::arrivals::*;
    #[allow(unused_imports)]
    use super::super::turns::*;
    #[allow(unused_imports)]
    use super::super::client::*;

    /// 卡片自帶介面時的導演指示：點名世界書那條格式規定的標題，且不再要求舊版的
    /// state 圍欄／下一位點名——那是兩邊指令打架的根因。
    #[test]
    fn card_format_instruction_points_to_worldbook_entry_and_drops_old_format_asks() {
        let zh_with_title = card_format_instruction("zh-TW", Some("回复规则")).content;
        assert!(zh_with_title.contains("回复规则"));
        assert!(!zh_with_title.contains("```state"));
        assert!(!zh_with_title.contains("下一位"));

        let zh_without_title = card_format_instruction("zh-TW", None).content;
        assert!(!zh_without_title.contains("見世界書"));

        let en_with_title = card_format_instruction("en", Some("Response Rules")).content;
        assert!(en_with_title.contains("Response Rules"));
        assert!(!en_with_title.contains("```"));
        assert!(!en_with_title.contains("Next:"));

        let en_without_title = card_format_instruction("en", None).content;
        assert!(!en_without_title.contains("see the worldbook entry"));
    }

    #[test]
    fn extract_next_speaker_strips_trailing_line_and_tolerates_variants() {
        // 標準形：旁白＋狀態欄剝除後尾端剩點名行
        let (name, display) = extract_next_speaker("夜更深了。\n下一位：狐狸");
        assert_eq!(name.as_deref(), Some("狐狸"));
        assert_eq!(display, "夜更深了。");
        // 英文標記與半形冒號、前後空白
        let (name, display) = extract_next_speaker("The night deepens.\nNext:  Fox ");
        assert_eq!(name.as_deref(), Some("Fox"));
        assert_eq!(display, "The night deepens.");
        // 玩家哨兵原文帶回，由 pick_speaker 對回代號
        let (name, _) =
            extract_next_speaker(format!("門開了。\n下一位：{PLAYER_SENTINEL}").as_str());
        assert_eq!(name.as_deref(), Some(PLAYER_SENTINEL));
        // 沒有點名行＝原樣返回；行首是普通英文 Next 不誤判
        let plain = "夜更深了。\nNext, the door opened.";
        assert_eq!(extract_next_speaker(plain), (None, plain.to_owned()));
    }

    /// 驗收：換幕順手取幕名——有標題行／無標題行／en 前綴，都不能報錯
    #[test]
    fn extract_scene_title_reads_zh_and_en_prefixes_and_falls_back_without_one() {
        let (title, rest) = extract_scene_title("標題：酒館夜話\n\n地點與時間：酒館");
        assert_eq!(title.as_deref(), Some("酒館夜話"));
        assert_eq!(rest, "地點與時間：酒館");

        let (title_en, rest_en) = extract_scene_title("Title: Tavern Talk\n\nLocation: the tavern");
        assert_eq!(title_en.as_deref(), Some("Tavern Talk"));
        assert_eq!(rest_en, "Location: the tavern");

        // 大小寫寬鬆
        let (title_mixed, _) = extract_scene_title("title: Mixed Case\n\nbody");
        assert_eq!(title_mixed.as_deref(), Some("Mixed Case"));

        // 沒有標題行：整段原文當摘要，不報錯
        let (none_title, whole) = extract_scene_title("地點與時間：酒館\n關鍵事件：無");
        assert_eq!(none_title, None);
        assert_eq!(whole, "地點與時間：酒館\n關鍵事件：無");
    }

    #[test]
    fn pick_speaker_handles_exact_verbose_and_player_sentinel() {
        let roster = vec!["狐狸".to_owned(), "騎士".to_owned()];
        assert_eq!(pick_speaker("狐狸", &roster, None).unwrap(), "狐狸");
        assert_eq!(pick_speaker("「騎士」。", &roster, None).unwrap(), "騎士");
        assert_eq!(
            pick_speaker("玩家", &roster, None).unwrap(),
            PLAYER_SENTINEL
        );
        // 輪到玩家時 GM 怎麼講都算：代號本身、「玩家」、英文 Player、玩家卡上的名字
        for reply in [PLAYER_SENTINEL, "玩家", "Player", "阿濤"] {
            assert_eq!(
                pick_speaker(reply, &roster, Some("阿濤")).unwrap(),
                PLAYER_SENTINEL
            );
        }
        // 模型多話：取回覆中最先出現的候選名
        assert_eq!(
            pick_speaker("下一位應該由騎士發言，逼問狐狸。", &roster, None).unwrap(),
            "騎士"
        );
        assert_eq!(pick_speaker("酒保", &roster, None), None);
    }

    #[test]
    fn extract_state_fence_returns_fields_and_hides_fence() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block(
            "雨停了。\n```state\ntime: 午夜\nplace：舊碼頭\npresent: 阿濤、船長\n```",
        );
        assert_eq!(
            fields,
            vec![
                (vec!["time".to_owned()], "午夜".to_owned()),
                (vec!["place".to_owned()], "舊碼頭".to_owned()),
                (vec!["present".to_owned()], "阿濤、船長".to_owned()),
            ]
        );
        assert_eq!(display, "雨停了。");
    }

    #[test]
    fn extract_state_block_collects_nested_yaml_and_skips_plain_list_items() {
        let StateBlock { fields, .. } = extract_state_block(
            "<Status_block>World:\n  - 城市:\n      名稱: \"晨港\"\n      - 純清單項\n      人口: '1200'\n</Status_block>",
        );
        assert_eq!(
            fields,
            vec![
                (
                    vec!["World".to_owned(), "城市".to_owned(), "名稱".to_owned()],
                    "晨港".to_owned(),
                ),
                (
                    vec!["World".to_owned(), "城市".to_owned(), "人口".to_owned()],
                    "1200".to_owned(),
                ),
            ]
        );
    }

    /// 縮排行與空字典都要保留成分支標記，葉子則保留實際值。
    #[test]
    fn parse_indented_fields_marks_branch_lines() {
        assert_eq!(
            parse_indented_fields("World:\n  Time: 清晨\n  Inventory: {}"),
            vec![
                (vec!["World".to_owned()], None),
                (
                    vec!["World".to_owned(), "Time".to_owned()],
                    Some("清晨".to_owned()),
                ),
                (vec!["World".to_owned(), "Inventory".to_owned()], None),
            ]
        );
    }

    #[test]
    fn extract_state_discards_bad_lines_without_losing_valid_fields() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block(
            "旁白\n```state\n- time: 清晨\n沒有冒號\nplace:   \n# 自訂：有效\n```",
        );
        assert_eq!(
            fields,
            vec![
                (vec!["time".to_owned()], "清晨".to_owned()),
                (vec!["自訂".to_owned()], "有效".to_owned()),
            ]
        );
        assert_eq!(display, "旁白");
    }

    #[test]
    fn extract_state_details_summary_is_parsed_and_hidden() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block(
            "港口傳來鐘聲。<details><summary>状态栏</summary>时间：黃昏\n地点：港口</details>",
        );
        assert_eq!(
            fields,
            vec![
                (vec!["time".to_owned()], "黃昏".to_owned()),
                (vec!["place".to_owned()], "港口".to_owned()),
            ]
        );
        assert_eq!(display, "港口傳來鐘聲。");
    }

    #[test]
    fn extract_status_tag_is_parsed_and_hidden() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block("門開了。<STATUS>time: 午夜\nplace: 走廊</status>剩下的話。");
        assert_eq!(
            fields,
            vec![
                (vec!["time".to_owned()], "午夜".to_owned()),
                (vec!["place".to_owned()], "走廊".to_owned()),
            ]
        );
        assert_eq!(display, "門開了。剩下的話。");
    }

    #[test]
    fn extract_update_variable_hides_json_without_parsing_it() {
        let StateBlock {
            fields,
            updates,
            display,
        } = extract_state_block("她點頭。<UpdateVariable>{\"time\":\"午夜\"}</UpdateVariable>");
        assert!(fields.is_empty());
        assert_eq!(updates, vec!["{\"time\":\"午夜\"}".to_owned()]);
        assert_eq!(display, "她點頭。");
    }

    /// 各家標籤名不同（donass 的 `<StatusData>`、鎮北王府的 `<Status_block>`），
    /// 開頭是 status 就認；同名才配對，`<statusdata>` 不會被 `</status_block>` 收掉。
    #[test]
    fn extract_state_accepts_any_status_prefixed_tag() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block(
            "地下城。<StatusData>体力:60\n好感:20</StatusData>之後。\
             <Status_block>时间: 戌时\n地点: 浴房</Status_block>",
        );
        assert_eq!(
            fields,
            vec![
                (vec!["体力".to_owned()], "60".to_owned()),
                (vec!["好感".to_owned()], "20".to_owned()),
                (vec!["time".to_owned()], "戌时".to_owned()),
                (vec!["place".to_owned()], "浴房".to_owned()),
            ]
        );
        assert_eq!(display, "地下城。之後。");
    }

    /// 沒有配對收尾的標籤整段留著：寧可讓玩家看到半截標籤，也不吞掉後面的旁白。
    #[test]
    fn extract_state_leaves_unclosed_status_tag_alone() {
        let reply = "他開口。<StatusData>体力:60\n後面還有很多話。";
        assert_eq!(
            extract_state_block(reply),
            StateBlock {
                fields: Vec::new(),
                updates: Vec::new(),
                display: reply.to_owned(),
            }
        );
    }

    /// 名字裡帶 status 但不是開頭的（`<combatStatus>`）是卡片自訂欄位，不能當狀態區塊剝掉。
    #[test]
    fn extract_state_ignores_tags_merely_containing_status() {
        let reply = "他喘著氣。<combatStatus>負傷</combatStatus>";
        assert_eq!(
            extract_state_block(reply),
            StateBlock {
                fields: Vec::new(),
                updates: Vec::new(),
                display: reply.to_owned(),
            }
        );
    }

    #[test]
    fn extract_state_unwraps_maintext_into_narration() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block(
            "<maintext>\n夜色濃重。\n</maintext>\n<Status_block>时间: 戌时</Status_block>",
        );
        assert_eq!(fields, vec![(vec!["time".to_owned()], "戌时".to_owned())]);
        assert_eq!(display, "\n夜色濃重。");
    }

    /// 只有正文外殼、沒有狀態區塊時，一樣要拆掉外殼。
    #[test]
    fn extract_state_unwraps_maintext_without_state_block() {
        let StateBlock {
            fields, display, ..
        } = extract_state_block("<mainText>夜色濃重。</mainText>");
        assert!(fields.is_empty());
        assert_eq!(display, "夜色濃重。");
    }

    #[test]
    fn extract_state_keeps_unwrapped_narration_byte_for_byte() {
        let reply = "純旁白\n保留尾端空行\n\n";
        assert_eq!(
            extract_state_block(reply),
            StateBlock {
                fields: Vec::new(),
                updates: Vec::new(),
                display: reply.to_owned(),
            }
        );
    }

    #[test]
    fn extract_state_keeps_middle_code_fence_but_removes_trailing_plain_fence() {
        let reply = "提示：\n```rust\nlet time = 1;\n```\n旁白\n```\ntime: 午夜\n```";
        let StateBlock {
            fields, display, ..
        } = extract_state_block(reply);
        assert_eq!(fields, vec![(vec!["time".to_owned()], "午夜".to_owned())]);
        assert_eq!(display, "提示：\n```rust\nlet time = 1;\n```\n旁白");
    }

}
