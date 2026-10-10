use crate::data::{CharacterCard, Mechanism, TableState, TranscriptEvent, TranscriptKind};
use crate::world_scan::{Placed, WorldScan};

use super::messages::{
    language_rule, message, narration_line, player_fallback_name, push_merged, scaffold_en,
    speaker_prefix, system_line, ChatMessage,
};

use super::arrivals::{prompt_speaker, prompt_text, Side};

use super::context::{gm_system_prompt, gm_worldbook, player_heading};
use super::worldbook::section_body;

use super::state_view::{character_state_block, gm_dynamic_block, StateScope};

/// resume 續聊線（prompt-cache-optimization 包 2）的回合尾段。
/// tail 是跟在新事件後送出的動態文字；confidential 是 tail 內回合結束後
/// 要從 session 檔抹掉的子段（chars 線的私設＋限定條目，防洩漏給下一個被點的角色）。
pub struct LaneTurn {
    pub tail: String,
    pub confidential: Option<String>,
    /// `hoist` 不是 `None` 時要提進凍結 system 的段落（呼叫端接在 system 後面）：本輪角色的私設，
    /// 加上世界書——`StableConfidential` 只收穩定的機密條目，`All` 收本輪觸發的全部世界書。
    pub hoisted_private: Option<String>,
    /// `hoisted_private` 裡的世界書部分（`Hoist::All` 才有）：它一變，不抹尾段的線就要重開、不走補丁
    /// （補丁回合後不抹，舊的世界書會留在 session 歷史裡，方案三之 3）
    pub hoisted_worldbook: Option<String>,
    /// tail 含本輪角色狀態區塊（claude 單角色線靠它偵測整塊消失）
    pub has_state_block: bool,
}

/// 角色回合的世界書與私設要不要提進凍結 system（方案三之 3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hoist {
    /// 全角色共用一條線（Claude／Grok 共線、API 多卡）：私設與機密條目都在回合尾段的機密段
    None,
    /// API 單卡：私設與穩定的機密條目進 system，其餘照尾段（API 無狀態，尾段不會疊）
    StableConfidential,
    /// 不抹尾段的線（Agy 一角一線、單人在場的 Claude）：私設與本輪觸發的全部世界書都進 system，
    /// 尾段不放世界書（不然每輪的世界書段會在 session 歷史裡一份份疊上去）
    All,
}

/// 事件在 lane prompt 裡的一行；`None`＝這則不送。續聊線的歷史全部以名字標注成純文字
/// （誰說的靠「X：」前綴分辨，不靠 role），與 session 內既有歷史逐字銜接。
/// 可見性與標頭照 `prompt_text`：chars 線傳 `Side::Character`，GM 線傳 `Side::Gm`。
pub fn lane_event_line(event: &TranscriptEvent, lang: &str, side: Side) -> Option<String> {
    let text = prompt_text(event, lang, side)?;
    Some(match event.kind {
        TranscriptKind::Dialogue | TranscriptKind::Player => {
            format!(
                "{}{text}",
                speaker_prefix(&prompt_speaker(event, lang), lang)
            )
        }
        TranscriptKind::Narration => narration_line(&text, lang),
        TranscriptKind::System => system_line(&text, lang),
    })
}

/// chars 線「你知道的世界情報」段標（凍結 system 與回合尾共用），結尾帶換行。
fn known_world_heading(lang: &str) -> &'static str {
    match scaffold_en(lang) {
        true => "## World knowledge you have\n",
        false => "## 你知道的世界情報\n",
    }
}

/// chars 線「你知道的世界情報」後組段標（凍結 system 與回合尾共用），結尾帶換行。
fn known_world_back_heading(lang: &str) -> &'static str {
    match scaffold_en(lang) {
        true => "## World knowledge you have, continued\n",
        false => "## 你知道的世界情報（續）\n",
    }
}

/// chars 線凍結 system（快照）：中性扮演引擎指示＋全部公開角色卡＋玩家卡＋共用快照的靜態世界書條目
/// （`TableBook::snapshot`：啟用、constant、`Public`、穩定、位置前／後／範例上下，不看任何一位角色的掃描）。
/// 全角色共用一條 session，這一輪演誰由回合尾段指定；私設與限定條目不進快照
/// （E7：凍結 system 動一字整條快取全滅，快照只能放全員共通且穩定的素材）。
/// 公開設定含動態巨集的卡（`CardTexts::dynamic`）只留名字，設定本文改在回合尾（`dynamic_profiles`）。
/// constant 人物條目改走名冊行，不進全文（包 4a）。
pub fn chars_lane_system(
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    scan: &WorldScan,
    lang: &str,
) -> String {
    let en = scaffold_en(lang);
    let intro = match en {
        true => {
            "You are the roleplay engine for this multiplayer tabletop RPG session. Any character on the \
             \"Characters\" list may be played by you, and the end of each turn tells you whom to play this turn. \
             Always narrate in the third person: actions and inner thoughts take the assigned character's name or \
             \"he/she\" as the subject, and spoken words go in quotation marks in that character's own voice. \
             Keep the viewpoint with the assigned character: you may describe what they see around them and what \
             they feel, but not what they do not know. Do not use \"I\" as the narrative subject, do not break \
             character, do not speak as an AI assistant, and do not speak for other characters or the player."
        }
        false => {
            "你是這場多人桌上角色扮演的扮演引擎，「登場角色」名單上的角色都可能由你扮演，\
             每一輪的結尾會指定你這一輪演誰。請一律用第三人稱敘事：動作與心理描寫都以\
             被指定角色的名字或「他／她」當主詞，說出口的話寫在引號裡、維持這個角色自己的口吻；\
             視角只跟著被指定的角色，可以寫他眼中所見的環境與心裡的感受，不要寫他不知道的事；\
             敘述不要用「我」當主詞、不要跳出角色、不要以 AI 助理的身分說話、\
             不要替其他角色或玩家代言。"
        }
    };
    let separator = if en { " " } else { "" };
    let mut system = format!("{intro}{separator}{}\n", language_rule(lang));
    if !cards.is_empty() {
        system.push_str(match en {
            true => "\n## Characters (public profiles)\n",
            false => "\n## 登場角色（公開設定）\n",
        });
        for card in cards {
            let texts = scan.texts.card(card);
            system.push_str(&format!("### {}\n", card.name));
            if !texts.dynamic && !texts.public.trim().is_empty() {
                system.push_str(&format!("{}\n", texts.public.trim()));
            }
        }
    }
    if let Some(player) = player {
        system.push_str(&player_heading(&player.name, lang));
        let texts = &scan.texts.player;
        if !texts.dynamic && !texts.public.trim().is_empty() {
            system.push_str(&format!("\n{}\n", texts.public.trim()));
        }
    }
    system.push_str(&snapshot_worldbook(scan, lang));
    system
}

/// 共用快照的世界書段落（`chars_lane_system` 尾端那幾段，含開頭換行）；沒有內容回空字串。
/// 不抹尾段的線拿它和 hoist 的世界書一起算指紋：快照條目改了或刪了也要重開。
fn snapshot_worldbook(scan: &WorldScan, lang: &str) -> String {
    let front: Vec<&Placed> = scan
        .snapshot_entries
        .iter()
        .filter(|entry| entry.in_front())
        .collect();
    let back: Vec<&Placed> = scan
        .snapshot_entries
        .iter()
        .filter(|entry| entry.in_back())
        .collect();
    let mut text = String::new();
    for (heading, entries) in [
        (known_world_heading(lang), front),
        (known_world_back_heading(lang), back),
    ] {
        let body = section_body(&entries, lang);
        if !body.is_empty() {
            text.push('\n');
            text.push_str(heading);
            text.push_str(&body);
        }
    }
    text
}

/// 凍結 system 裡會變的全部內容（共用快照段＋提進來的本輪世界書與動態公開設定）：不抹尾段的線以它為重開依據。
pub fn system_worldbook(scan: &WorldScan, hoisted: Option<&str>, lang: &str) -> String {
    let mut text = snapshot_worldbook(scan, lang);
    text.push_str(hoisted.unwrap_or_default());
    text
}

/// 公開設定含動態巨集的卡（與玩家卡）：本輪以這個角色的視角代換好的公開設定（三之 3）。標題仍是公開設定，
/// 放在回合後會抹掉的段落（共線回合後不抹的公開段會每輪疊一份）；沒有回空字串。
fn dynamic_profiles(
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    scan: &WorldScan,
    lang: &str,
) -> String {
    let mut body = String::new();
    for card in cards {
        let texts = scan.texts.card(card);
        if texts.dynamic && !texts.public.trim().is_empty() {
            body.push_str(&format!("### {}\n{}\n", card.name, texts.public.trim()));
        }
    }
    if let Some(player) = player {
        let texts = &scan.texts.player;
        if texts.dynamic && !texts.public.trim().is_empty() {
            body.push_str(&format!(
                "{}\n{}\n",
                player_heading(&player.name, lang).trim_start_matches('\n'),
                texts.public.trim()
            ));
        }
    }
    if body.is_empty() {
        return body;
    }
    let heading = match scaffold_en(lang) {
        true => "## Characters (public profiles, this turn)\n",
        false => "## 登場角色（公開設定，本輪）\n",
    };
    format!("{heading}{body}")
}

/// 「只有某角色知道的世界情報」段：限定可見與私密觸發的條目；沒有內容回空字串。
fn limited_block(entries: &[&Placed], card: &CharacterCard, lang: &str) -> String {
    let body = section_body(entries, lang);
    if body.is_empty() {
        return body;
    }
    let heading = match scaffold_en(lang) {
        true => format!("## World knowledge only {} has\n", card.name),
        false => format!("## 只有「{}」知道的世界情報\n", card.name),
    };
    format!("{heading}{body}")
}

/// chars 線「你知道的世界情報」段（本輪公開觸發的 `Public` 條目，共用快照已有的不重複）；沒有內容回空字串。
fn public_block(entries: &[&Placed], lang: &str) -> String {
    let body = section_body(entries, lang);
    if body.is_empty() {
        return body;
    }
    format!("{}{body}", known_world_heading(lang))
}

/// chars 線回合尾段：本輪公開觸發的 `Public` 條目＋機密段（動態公開設定＋本輪角色的私設＋限定可見與私密觸發的
/// 條目）＋本輪指定。機密段回合結束後從 session 檔抹掉；共用快照的靜態條目已在凍結 system，不重複。
/// `hoist` 決定私設與世界書要不要改提進 system（見 `Hoist`）。`cards` 是凍結 system 列的那份卡清單。
#[allow(clippy::too_many_arguments)]
pub fn chars_lane_turn(
    card: &CharacterCard,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    scan: &WorldScan,
    state: &TableState,
    mechanism: &Mechanism,
    branch: Option<&[String]>,
    lang: &str,
    hoist: Hoist,
) -> LaneTurn {
    let user_name = player
        .map(|player| player.name.as_str())
        .unwrap_or_else(|| player_fallback_name(lang));
    let (confidential_entries, public_entries): (Vec<&Placed>, Vec<&Placed>) = scan
        .placed
        .iter()
        .filter(|entry| !scan.snapshot.contains(&entry.uid))
        .partition(|entry| entry.confidential());
    let public = public_block(&public_entries, lang);
    let profiles = dynamic_profiles(cards, player, scan, lang);
    let own = scan.texts.card(card);

    let mut tail = String::new();
    let mut confidential = String::new();
    let mut hoisted = String::new();
    let mut hoisted_worldbook = None;
    if !profiles.is_empty() && hoist != Hoist::All {
        confidential.push_str(&profiles);
    }
    if !own.private.trim().is_empty() {
        let heading = match scaffold_en(lang) {
            true => format!(
                "## {}'s private profile (only they know this; do not reveal it unless the story gets there)",
                card.name
            ),
            false => format!(
                "## 「{}」的私有設定（只有他自己知道；除非劇情走到，不要主動說破）",
                card.name
            ),
        };
        let block = format!("{heading}\n{}\n", own.private.trim());
        match hoist {
            Hoist::None => confidential.push_str(&block),
            Hoist::StableConfidential | Hoist::All => hoisted.push_str(&block),
        }
    }
    match hoist {
        Hoist::None => {
            if !public.is_empty() {
                tail.push_str(&public);
                tail.push('\n');
            }
            confidential.push_str(&limited_block(&confidential_entries, card, lang));
        }
        Hoist::StableConfidential => {
            if !public.is_empty() {
                tail.push_str(&public);
                tail.push('\n');
            }
            // 穩定、前／後組的機密條目每輪都一樣，跟私設一起進 system；其餘隨掃描翻動，照舊走尾段
            let (stable, rest): (Vec<&Placed>, Vec<&Placed>) = confidential_entries
                .into_iter()
                .partition(|entry| entry.stable && (entry.in_front() || entry.in_back()));
            hoisted.push_str(&limited_block(&stable, card, lang));
            confidential.push_str(&limited_block(&rest, card, lang));
        }
        Hoist::All => {
            // 動態公開設定與本輪世界書一起進 system、一起算重開指紋
            let mut worldbook = profiles;
            for block in [public, limited_block(&confidential_entries, card, lang)] {
                if !worldbook.is_empty() && !block.is_empty() {
                    worldbook.push('\n');
                }
                worldbook.push_str(&block);
            }
            hoisted.push_str(&worldbook);
            hoisted_worldbook = Some(worldbook);
        }
    }
    let state_block = branch.and_then(|branch| {
        character_state_block(state, mechanism, branch, &card.name, user_name, lang)
    });
    let has_state_block = state_block.is_some();
    if let Some(block) = state_block {
        confidential.push_str(&block);
        confidential.push('\n');
    }
    if !confidential.is_empty() {
        tail.push_str(&confidential);
        tail.push('\n');
    }
    tail.push_str(&match scaffold_en(lang) {
        true => format!(
            "You are now \"{name}\". Write {name}'s actions, lines, and inner thoughts directly in the third person, \
             with \"{name}\" or \"he/she\" as the subject and never \"I\"; put spoken words in quotation marks. \
             Do not add a name prefix or any explanation outside the character.",
            name = card.name
        ),
        false => format!(
            "現在你是「{name}」。請直接用第三人稱輸出「{name}」的動作、台詞與心理描寫，\
             敘述主詞是「{name}」或「他／她」、不要用「我」，說出口的話寫在引號裡；\
             不要加名字前綴、不要任何角色之外的說明。",
            name = card.name
        ),
    });
    LaneTurn {
        tail,
        confidential: (!confidential.is_empty()).then_some(confidential),
        hoisted_private: (!hoisted.is_empty()).then_some(hoisted),
        hoisted_worldbook,
        has_state_block,
    }
}

/// gm 線凍結 system（快照）：GM 指示＋world.md＋穩定觸發的世界書（前組、後組）＋全卡（含私設）＋玩家卡。
/// GM 看得到一切，不分可見度；不穩定的觸發條目在回合尾段（`gm_lane_turn`）。
pub fn gm_lane_system(
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    scan: &WorldScan,
    mechanism: &Mechanism,
    lang: &str,
) -> String {
    let split = gm_worldbook(scan);
    gm_system_prompt(
        &scan.texts,
        cards,
        player,
        &split.system_front,
        &split.system_back,
        mechanism,
        lang,
    )
}

/// gm 線回合尾段：不穩定的觸發條目與作者註記、依深度條目＋目前狀態＋導演指示（旁白＋點名合併版，由呼叫端組好傳入）。
#[allow(clippy::too_many_arguments)]
pub fn gm_lane_turn(
    scan: &WorldScan,
    player: Option<&CharacterCard>,
    state: &TableState,
    mechanism: &Mechanism,
    scope: &StateScope,
    instruction: &str,
    lang: &str,
) -> LaneTurn {
    let user_name = player
        .map(|player| player.name.as_str())
        .unwrap_or_else(|| player_fallback_name(lang));
    let worldbook = section_body(&gm_worldbook(scan).tail, lang);
    let dynamic = gm_dynamic_block(&worldbook, state, user_name, mechanism, scope, lang);
    let mut tail = String::new();
    if !dynamic.is_empty() {
        tail.push_str(&dynamic);
        tail.push_str("\n\n");
    }
    tail.push_str(instruction);
    LaneTurn {
        tail,
        confidential: None,
        hoisted_private: None, // GM 線沒有「本輪角色的私設」這個概念
        hoisted_worldbook: None,
        has_state_block: false,
    }
}

/// 組裝「換場摘要」上下文：GM 檔位讀角色側看得到的 transcript，把本場景壓成一則前情提要。
/// 不含 world.md／角色卡——摘要只需壓縮已發生的公開事件，不需要世界觀全貌。
/// 介面接管桌（`data::is_interface_takeover`）多一句禁標記：摘要會填進卡片介面的正文槽，
/// 標籤或圍欄會被卡的顯示腳本當成結構（渲染層另有佔位防線，這是第二層）。其他桌指示逐字不變。
pub fn summary_messages(
    events: &[TranscriptEvent],
    lang: &str,
    interface_takeover: bool,
) -> Vec<ChatMessage> {
    let en = scaffold_en(lang);
    let plain_only = summary_plain_only(interface_takeover, en);
    let instruction = if en {
        format!(
            "You are the GM of a multiplayer tabletop RPG session that is about to change scenes. \
             The first line of your reply must be exactly \"Title: <act name, 10 words or fewer>\", \
             followed by a blank line before the recap. \
             Summarize everything that happened in this scene as a recap, covering: \
             location and time, who is present and their state, key events, relationship changes, \
             and unresolved threads — as a compact bulleted list. \
             Output only the summary body. {plain_only}{language_rule}",
            language_rule = language_rule(lang),
        )
    } else {
        format!(
            "你是這場多人桌上角色扮演的 GM，現在要換場。\
             回覆第一行固定輸出「標題：〈10 字內的幕名〉」，空一行後才是摘要條列。\
             請把本場景發生的一切壓成一則前情提要，條列涵蓋：\
             地點與時間、在場人物與狀態、關鍵事件、關係變化、未解懸念。\
             {plain_only}{language_rule}",
            language_rule = language_rule(lang),
        )
    };

    let mut messages = vec![message("system", instruction)];
    // 摘要會變成下一幕的公開旁白、回到每個角色的上下文，只能讀角色側看得到的內容
    for line in events
        .iter()
        .filter_map(|event| lane_event_line(event, lang, Side::Character))
    {
        push_merged(&mut messages, "user", line);
    }
    messages
}

fn summary_plain_only(interface_takeover: bool, en: bool) -> &'static str {
    match (interface_takeover, en) {
        (false, _) => "",
        (true, true) => {
            "Write plain narrative text only: no XML or HTML tags, no angle-bracket markup, no code fences. "
        }
        (true, false) => "只寫純文字：不得輸出 XML／HTML 標籤、角括號標記或程式碼圍欄。",
    }
}

/// 換幕摘要的角色側紀錄行（與 `summary_messages` 同一渲染）；分段摘要按這些行切塊。
pub fn summary_lines(events: &[TranscriptEvent], lang: &str) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| lane_event_line(event, lang, Side::Character))
        .collect()
}

/// 分段摘要的上限字數（中間摘要要求寫在這以內；回來超過就重寫或截斷）。
pub const SEGMENT_SUMMARY_CHARS: usize = 1500;

/// 分段摘要：這一幕太長，一次送不進模型時，切成幾段各自壓成條列（第 part／parts 段）。
/// `lines` 第一層是紀錄行，更深的層是上一層的中間摘要。
pub fn segment_summary_messages(
    lines: &[String],
    part: usize,
    parts: usize,
    lang: &str,
) -> Vec<ChatMessage> {
    let instruction = if scaffold_en(lang) {
        format!(
            "You are the GM of a multiplayer tabletop RPG session. This scene is too long to summarize at once, \
             so it is being recapped in parts; this is part {part} of {parts}. \
             Compress everything that happens in this part into a bulleted recap of at most {limit} characters, \
             covering location and time, who is present, key events, relationship changes, and unresolved threads. \
             No title. Output only the bullets. {language_rule}",
            limit = SEGMENT_SUMMARY_CHARS,
            language_rule = language_rule(lang),
        )
    } else {
        format!(
            "你是這場多人桌上角色扮演的 GM。這一幕太長，一次整理不完，正在分段整理；這是第 {part}／{parts} 段。\
             請把這一段發生的事壓成 {limit} 字以內的條列，涵蓋地點與時間、在場人物、關鍵事件、關係變化、未解懸念。\
             不要標題，只輸出條列。{language_rule}",
            limit = SEGMENT_SUMMARY_CHARS,
            language_rule = language_rule(lang),
        )
    };
    let mut messages = vec![message("system", instruction)];
    for line in lines {
        push_merged(&mut messages, "user", line.clone());
    }
    messages
}

/// 合併：各段中間摘要依序交給模型，產出跟一次換幕相同格式的「標題＋前情提要」。
/// 合併的輸出就是前情提要本身，介面接管桌同 `summary_messages` 多一句禁標記。
pub fn merge_summary_messages(
    segments: &[String],
    lang: &str,
    interface_takeover: bool,
) -> Vec<ChatMessage> {
    let plain_only = summary_plain_only(interface_takeover, scaffold_en(lang));
    let instruction = if scaffold_en(lang) {
        format!(
            "You are the GM of a multiplayer tabletop RPG session that is about to change scenes. \
             The scene was recapped in parts; the part recaps follow in order. \
             The first line of your reply must be exactly \"Title: <act name, 10 words or fewer>\", \
             followed by a blank line before the recap. \
             Merge the parts into one recap of the whole scene, covering: location and time, who is present and their state, \
             key events, relationship changes, and unresolved threads — as a compact bulleted list. \
             Output only the summary body. {plain_only}{language_rule}",
            language_rule = language_rule(lang),
        )
    } else {
        format!(
            "你是這場多人桌上角色扮演的 GM，現在要換場。這一幕是分段整理的，以下依序是各段摘要。\
             回覆第一行固定輸出「標題：〈10 字內的幕名〉」，空一行後才是摘要條列。\
             請把各段合併成整幕的一則前情提要，條列涵蓋：地點與時間、在場人物與狀態、關鍵事件、關係變化、未解懸念。\
             {plain_only}{language_rule}",
            language_rule = language_rule(lang),
        )
    };
    let mut messages = vec![message("system", instruction)];
    let total = segments.len();
    for (index, segment) in segments.iter().enumerate() {
        let header = match scaffold_en(lang) {
            true => format!("[Part {}/{total}]", index + 1),
            false => format!("【第 {}／{total} 段】", index + 1),
        };
        push_merged(&mut messages, "user", format!("{header}\n{segment}"));
    }
    messages
}

/// 中間摘要寫太長：請模型縮到上限內。
pub fn shorten_summary_messages(text: &str, lang: &str) -> Vec<ChatMessage> {
    let instruction = if scaffold_en(lang) {
        format!(
            "Shorten the recap below to at most {limit} characters, keeping the key events and unresolved threads. \
             Output only the shortened bullets. {language_rule}",
            limit = SEGMENT_SUMMARY_CHARS,
            language_rule = language_rule(lang),
        )
    } else {
        format!(
            "把下面這段摘要縮短到 {limit} 字以內，保留關鍵事件與未解懸念，只輸出縮短後的條列。{language_rule}",
            limit = SEGMENT_SUMMARY_CHARS,
            language_rule = language_rule(lang),
        )
    };
    vec![
        message("system", instruction),
        message("user", text.to_owned()),
    ]
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::super::arrivals::*;
    #[allow(unused_imports)]
    use super::super::assemble::*;
    #[allow(unused_imports)]
    use super::super::client::*;
    #[allow(unused_imports)]
    use super::super::context::*;
    #[allow(unused_imports)]
    use super::super::messages::*;
    #[allow(unused_imports)]
    use super::super::response::*;
    #[allow(unused_imports)]
    use super::super::state_view::*;
    #[allow(unused_imports)]
    use super::super::test_support::{card, event, worldbook_entry};
    use super::*;
    #[allow(unused_imports)]
    use crate::data::{
        self, AppConfig, CharacterCard, DataResult, FieldKind, FieldRule, InjectLevel, Mechanism,
        StateNode, TableState, Tier, TranscriptEvent, TranscriptKind, Visibility, WorldbookEntry,
    };
    #[allow(unused_imports)]
    use crate::mechanism;
    #[allow(unused_imports)]
    use crate::transport::test_support::legacy::{
        assemble_gm_messages, assemble_shared_messages, chars_lane_system, chars_lane_turn,
        gm_lane_system, gm_lane_turn,
    };
    #[allow(unused_imports)]
    use std::collections::{BTreeMap, BTreeSet};

    /// 驗收：換場摘要指示依語系切換，且 transcript 事件正確攤平成 user 訊息
    #[test]
    fn summary_messages_follow_lang_and_include_transcript() {
        let events = [
            event(TranscriptKind::Narration, "", "GM", "夜幕低垂"),
            event(TranscriptKind::Player, "", "玩家", "老闆，來杯麥酒"),
            event(TranscriptKind::Dialogue, "fox-id", "狐狸", "馬上來！"),
        ];
        let zh = summary_messages(&events, "zh-TW", false);
        assert_eq!(zh[0].role, "system");
        assert!(zh[0].content.contains("前情提要"));
        let joined: String = zh.iter().map(|m| m.content.as_str()).collect();
        assert!(joined.contains("（旁白）夜幕低垂"));
        assert!(joined.contains("玩家：老闆，來杯麥酒"));
        assert!(joined.contains("狐狸：馬上來！"));

        let en = summary_messages(&events, "en", false);
        assert!(en[0].content.contains("recap"));
    }

    /// 接管桌摘要只多一句禁標記（插在語系規則前），其餘逐字相同；transcript 部分不受影響
    #[test]
    fn summary_messages_takeover_adds_only_plain_text_rule() {
        let events = [event(TranscriptKind::Narration, "", "GM", "夜幕低垂")];
        for (lang, rule) in [
            ("zh-TW", "只寫純文字：不得輸出 XML／HTML 標籤、角括號標記或程式碼圍欄。"),
            (
                "en",
                "Write plain narrative text only: no XML or HTML tags, no angle-bracket markup, no code fences. ",
            ),
        ] {
            let plain = summary_messages(&events, lang, false);
            let takeover = summary_messages(&events, lang, true);
            assert!(!plain[0].content.contains(rule));
            let at = plain[0].content.rfind(language_rule(lang)).unwrap();
            let mut expected = plain[0].content.clone();
            expected.insert_str(at, rule);
            assert_eq!(takeover[0].content, expected);
            assert!(takeover[0].content.ends_with(language_rule(lang)));
            assert_eq!(plain[1..], takeover[1..]);
        }
    }

    /// chars 線凍結快照只能有全員共通且穩定的素材：公開卡＋玩家卡＋Public constant。
    /// 私設、GM 專有、角色限定、keyword 條目一律不進快照（回合注入或不可見）。
    #[test]
    fn chars_lane_snapshot_holds_shared_public_material_only() {
        let fox = card("fox-id", "狐狸", "旅店老闆", "其實是通緝犯");
        let knight = card("knight-id", "騎士", "遊歷的騎士", "");
        let player = card("player-id", "阿濤", "商隊護衛", "");
        let entries = [
            worldbook_entry(1, "公開常識", &[], true, 0, false, Visibility::Public),
            worldbook_entry(2, "GM專有", &[], true, 0, false, Visibility::Gm),
            worldbook_entry(
                3,
                "狐狸限定",
                &[],
                true,
                0,
                false,
                Visibility::Characters(vec!["fox-id".to_owned()]),
            ),
            worldbook_entry(
                4,
                "關鍵字條目",
                &["寶箱"],
                false,
                0,
                false,
                Visibility::Public,
            ),
        ];
        let snapshot = chars_lane_system(
            &[fox.clone(), knight.clone()],
            Some(&player),
            &entries,
            "zh-TW",
        );
        assert!(snapshot.contains("扮演引擎"));
        assert!(snapshot.contains("旅店老闆"));
        assert!(snapshot.contains("遊歷的騎士"));
        assert!(snapshot.contains("阿濤"));
        assert!(snapshot.contains("公開常識內容"));
        assert!(!snapshot.contains("通緝犯"));
        assert!(!snapshot.contains("GM專有"));
        assert!(!snapshot.contains("狐狸限定"));
        assert!(!snapshot.contains("關鍵字條目"));
        // 快照不依賴 events，本質上逐輪穩定；再組一次逐字相同
        assert_eq!(
            snapshot,
            chars_lane_system(&[fox, knight], Some(&player), &entries, "zh-TW")
        );
    }

    /// chars 線回合尾段：公開 keyword 條目留在 tail、私設與限定條目集中在 confidential；
    /// confidential 在 tail 中恰好出現一次（回合後靠這個子段從 session 檔抹掉）。
    #[test]
    fn chars_lane_turn_isolates_confidential_segment() {
        let fox = card("fox-id", "狐狸", "旅店老闆", "其實是通緝犯");
        let events = [event(
            TranscriptKind::Player,
            "",
            "阿濤",
            "打開寶箱，讀羊皮卷",
        )];
        let entries = [
            worldbook_entry(1, "公開常識", &[], true, 0, false, Visibility::Public),
            worldbook_entry(
                2,
                "寶箱情報",
                &["寶箱"],
                false,
                0,
                false,
                Visibility::Public,
            ),
            worldbook_entry(
                3,
                "羊皮卷密文",
                &["羊皮卷"],
                false,
                0,
                false,
                Visibility::Characters(vec!["fox-id".to_owned()]),
            ),
            worldbook_entry(
                4,
                "狐狸長設",
                &[],
                true,
                0,
                false,
                Visibility::Characters(vec!["fox-id".to_owned()]),
            ),
        ];
        let turn = chars_lane_turn(
            &fox,
            None,
            &events,
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
            false,
        );
        let confidential = turn.confidential.expect("私設＋限定條目必須進機密段");
        assert!(confidential.contains("通緝犯"));
        assert!(confidential.contains("羊皮卷密文內容"));
        assert!(confidential.contains("狐狸長設內容")); // 限定 constant 也走回合注入
        assert!(!confidential.contains("寶箱情報"));
        assert_eq!(turn.tail.matches(confidential.as_str()).count(), 1);
        // 抹掉機密段後，公開條目與本輪指定仍在（session 歷史剩這些）
        let erased = turn.tail.replacen(confidential.as_str(), "", 1);
        assert!(erased.contains("寶箱情報內容"));
        assert!(erased.contains("現在你是「狐狸」"));
        assert!(!erased.contains("公開常識")); // constant 已在快照，不重複
                                               // 沒有私設也沒有限定條目時不產生機密段
        let knight = card("knight-id", "騎士", "遊歷的騎士", "");
        let plain = chars_lane_turn(
            &knight,
            None,
            &events,
            &entries[..2],
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
            false,
        );
        assert!(plain.confidential.is_none());
        assert!(plain.tail.contains("現在你是「騎士」"));
    }

    /// gm 線凍結快照＝GM 單發 system 的同等素材（全 constant＋全卡含私設＋world.md）；
    /// 回合尾段＝keyword 條目＋目前狀態＋導演指示。
    #[test]
    fn gm_lane_snapshot_and_turn_cover_gm_material() {
        let fox = card("fox-id", "狐狸", "旅店老闆", "其實是通緝犯");
        let events = [event(TranscriptKind::Player, "", "阿濤", "打開寶箱")];
        let entries = [
            worldbook_entry(1, "GM專有", &[], true, 0, false, Visibility::Gm),
            worldbook_entry(
                2,
                "寶箱情報",
                &["寶箱"],
                false,
                0,
                false,
                Visibility::Public,
            ),
        ];
        let snapshot = gm_lane_system(
            "世界總覽",
            &[fox],
            None,
            &entries,
            &Mechanism::default(),
            "zh-TW",
        );
        assert!(snapshot.contains("世界總覽"));
        assert!(snapshot.contains("GM專有內容"));
        assert!(snapshot.contains("通緝犯"));
        assert!(!snapshot.contains("寶箱情報"));

        let mut state = TableState::default();
        state.table.insert("place".to_owned(), "酒館".to_owned());
        let turn = gm_lane_turn(
            &events,
            &entries,
            None,
            &state,
            &Mechanism::default(),
            &StateScope::default(),
            "（導演指示）請插入旁白。",
            "zh-TW",
        );
        assert!(turn.confidential.is_none());
        assert!(turn.tail.contains("寶箱情報內容"));
        assert!(turn.tail.contains("地點：酒館"));
        assert!(turn.tail.ends_with("（導演指示）請插入旁白。"));
    }

    #[test]
    fn lane_event_line_labels_every_kind_by_name() {
        assert_eq!(
            lane_event_line(
                &event(TranscriptKind::Dialogue, "fox-id", "狐狸", "晚安"),
                "zh-TW",
                Side::Gm
            )
            .as_deref(),
            Some("狐狸：晚安")
        );
        assert_eq!(
            lane_event_line(
                &event(TranscriptKind::Player, "", "阿濤", "好啊"),
                "zh-TW",
                Side::Gm
            )
            .as_deref(),
            Some("阿濤：好啊")
        );
        assert_eq!(
            lane_event_line(
                &event(TranscriptKind::Narration, "", "GM", "夜深了"),
                "zh-TW",
                Side::Gm
            )
            .as_deref(),
            Some("（旁白）夜深了")
        );
        assert_eq!(
            lane_event_line(
                &event(TranscriptKind::System, "", "", "擲骰 3"),
                "zh-TW",
                Side::Gm
            )
            .as_deref(),
            Some("（系統）擲骰 3")
        );
    }

    fn marked(marker: data::EventMarker, text: &str, gm_only: bool) -> TranscriptEvent {
        TranscriptEvent {
            gm_only,
            marker: Some(marker),
            ..event(TranscriptKind::System, "", "GM", text)
        }
    }

    /// visibility 洩漏修正（包 4b）：gm_only 事件在 chars 線只留標頭；GM 線與非 gm_only 事件
    /// 一律全文；沒有玩家名的發言退回該語系稱呼。
    #[test]
    fn lane_event_line_redacts_gm_only_text_for_chars_lane_only() {
        let secret = marked(
            data::EventMarker::PersonArrival {
                title: "密探".to_owned(),
            },
            "只有 GM 知道的全文。",
            true,
        );
        assert_eq!(
            lane_event_line(&secret, "zh-TW", Side::Character).as_deref(),
            Some("（系統）（人物登場）〈密探〉")
        );
        assert_eq!(
            lane_event_line(&secret, "zh-TW", Side::Gm).as_deref(),
            Some("（系統）（人物登場）〈密探〉\n只有 GM 知道的全文。")
        );

        let public = event(TranscriptKind::System, "", "GM", "擲骰 3");
        assert_eq!(
            lane_event_line(&public, "zh-TW", Side::Character).as_deref(),
            Some("（系統）擲骰 3")
        );
        let nameless = event(TranscriptKind::Player, "", "", "你好");
        assert_eq!(
            lane_event_line(&nameless, "ko", Side::Character).as_deref(),
            Some("플레이어: 你好")
        );
    }

    /// 角色私設（系統事件）：世界書掃描一律先排除系統事件（方案二「掃描範圍」，同 ST coreChat），
    /// 兩個視角都不靠它觸發。換幕摘要只讀角色側內容，回歸標頭只出現一次。
    #[test]
    fn character_side_keyword_and_summary_ignore_card_private() {
        let fox = card("fox-id", "狐狸", "尾巴很大。", "身上藏著龍鱗。");
        let (marker, text) =
            card_private(&fox, &crate::transport::test_support::plain_fill("阿濤")).unwrap();
        let private = marked(marker, &text, true);
        let knight = card("knight-id", "騎士", "王國騎士", "");
        let (marker, text) =
            card_arrival(&knight, &crate::transport::test_support::plain_fill("阿濤"));
        let arrival = marked(marker, &text, false);
        let entries = [worldbook_entry(
            1,
            "龍鱗傳說",
            &["龍鱗"],
            false,
            0,
            false,
            Visibility::Public,
        )];
        let events = [
            event(TranscriptKind::Player, "", "阿濤", "聽說有寶箱"),
            event(TranscriptKind::Narration, "", "GM", "夜深了"),
            arrival,
            private,
            event(TranscriptKind::Narration, "", "GM", "風起"),
        ];
        let turn = chars_lane_turn(
            &fox,
            None,
            &events,
            &entries,
            &TableState::default(),
            &Mechanism::default(),
            None,
            "zh-TW",
            false,
        );
        assert!(!turn.tail.contains("龍鱗傳說"));
        let gm_turn = gm_lane_turn(
            &events,
            &entries,
            None,
            &TableState::default(),
            &Mechanism::default(),
            &StateScope::default(),
            "導演指示",
            "zh-TW",
        );
        assert!(!gm_turn.tail.contains("龍鱗傳說"));

        let joined: String = summary_messages(&events, "zh-TW", false)
            .iter()
            .map(|message| message.content.as_str())
            .collect();
        assert_eq!(
            joined
                .matches("（系統）（角色回歸）〈騎士〉\n公開設定：\n王國騎士")
                .count(),
            1
        );
        assert!(!joined.contains("龍鱗"));
        assert!(!joined.contains("角色私設"));
    }
}
