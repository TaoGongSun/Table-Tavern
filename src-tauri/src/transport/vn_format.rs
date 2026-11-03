//! VN 桌（vn-mode 包 1）送模型的格式規範、導演指示與收尾句：規範放 GM／角色線的穩定 system，
//! 導演指示在 GM 回合尾。標記語意與解析在 `chat_turn::vn_script`；zh／en 兩套骨架照 `scaffold_en` 分流。
use super::messages::{message, scaffold_en, ChatMessage};
use super::narration_update_note;
use super::response::{next_line_rule, state_fence_rule};
use crate::data::vn::{Branching, Pov};
use crate::data::vn_art::CardLocation;

/// 包 1 固定的五種表情鍵（立繪臉層鍵）；清單外退 neutral。
pub const VN_EXPRESSIONS: [&str; 5] = ["neutral", "happy", "sad", "angry", "surprised"];

/// GM 線 system 附加段：VN 劇本格式全文＋範例＋文體要求。`extra`＝有角色宣告了專屬表情、`places`＝卡有地點清單
/// （兩者的清單見 `vn_card_note`）。
pub fn vn_format_rules(lang: &str, extra: bool, places: bool) -> String {
    let times = match (scaffold_en(lang), places) {
        (true, false) => "time is one of morning, day, dusk, night",
        (true, true) => "for listed locations the time is one of that location's listed times, otherwise one of morning, day, dusk, night",
        (false, false) => "時段只能寫 清晨、白天、黃昏、夜晚 其中之一",
        (false, true) => "「可用地點」裡的地點寫該地點列出的時段，其他地點寫 清晨、白天、黃昏、夜晚 其中之一",
    };
    let expressions = match (scaffold_en(lang), extra) {
        (_, false) => VN_EXPRESSIONS.join(", "),
        (true, true) => format!(
            "{} (characters under \"Character-specific expressions\" may also use theirs)",
            VN_EXPRESSIONS.join(", ")
        ),
        (false, true) => format!(
            "{}（「角色專屬表情」段列出的角色另可用自己的）",
            VN_EXPRESSIONS.join(", ")
        ),
    };
    match scaffold_en(lang) {
        true => format!(
            "\n## Visual novel script format\n\
             This table is played as a visual novel (VN). Write all story text in the script format below; the app splits it into cells and plays them one at a time.\n\
             - Narration: plain lines; a blank line starts the next cell. Keep narration lean, two or three sentences per cell.\n\
             - Dialogue: `Name (expression): \"line\"`, one line per cell, short lines. Expressions: {expressions}; leaving out the parentheses means neutral. Write names exactly as on the Characters list. Supporting characters without cards use the same form and must keep the parentheses.\n\
             - Inner thoughts: `Name (expression) thought: text`, without quotation marks.\n\
             - Staging: after the expression you may add `, closeup` or `, shake`, e.g. `Name (surprised, shake): \"line\"`; at most two per turn.\n\
             - Scene: `[bg: location | time]` on its own line, only when the location or time changes; {times}. A new location clears the stage.\n\
             - Choices: only when the story reaches a point where the player must decide, end the story text with `[choices]` followed by 2–4 lines `- option`. No choices anywhere else.\n\
             - Markers always stand on their own line with lowercase English keywords. No Markdown headings, bold, or lists other than option lines.\n\
             Example (the name is only illustrative):\n\
             [bg: harbor | dusk]\nSalt wind sweeps across the pier.\n\n\
             Irene (happy): \"You finally came.\"\nIrene (sad) thought: Does he still remember that day...\n\n\
             [choices]\n- Ask who she was waiting for\n- Walk over to her in silence\n"
        ),
        false => format!(
            "\n## 視覺小說劇本格式\n\
             這桌以視覺小說（VN）呈現。正文一律用下面的劇本格式寫，App 會把它拆成一格一格播放：\n\
             - 旁白：直接寫，空一行＝換下一格。旁白惜字，一格兩三句。\n\
             - 對白：`名字 (表情): 「台詞」`，一行一格，台詞要短。表情只能用 {expressions}，不寫括號＝neutral。名字照「登場角色」名單原字逐字寫；沒有角色卡的配角同樣寫法，括號不可省。\n\
             - 內心獨白：`名字 (表情) thought: 台詞`，不加引號。\n\
             - 演出：表情後可加 `, closeup`（特寫）或 `, shake`（震動），例如 `名字 (surprised, shake): 「台詞」`；每回合最多兩個。\n\
             - 換場景：`[bg: 地點 | 時段]` 單獨一行，只在地點或時段改變時寫；{times}；換地點＝舞台清空。\n\
             - 選項：只在劇情走到需要玩家抉擇的分岔點時，在正文最後寫 `[choices]`，下面每行 `- 選項` 共 2–4 個；其他時候不要出選項。\n\
             - 標記一律單獨一行、關鍵字用英文小寫；不要用 Markdown 標題、粗體或清單（選項行除外）。\n\
             範例（名字只是示意）：\n\
             [bg: 港口 | 黃昏]\n海風帶著鹹味吹過碼頭。\n\n\
             艾琳 (happy): 「你終於來了。」\n艾琳 (sad) thought: 他還記得那天的事嗎……\n\n\
             [choices]\n- 問她在等誰\n- 默默走到她身邊\n"
        ),
    }
}

/// 可用立繪變體（plans/vn-mode-pkg7.md §5）：GM 與角色線都附在 VN 規範後；沒變體的角色不列，全都沒有回空字串。
pub fn vn_variant_note(lang: &str, roster: &[(String, Vec<String>)]) -> String {
    let lines: Vec<String> = roster
        .iter()
        .filter(|(_, labels)| !labels.is_empty())
        .map(|(name, labels)| format!("- {name}: {}", labels.join(", ")))
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    let head = match scaffold_en(lang) {
        true => "\n## Sprite variants\nWhen a character changes outfit or pose, add `@variant` after the expression, e.g. `Name (happy @variant): \"line\"`; it stays until switched, and `@base` switches back. Use only these:\n",
        false => "\n## 立繪變體\n角色換裝或換姿態時，在表情後加 `@變體`，例如 `名字 (happy @變體): 「台詞」`；沿用到下次切換，寫 `@base` 換回。只能用下列：\n",
    };
    format!("{head}{}\n", lines.join("\n"))
}

/// VN 卡給的角色專屬表情與卡內地點（plans/vn-mode-pkg9b.md §2.3、§2.5）：GM 線附在 VN 規範後（system 穩定前綴）。
/// `expressions`＝(角色名, [(鍵, 顯示名)])；兩樣都沒有回空字串。
pub fn vn_card_note(
    lang: &str,
    expressions: &[(String, Vec<(String, String)>)],
    places: &[CardLocation],
) -> String {
    let en = scaffold_en(lang);
    let mut text = expression_note(lang, expressions);
    if !places.is_empty() {
        let fixed = match en {
            true => "morning, day, dusk, night",
            false => "清晨、白天、黃昏、夜晚",
        };
        let lines: Vec<String> = places
            .iter()
            .map(|place| {
                let times = match place.times.is_empty() {
                    true => fixed.to_owned(),
                    false => place.times.join(match en {
                        true => ", ",
                        false => "、",
                    }),
                };
                match en {
                    true => format!("- {} (times: {times})", place.name),
                    false => format!("- {}（時段：{times}）", place.name),
                }
            })
            .collect();
        text.push_str(match en {
            true => "\n## Locations\nFor `[bg]`, prefer these locations and write the name exactly as listed; for the time, write one of that location's listed times. Places not on the list are still allowed for improvised scenes (their time is one of morning, day, dusk, night).\n",
            false => "\n## 可用地點\n`[bg]` 優先用下列地點，名稱照寫；時段寫該地點列出的其中一個。清單外的地點仍可即興使用（時段照 清晨、白天、黃昏、夜晚）。\n",
        });
        text.push_str(&lines.join("\n"));
        text.push('\n');
    }
    text
}

/// 角色專屬表情段：GM 線列全部角色，角色線只列自己
pub fn expression_note(lang: &str, expressions: &[(String, Vec<(String, String)>)]) -> String {
    let en = scaffold_en(lang);
    let lines: Vec<String> = expressions
        .iter()
        .filter(|(_, entries)| !entries.is_empty())
        .map(|(name, entries)| {
            let keys: Vec<String> = entries
                .iter()
                .map(|(key, label)| match en {
                    true => format!("{key} ({label})"),
                    false => format!("{key}（{label}）"),
                })
                .collect();
            format!("- {name}: {}", keys.join(", "))
        })
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    let head = match en {
        true => "\n## Character-specific expressions\nBesides the five fixed expressions, these characters may also use the following expression keys (meaning in parentheses; write the key):\n",
        false => "\n## 角色專屬表情\n除固定五種外，下列角色另可用這些表情鍵（括號是意思，寫鍵）：\n",
    };
    format!("{head}{}\n", lines.join("\n"))
}

/// 角色線 system 附加段（全卡共用的穩定前綴，不帶名字；這一輪演誰在回合尾指定）。
pub fn vn_character_rules(lang: &str) -> String {
    let expressions = VN_EXPRESSIONS.join(", ");
    match scaffold_en(lang) {
        true => format!(
            "\n## Visual novel script format\n\
             This table is played as a visual novel. Write only the assigned character's own part: dialogue as `Name (expression): \"line\"` \
             (expressions: {expressions}), inner thoughts as `Name (expression) thought: text`, and actions or expressions as narration \
             (plain lines, no `Narration:` prefix); a blank line starts the next cell. Use the name exactly as on the list. Do not write markers such as `[bg]` or `[choices]`, \
             and do not speak for other characters or the player.\n"
        ),
        false => format!(
            "\n## 視覺小說劇本格式\n\
             這桌以視覺小說呈現。每一輪只寫被指定角色自己的部分：對白 `名字 (表情): 「台詞」`（表情限 {expressions}）、\
             內心獨白 `名字 (表情) thought: 台詞`，動作與神情寫成旁白（直接寫，不加「旁白：」），空一行換下一格。名字照名單原字。\
             不要寫 `[bg]`、`[choices]` 這類標記，不要替其他角色或玩家說話。\n"
        ),
    }
}

/// 角色線回合尾的本輪指定（VN 桌）：演誰＋沿用 system 的視覺小說劇本格式。
pub fn vn_assigned_line(name: &str, lang: &str) -> String {
    match scaffold_en(lang) {
        true => format!(
            "You are now \"{name}\". Following the visual novel script format above, write only {name}'s own part: \
             lines as `{name} (expression): \"line\"`, inner thoughts as `{name} (expression) thought: text`, and actions or expressions as narration. \
             Do not write markers or any explanation outside the character."
        ),
        false => format!(
            "現在你是「{name}」。照上面的視覺小說劇本格式只寫「{name}」自己的部分：台詞用 `{name} (表情): 「台詞」`、\
             內心獨白用 `{name} (表情) thought: 台詞`，動作與神情寫成旁白；不要寫標記、不要任何角色之外的說明。"
        ),
    }
}

/// 導演指示隨聖經與 VN 卡變的部分（plans/vn-mode-pkg9.md §2.5–§2.6）：`branching: none` 的節奏句、
/// 卡給的旁白人稱、卡定主角（主角名）。都沒有＝原本的指示。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VnDirection {
    pub branching: Option<Branching>,
    pub pov: Option<Pov>,
    pub protagonist: Option<String>,
}

/// VN 桌 GM 回合尾的導演指示：劇本正文→選項區塊→```state 圍欄→「下一位」行（名單非空才有）。
pub fn vn_narrate_instruction(
    lang: &str,
    roster: &[String],
    player_name: Option<&str>,
    updates: bool,
    direction: &VnDirection,
) -> ChatMessage {
    let fence = state_fence_rule(lang);
    let en = scaffold_en(lang);
    let voices = match (en, &direction.protagonist) {
        (true, None) => "do not speak for listed characters or the player, they reply on their own turns.".to_owned(),
        (false, None) => "不要替「登場角色」名單上的角色或玩家說話，他們會在自己的回合接話。".to_owned(),
        (true, Some(name)) => format!(
            "do not speak for listed characters, they reply on their own turns; you write the protagonist {name}'s lines and inner monologue in the script format, and each choice is {name}'s action or line for this beat."
        ),
        (false, Some(name)) => format!(
            "不要替「登場角色」名單上的角色說話，他們會在自己的回合接話；主角「{name}」的台詞與內心獨白由你照劇本格式寫，選項＝主角這一拍的行動或台詞。"
        ),
    };
    let mut instruction = match en {
        true => format!(
            "(Director instruction) Write this beat in the VN script format: advance the plot and describe the scene and the world's response. \
             You may portray supporting characters without character cards; {voices} \
             Do not output a status bar or any explanation outside the format. Order: script text (including `[bg]` and dialogue) → the `[choices]` block if needed → \
             the ```state fence → the `Next:` line. After the script text, start a new line and {fence}\
             The fence's `place` must match the latest `[bg]`, and its `time` must fall in that `[bg]` time of day (it may be more specific, e.g. with the date)."
        ),
        false => format!(
            "（導演指示）請以視覺小說劇本格式寫這一拍：推進劇情、描述場景與世界反應。沒有角色卡的配角由你扮演；\
             {voices}不要輸出狀態欄或格式以外的說明。\
             輸出順序：劇本正文（含 `[bg]` 與對白）→ 需要時的 `[choices]` 區塊 → ```state 圍欄 → 「下一位」行。\
             劇本正文寫完後另起一行，{fence}圍欄的 place 要與最近一次 `[bg]` 一致，time 要落在該 `[bg]` 的時段內（可更具體，例如帶日期）。"
        ),
    };
    if direction.branching == Some(Branching::None) {
        instruction.push_str(match en {
            true => " Choices only change tone, pacing, and affection; they never change this chapter's goal or end condition.",
            false => "選項只改語氣、節奏、好感，不改本章目標與結束條件。",
        });
    }
    if let Some(pov) = direction.pov {
        instruction.push_str(match (en, pov) {
            (true, Pov::First) => " Narrate in the first person, as the protagonist's \"I\".",
            (true, Pov::Second) => {
                " Narrate in the second person, addressing the protagonist as \"you\"."
            }
            (true, Pov::Third) => " Narrate in the third person.",
            (false, Pov::First) => "旁白用第一人稱，以主角的「我」敘述。",
            (false, Pov::Second) => "旁白用第二人稱，以「你」稱呼主角。",
            (false, Pov::Third) => "旁白用第三人稱。",
        });
    }
    if updates {
        instruction.push_str(narration_update_note(lang));
    }
    if !roster.is_empty() {
        instruction.push_str(&next_line_rule(lang, roster, player_name));
    }
    message("user", instruction)
}

/// VN 桌 GM 收尾句；`updates`＝這桌開了增量協定。
pub fn vn_closing(has_roster: bool, updates: bool, lang: &str) -> &'static str {
    match (scaffold_en(lang), has_roster, updates) {
        (true, false, false) => "Now, as the GM, carry out the director instruction above. Output only the script text, the choices block if needed, and the required ```state fence. Do not add a name prefix.",
        (true, true, false) => "Now, as the GM, carry out the director instruction above. Output only the script text, the choices block if needed, the required ```state fence, and the `Next:` line. Do not add a name prefix.",
        (true, false, true) => "Now, as the GM, carry out the director instruction above. Output only the script text, the choices block if needed, the required ```state fence, and the update block if anything else changed. Do not add a name prefix.",
        (true, true, true) => "Now, as the GM, carry out the director instruction above. Output only the script text, the choices block if needed, the required ```state fence, the update block if anything else changed, and the `Next:` line. Do not add a name prefix.",
        (false, false, false) => "現在請以 GM 身分執行上述導演指示，只輸出劇本正文、需要時的選項區塊與要求的 ```state 圍欄，不要加名字前綴。",
        (false, true, false) => "現在請以 GM 身分執行上述導演指示，只輸出劇本正文、需要時的選項區塊、要求的 ```state 圍欄與「下一位」行，不要加名字前綴。",
        (false, false, true) => "現在請以 GM 身分執行上述導演指示，只輸出劇本正文、需要時的選項區塊、要求的 ```state 圍欄與其他欄位有變動時的更新區塊，不要加名字前綴。",
        (false, true, true) => "現在請以 GM 身分執行上述導演指示，只輸出劇本正文、需要時的選項區塊、要求的 ```state 圍欄、其他欄位有變動時的更新區塊與「下一位」行，不要加名字前綴。",
    }
}

/// 「換一組」：只要一個新的選項區塊（與目前這組不同），不要正文與圍欄。
pub fn vn_reroll_instruction(lang: &str, current: &[String]) -> ChatMessage {
    let listed: String = current
        .iter()
        .map(|option| format!("\n- {option}"))
        .collect();
    let instruction = match scaffold_en(lang) {
        true => format!(
            "(Director instruction) The player wants a different set of choices. The story stays at the decision point at the end of the last narration. \
             Output only one new `[choices]` block: the line `[choices]` followed by 2–4 lines `- option`, different from the current set:{listed}\n\
             Do not write story text, a ```state fence, or a `Next:` line."
        ),
        false => format!(
            "（導演指示）玩家想換一組選項，劇情停在上一則旁白結尾的分岔點。請只輸出一個新的 `[choices]` 區塊：\
             第一行 `[choices]`，下面 2–4 行 `- 選項`，要與目前這組不同：{listed}\n\
             不要寫正文、```state 圍欄或「下一位」行。"
        ),
    };
    message("user", instruction)
}

/// 「換一組」的收尾句。
pub fn vn_reroll_closing(lang: &str) -> &'static str {
    match scaffold_en(lang) {
        true => "Now output only the new [choices] block.",
        false => "現在請只輸出新的 [choices] 區塊。",
    }
}
