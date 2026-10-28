use super::messages::{message, scaffold_en, ChatMessage};

use super::assemble::PLAYER_SENTINEL;
use super::state_yaml::{parse_state_fields, ParsedFields};

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
/// `updates`：這桌開了增量協定，圍欄之後另附 `<UpdateVariable>`（插在「下一位」之前，順序才不打架）。
pub fn narrate_instruction(
    lang: &str,
    roster: &[String],
    player_name: Option<&str>,
    updates: bool,
) -> ChatMessage {
    let mut instruction = if scaffold_en(lang) {
        "(Director instruction) Insert a narration: describe scene changes, the world's response, or plot progress. \
         Use any length the story needs. You may portray supporting characters without character cards, but do not speak for listed characters or the player. \
         If the worldbook or character settings require extra content in every reply (such as a status bar or suggested actions), add it after the narration in the format they specify. When all of that is written, start a new line and output one ```state fence. Inside it, write one `key: value` field per line. \
         Always output `time`, `place`, and `present`; write `time` and `place` in the player's language and repeat them if unchanged. \
         Output `present` in full every turn: based on this turn's story as it stands at the end of the narration, list only the people who are actually at `place` at that moment. \
         The character roster only identifies who people are; it is not a list of who is here, so do not list anyone just because they are on the roster, were mentioned, or are expected to come. \
         If nobody is there, leave `present:` empty. As an exception to the language rule, write each `present` name exactly as it appears in the cast or setting (same characters, no translation, transliteration, or script change), separated by `、`. \
         Do not add any explanation beyond the content above."
            .to_owned()
    } else {
        "（導演指示）請插入一段旁白：描述場景變化、世界反應或劇情推進，\
         篇幅不設限，依劇情需要自由發揮。沒有角色卡的配角由你扮演，可以出場與說話；\
         不要替「登場角色」名單上的角色或玩家說話。世界書或角色設定若規定每回合回覆要附的內容（例如狀態欄、推薦行動），\
         照它的格式附在旁白本文之後。全部寫完後另起一行，輸出一個 ```state 圍欄，\
         圍欄內一行一欄、格式為「鍵: 值」，固定輸出 time、place、present 三個鍵；\
         time、place 用玩家的語言寫，沒有變化就照抄目前值。\
         present 每一回合都要完整輸出：依本輪敘事、以旁白結束當下為準，只列此刻確實身在 place 的人物；\
         人物名冊只供辨識是誰，不是在場名單，不因列在名冊、被提到或約好要見就列；沒有人在場就寫「present:」後面留空。\
         present 的名字照登場角色名單或設定裡的原字逐字寫，不翻譯、不音譯、不換簡繁字形（這是語言規範的例外）；多人用頓號分隔。\
         除了上述內容，不要有多餘說明。"
            .to_owned()
    };
    if updates {
        instruction.push_str(narration_update_note(lang));
    }
    if !roster.is_empty() {
        let call = if scaffold_en(lang) {
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

/// GM 回合尾該用哪一種導演指示。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmTurnFormat {
    /// 卡片自帶介面、還沒被 App 接管：卡片自己規定了輸出格式，照卡片格式輸出，
    /// 否則兩套指令打架、卡片的介面永遠對不上。
    CardFormat,
    /// 卡片自帶介面，但它的格式條目全文不在本輪提示裡（停用、keyword 沒命中、根本沒有）：
    /// 不點名缺席的格式，只交代遵循上下文已有的格式與狀態更新協定。由 `card_format_turn`
    /// 依格式條目判定，`gm_turn_format` 不會回這個值。
    CardFormatAbsent,
    /// 介面已由 App 接管（桌上有重構產的介面骨架）：固定格式由 App 用狀態樹組，模型只寫正文＋
    /// `<UpdateVariable>` 變動，與 system 的增量協定一致；不要 ```state 圍欄、不顯示頂部狀態欄。
    InterfaceTakeover,
    /// 一般旁白＋```state 圍欄（沒重構的桌、角色優先桌）。
    Narration,
}

/// 依據：實際的介面骨架＋玩法標記。只看 guide 不準——清殼後 guide 可能還留著，MVU 桌開了增量但
/// guide 是空的。角色優先（characters）桌一律一般旁白。
pub fn gm_turn_format(
    has_card_scripts: bool,
    has_interface_shell: bool,
    refactor_mode: Option<&str>,
) -> GmTurnFormat {
    if refactor_mode == Some("characters") {
        GmTurnFormat::Narration
    } else if has_interface_shell {
        GmTurnFormat::InterfaceTakeover
    } else if has_card_scripts {
        GmTurnFormat::CardFormat
    } else {
        GmTurnFormat::Narration
    }
}

/// 介面接管桌的導演指示：正文之後只附 `<UpdateVariable>`（只寫有變動的欄位，沒變動就不寫），
/// 不要 ```state 圍欄、不重印狀態區塊——與 system 的增量協定同一份契約，兩邊不互斥。
/// 這種桌只走 GM 一條敘事線（作者裁決 2026-10-27）：名單上的角色也由 GM 演，不點名下一位。
pub fn takeover_instruction(lang: &str) -> ChatMessage {
    let instruction = if scaffold_en(lang) {
        "(Director instruction) Write this turn's story: describe scene changes, the world's response, or plot progress. \
         Use any length the story needs. You play every character, including those on the Characters list, and they may appear and speak; never speak or act for the player. \
         After the story, start a new line and output the `<UpdateVariable>` update block, writing only the fields that changed this turn; if nothing changed, omit the block. \
         Copy every path and every key inside values exactly as they appear in the current state (same characters, no translation or script change). \
         The app draws the status display itself: do not output a ```state fence, do not reprint any status block, and add no explanation outside the story and the update block."
    } else {
        "（導演指示）請寫這一回合的劇情正文：描述場景變化、世界反應或劇情推進，\
         篇幅不設限，依劇情需要自由發揮。所有角色（含「登場角色」名單上的角色）都由你扮演，可以出場與說話；\
         不要替玩家說話或行動。正文結束後另起一行，輸出 `<UpdateVariable>` 更新區塊，\
         只寫這一回合有變動的欄位；沒有任何變動就不要輸出更新區塊。\
         路徑與值裡的鍵名一律照目前狀態裡的原字逐字抄，不翻譯、不換簡繁字形。\
         狀態畫面由 App 自己畫：不要輸出 ```state 圍欄，不要重印任何狀態區塊，正文與更新區塊以外不要有多餘說明。"
    };
    message("user", instruction.to_owned())
}

/// 卡片自帶介面桌的導演指示＋收尾句：同一個判定（格式條目全文在不在本輪提示）同時決定兩者。
/// `entry_title` 由 `import::card_format_entry` 從 `gm_prompt_full_entries` 裡找。
pub fn card_format_turn(lang: &str, entry_title: Option<&str>) -> (ChatMessage, &'static str) {
    let format = match entry_title {
        Some(_) => GmTurnFormat::CardFormat,
        None => GmTurnFormat::CardFormatAbsent,
    };
    (
        card_format_instruction(lang, entry_title),
        gm_closing(format, false, lang),
    )
}

/// 有格式條目就點名它、要求完全照規定；沒有就用中性版——不指向缺席的格式，
/// 也不禁狀態欄或更新區塊（卡片協定可能要它們）。
pub fn card_format_instruction(lang: &str, entry_title: Option<&str>) -> ChatMessage {
    let instruction = match (scaffold_en(lang), entry_title) {
        (true, Some(title)) => format!(
            "(Director instruction) This table uses the interface that ships with the card, and the card already defines the reply format (see the worldbook entry \"{title}\"). \
             Follow that specification exactly for this turn: same tags, same block order, same counts, same required fields, with the content advancing the story. \
             Do not rewrite it as ordinary narration, and do not output anything outside that format."
        ),
        (true, None) => "(Director instruction) Advance this turn's story, following the card's format requirements and the state update protocol already given in the context. \
             If the context gives no additional format, simply continue the story; do not add format explanations or ask for a specification."
            .to_owned(),
        (false, Some(title)) => format!(
            "（導演指示）這桌使用卡片自帶的介面，卡片已經規定了回覆的輸出格式（見世界書「{title}」）。\
             請完全依照那份規定產生本回合的回覆：標籤、區塊順序、數量與必填欄位都照規定，內容依劇情推進。\
             不要改寫成一般旁白，也不要輸出規定格式以外的任何說明或狀態欄。"
        ),
        (false, None) => "（導演指示）請推進本回合的劇情，遵循上下文中已有的卡片格式要求與狀態更新協定；\
             若上下文沒有提供額外的格式，就直接續寫劇情，不要加入格式說明，也不要要求補充規格。"
            .to_owned(),
    };
    message("user", instruction)
}

/// GM 回合 CLI 收尾句（lane 路徑接在導演指示後面）；`has_roster`＝有點名行。
pub fn gm_closing(format: GmTurnFormat, has_roster: bool, lang: &str) -> &'static str {
    match (scaffold_en(lang), format, has_roster) {
        (true, GmTurnFormat::CardFormat, _) => {
            "Now, as the GM, produce this turn's reply exactly in the output format above. Do not add a name prefix, and do not output anything outside that format."
        }
        (true, GmTurnFormat::CardFormatAbsent, _) => {
            "Now, as the GM, carry out the director instruction above and produce this turn's reply. Do not add a name prefix."
        }
        // 介面接管桌不點名，名冊有無都同一句
        (true, GmTurnFormat::InterfaceTakeover, _) => {
            "Now, as the GM, carry out the director instruction above. Output only the story and, if anything changed, the update block. Do not add a name prefix."
        }
        (true, GmTurnFormat::Narration, false) => {
            "Now, as the GM, carry out the director instruction above. Output only the narration, any content the settings require, and the required ```state fence. Do not add a name prefix."
        }
        (true, GmTurnFormat::Narration, true) => {
            "Now, as the GM, carry out the director instruction above. Output only the narration, any content the settings require, the required ```state fence, and the `Next:` line. Do not add a name prefix."
        }
        (false, GmTurnFormat::CardFormat, _) => {
            "現在請以 GM 身分，完全依照上述輸出格式產生本回合的回覆，不要加名字前綴，也不要輸出格式以外的任何內容。"
        }
        (false, GmTurnFormat::CardFormatAbsent, _) => {
            "現在請以 GM 身分執行上述導演指示，產生本回合的回覆，不要加名字前綴。"
        }
        (false, GmTurnFormat::InterfaceTakeover, _) => {
            "現在請以 GM 身分執行上述導演指示，只輸出劇情正文與有變動時的更新區塊，不要加名字前綴。"
        }
        (false, GmTurnFormat::Narration, false) => {
            "現在請以 GM 身分執行上述導演指示，只輸出旁白本文、設定規定要附的內容與要求的 ```state 圍欄，不要加名字前綴。"
        }
        (false, GmTurnFormat::Narration, true) => {
            "現在請以 GM 身分執行上述導演指示，只輸出旁白本文、設定規定要附的內容、要求的 ```state 圍欄與「下一位」行，不要加名字前綴。"
        }
    }
}

/// 一般旁白桌開了增量協定（收編過規則或觸發）：```state 圍欄只管 time／place／present，其餘欄位的變動
/// 要另附 `<UpdateVariable>`，不然模型以為圍欄就是全部、規則欄位永遠沒人回報（ai-card-refactor 紅 3）。
pub fn narration_update_note(lang: &str) -> &'static str {
    match scaffold_en(lang) {
        true => " After the fence, if any field under \"Current state\" other than time, place, and present changed this turn, also output a `<UpdateVariable>` update block with only those changes, copying each path exactly as it appears there.",
        false => "圍欄之後，「目前狀態」裡 time、place、present 以外的欄位這一回合有變動時，另附一個 `<UpdateVariable>` 更新區塊，只寫有變動的欄位，路徑照「目前狀態」裡的原字逐字抄。",
    }
}

/// `narration_update_note` 那種桌的收尾句：允許更新區塊。
pub fn narration_closing_with_updates(has_roster: bool, lang: &str) -> &'static str {
    match (scaffold_en(lang), has_roster) {
        (true, false) => "Now, as the GM, carry out the director instruction above. Output only the narration, any content the settings require, the required ```state fence, and the update block if anything else changed. Do not add a name prefix.",
        (true, true) => "Now, as the GM, carry out the director instruction above. Output only the narration, any content the settings require, the required ```state fence, the update block if anything else changed, and the `Next:` line. Do not add a name prefix.",
        (false, false) => "現在請以 GM 身分執行上述導演指示，只輸出旁白本文、設定規定要附的內容、要求的 ```state 圍欄與其他欄位有變動時的更新區塊，不要加名字前綴。",
        (false, true) => "現在請以 GM 身分執行上述導演指示，只輸出旁白本文、設定規定要附的內容、要求的 ```state 圍欄、其他欄位有變動時的更新區塊與「下一位」行，不要加名字前綴。",
    }
}

/// 換場摘要 CLI 收尾句。
pub fn summary_closing(lang: &str) -> &'static str {
    match scaffold_en(lang) {
        true => "Now carry out the director instruction above. Output only the summary body. Do not add a name prefix.",
        false => "現在請執行上述導演指示，只輸出摘要本文，不要加名字前綴。",
    }
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
/// 剝除後的顯示文字、清單與具名容器路徑。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBlock {
    pub fields: Vec<(Vec<String>, String)>,
    pub updates: Vec<String>,
    pub display: String,
    /// 狀態區塊裡的清單容器路徑：套用時整支替換（見 `mechanism::apply_block_typed`）
    pub lists: Vec<Vec<String>>,
    /// 具名容器路徑：樹上該處若是清單出身的索引分支，套用時先整支移除再寫
    pub maps: Vec<Vec<String>>,
    /// 回覆裡有卡片自帶狀態區塊時，只由 ```state 圍欄收到的欄位（`None`＝沒有卡片狀態區塊，全部欄位都來自圍欄）。
    /// 卡片狀態欄退場的桌只套這份（`mechanism::BlockScope::RetiredCardBlocks`）。
    pub fenced: Option<BlockFields>,
}

/// 狀態區塊合併後的欄位、清單與具名容器路徑。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockFields {
    pub fields: Vec<(Vec<String>, String)>,
    pub lists: Vec<Vec<String>>,
    pub maps: Vec<Vec<String>>,
}

/// 從 GM 回覆剝出狀態區塊。
/// 標籤比對一律走 to_ascii_lowercase——full lowercase 會改變某些字母的長度（如土耳其文 İ），
/// 算出的位移拿回原字串切片就會切在非字元邊界上 panic。
pub fn extract_state_block(reply: &str) -> StateBlock {
    let mut display = reply.to_owned();
    // 狀態區塊（原文起點, 內容）；display 一路被剝，起點換回原文座標靠 removals 逐筆倒推
    let mut blocks: Vec<(usize, String)> = Vec::new();
    let mut removals: Vec<(usize, usize)> = Vec::new();
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
        blocks.push((
            original_offset(&removals, start),
            inner[summary_end + "</summary>".len()..].to_owned(),
        ));
        cut(
            &mut display,
            &mut removals,
            start..end_start + "</details>".len(),
        );
        details_cursor = start;
        removed = true;
    }

    while let Some(tag) = find_state_tag(&display) {
        let content = display[tag.content_start..tag.content_end].to_owned();
        if tag.collect {
            blocks.push((original_offset(&removals, tag.start), content));
        } else {
            // UpdateVariable 是 MVU 的 JSON patch，原始內容交給 mechanism::parse_updates 解析。
            updates.push(content);
        }
        cut(&mut display, &mut removals, tag.start..tag.end);
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
            cut(
                &mut display,
                &mut removals,
                close_start..close_start + "</maintext>".len(),
            );
        }
        cut(&mut display, &mut removals, start..content_start);
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
    let fence_blocks: Vec<_> = fences
        .iter()
        .map(|(start, _, block)| (original_offset(&removals, *start), block.clone()))
        .collect();
    for (start, end, _) in fences.into_iter().rev() {
        display.replace_range(start..end, "");
        removed = true;
    }
    // 卡片自帶的狀態區塊（`<details>` 狀態欄、`<status…>` 標籤）與 ```state 圍欄分開記：狀態欄退場的桌只認圍欄
    let has_card_blocks = !blocks.is_empty();
    blocks.extend(fence_blocks.iter().cloned());

    if !removed {
        return StateBlock {
            display: reply.to_owned(),
            ..StateBlock::default()
        };
    }

    let merged = merge_blocks(blocks);
    StateBlock {
        fields: merged.fields,
        updates,
        display: display.trim_end().to_owned(),
        lists: merged.lists,
        maps: merged.maps,
        fenced: has_card_blocks.then(|| merge_blocks(fence_blocks)),
    }
}

/// 一批狀態區塊合併成欄位：依原文起點排序、後塊勝出，固定鍵改內部鍵、純量清單併進平欄。
fn merge_blocks(mut blocks: Vec<(usize, String)>) -> BlockFields {
    // 依各區塊在原文的起點合併（上面是依外殼種類分批收的），「最後一塊勝出」才是原文的最後一塊。
    blocks.sort_by_key(|(start, _)| *start);
    let mut parsed = ParsedFields::default();
    for (_, block) in &blocks {
        let next = parse_state_fields(block);
        // 同一回覆多個區塊寫同一清單：後塊整份勝出，前塊在這支底下留的欄位與容器標記一併作廢；
        // 後塊把前塊的清單改報成具名形（兩項變一項）也一樣
        let replaced: Vec<Vec<String>> = next
            .lists
            .iter()
            .chain(next.maps.iter().filter(|map| parsed.lists.contains(map)))
            .cloned()
            .collect();
        for list in &replaced {
            parsed.fields.retain(|(path, _)| !path.starts_with(list));
            parsed.lists.retain(|path| !path.starts_with(list));
            parsed.maps.retain(|path| !path.starts_with(list));
        }
        parsed.fields.extend(next.fields);
        parsed.lists.extend(next.lists);
        parsed.maps.extend(next.maps);
    }
    let ParsedFields {
        mut fields,
        mut lists,
        maps,
    } = parsed;
    for (path, _) in &mut fields {
        if path.len() == 1 {
            path[0] = table_key(&path[0]).unwrap_or(&path[0]).to_owned();
        }
    }
    // 時間／地點／在場人物寫成純量清單：以「、」串起進平欄，不進樹（空清單寫空值）
    lists.retain(|list| {
        let [name] = list.as_slice() else {
            return true;
        };
        let Some(key) = table_key(name) else {
            return true;
        };
        let under: Vec<&(Vec<String>, String)> = fields
            .iter()
            .filter(|(path, _)| path.starts_with(list))
            .collect();
        if under.iter().any(|(path, _)| path.len() != 2) {
            return true;
        }
        let joined = under
            .iter()
            .map(|(_, value)| value.as_str())
            .collect::<Vec<_>>()
            .join("、");
        fields.retain(|(path, _)| !path.starts_with(list));
        fields.push((vec![key.to_owned()], joined));
        false
    });
    // 在場人物寫成裸空值（`present:`）＝沒有人在場，跟空清單一樣寫空值進平欄；
    // 時間、地點與其他鍵的裸空值照舊只標空分支（不清掉平欄舊值）
    let mut maps = maps;
    maps.retain(|map| {
        let [name] = map.as_slice() else {
            return true;
        };
        if table_key(name) != Some("present")
            || fields.iter().any(|(path, _)| path.starts_with(map))
        {
            return true;
        }
        fields.push((vec!["present".to_owned()], String::new()));
        false
    });
    BlockFields {
        fields,
        lists,
        maps,
    }
}

/// 從 display 剝掉一段，記下（剝除處, 長度）供 `original_offset` 倒推原文座標。
fn cut(display: &mut String, removals: &mut Vec<(usize, usize)>, range: std::ops::Range<usize>) {
    removals.push((range.start, range.len()));
    display.replace_range(range, "");
}

/// display 目前的位移換回原文位移：由最後一次剝除往前倒推，落在剝除處之後就加回被剝的長度。
fn original_offset(removals: &[(usize, usize)], offset: usize) -> usize {
    removals.iter().rev().fold(offset, |position, (at, len)| {
        if position >= *at {
            position + len
        } else {
            position
        }
    })
}

/// 平欄的三個固定鍵（時間、地點、在場人物）各種寫法對回內部鍵。
fn table_key(key: &str) -> Option<&'static str> {
    match key.to_ascii_lowercase().as_str() {
        "time" | "時間" | "时间" => Some("time"),
        "place" | "location" | "地點" | "地点" => Some("place"),
        "present" | "在場" | "在场" | "在場人物" | "在场人物" => Some("present"),
        _ => None,
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
mod tests;
