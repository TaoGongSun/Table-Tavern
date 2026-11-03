//! VN 桌（vn-mode 包 1）送模型的格式規範、導演指示與收尾句：規範放 GM／角色線的穩定 system，
//! 導演指示在 GM 回合尾。標記語意與解析在 `chat_turn::vn_script`；zh／en 兩套骨架照 `scaffold_en` 分流。
use super::messages::{message, scaffold_en, ChatMessage};
use super::narration_update_note;
use super::response::{next_line_rule, state_fence_rule};

/// 包 1 固定的五種表情鍵（立繪臉層鍵）；清單外退 neutral。
pub const VN_EXPRESSIONS: [&str; 5] = ["neutral", "happy", "sad", "angry", "surprised"];

/// GM 線 system 附加段：VN 劇本格式全文＋範例＋文體要求。
pub fn vn_format_rules(lang: &str) -> String {
    let expressions = VN_EXPRESSIONS.join(", ");
    match scaffold_en(lang) {
        true => format!(
            "\n## Visual novel script format\n\
             This table is played as a visual novel (VN). Write all story text in the script format below; the app splits it into cells and plays them one at a time.\n\
             - Narration: plain lines; a blank line starts the next cell. Keep narration lean, two or three sentences per cell.\n\
             - Dialogue: `Name (expression): \"line\"`, one line per cell, short lines. Expressions: {expressions}; leaving out the parentheses means neutral. Write names exactly as on the Characters list. Supporting characters without cards use the same form and must keep the parentheses.\n\
             - Inner thoughts: `Name (expression) thought: text`, without quotation marks.\n\
             - Staging: after the expression you may add `, closeup` or `, shake`, e.g. `Name (surprised, shake): \"line\"`; at most two per turn.\n\
             - Scene: `[bg: location | time]` on its own line, only when the location or time changes. A new location clears the stage.\n\
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
             - 換場景：`[bg: 地點 | 時間]` 單獨一行，只在地點或時間改變時寫；換地點＝舞台清空。\n\
             - 選項：只在劇情走到需要玩家抉擇的分岔點時，在正文最後寫 `[choices]`，下面每行 `- 選項` 共 2–4 個；其他時候不要出選項。\n\
             - 標記一律單獨一行、關鍵字用英文小寫；不要用 Markdown 標題、粗體或清單（選項行除外）。\n\
             範例（名字只是示意）：\n\
             [bg: 港口 | 黃昏]\n海風帶著鹹味吹過碼頭。\n\n\
             艾琳 (happy): 「你終於來了。」\n艾琳 (sad) thought: 他還記得那天的事嗎……\n\n\
             [choices]\n- 問她在等誰\n- 默默走到她身邊\n"
        ),
    }
}

/// 角色線 system 附加段（全卡共用的穩定前綴，不帶名字；這一輪演誰在回合尾指定）。
pub fn vn_character_rules(lang: &str) -> String {
    let expressions = VN_EXPRESSIONS.join(", ");
    match scaffold_en(lang) {
        true => format!(
            "\n## Visual novel script format\n\
             This table is played as a visual novel. Write only the assigned character's own part: dialogue as `Name (expression): \"line\"` \
             (expressions: {expressions}), inner thoughts as `Name (expression) thought: text`, and actions or expressions as narration; \
             a blank line starts the next cell. Use the name exactly as on the list. Do not write markers such as `[bg]` or `[choices]`, \
             and do not speak for other characters or the player.\n"
        ),
        false => format!(
            "\n## 視覺小說劇本格式\n\
             這桌以視覺小說呈現。每一輪只寫被指定角色自己的部分：對白 `名字 (表情): 「台詞」`（表情限 {expressions}）、\
             內心獨白 `名字 (表情) thought: 台詞`，動作與神情寫成旁白，空一行換下一格。名字照名單原字。\
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

/// VN 桌 GM 回合尾的導演指示：劇本正文→選項區塊→```state 圍欄→「下一位」行（名單非空才有）。
pub fn vn_narrate_instruction(
    lang: &str,
    roster: &[String],
    player_name: Option<&str>,
    updates: bool,
) -> ChatMessage {
    let fence = state_fence_rule(lang);
    let mut instruction = match scaffold_en(lang) {
        true => format!(
            "(Director instruction) Write this beat in the VN script format: advance the plot and describe the scene and the world's response. \
             You may portray supporting characters without character cards; do not speak for listed characters or the player, they reply on their own turns. \
             Do not output a status bar or any explanation outside the format. Order: script text (including `[bg]` and dialogue) → the `[choices]` block if needed → \
             the ```state fence → the `Next:` line. After the script text, start a new line and {fence}\
             The fence's `place` and `time` must match the latest `[bg]`."
        ),
        false => format!(
            "（導演指示）請以視覺小說劇本格式寫這一拍：推進劇情、描述場景與世界反應。沒有角色卡的配角由你扮演；\
             不要替「登場角色」名單上的角色或玩家說話，他們會在自己的回合接話。不要輸出狀態欄或格式以外的說明。\
             輸出順序：劇本正文（含 `[bg]` 與對白）→ 需要時的 `[choices]` 區塊 → ```state 圍欄 → 「下一位」行。\
             劇本正文寫完後另起一行，{fence}圍欄的 place、time 要與最近一次 `[bg]` 一致。"
        ),
    };
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
