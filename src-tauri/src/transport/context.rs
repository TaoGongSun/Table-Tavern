use crate::data::{
    CharacterCard, FieldKind, Mechanism, NumericUpdate, TranscriptEvent, UpdateMode,
};
use crate::world_scan::{Placed, PromptTexts, WorldScan};

use super::gm_opening::{example_section, GmOpening};
use super::list_protocol::push_list_protocols;
use super::messages::{language_rule, scaffold_en};
use super::worldbook::{full_text_members, section_body};

/// 本輪 GM 提示裡真的看得到的一段世界書全文：標題＋實際送出的本文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptEntry<'a> {
    pub title: &'a str,
    pub content: &'a str,
}

/// GM 視角的觸發條目分成凍結 system 的兩段（穩定、前／後）與回合尾段（其餘）（方案三之 3 放置表）。
pub(super) struct GmWorldbook<'a> {
    pub system_front: Vec<&'a Placed>,
    pub system_back: Vec<&'a Placed>,
    pub tail: Vec<&'a Placed>,
}

pub(super) fn gm_worldbook(scan: &WorldScan) -> GmWorldbook<'_> {
    let mut split = GmWorldbook {
        system_front: Vec::new(),
        system_back: Vec::new(),
        tail: Vec::new(),
    };
    for entry in &scan.placed {
        match (entry.stable, entry.in_front(), entry.in_back()) {
            (true, true, _) => split.system_front.push(entry),
            (true, _, true) => split.system_back.push(entry),
            _ => split.tail.push(entry),
        }
    }
    split
}

/// 本輪 GM 提示裡「全文」真的進得去的世界書內容（API 單發與 CLI lane 同一套，掃描結果與實送共用）：
/// - 本輪掃描放進提示的條目：前／後組與作者註記、依深度的注入段落都算；outlet 條目只有在 `{{outlet}}` 真的
///   被代入時才算；只進名冊的 constant 人物條目不算；
/// - 人物登場事件：歷史裡帶的是登場當時的全文（`person_arrival`），取事件本文、不回頭讀目前的條目——
///   登場後再改條目、或同名不同內容的條目，都不能冒充已在提示裡。
///
/// 給「導演指示能不能點名格式條目」用，不得另寫近似判定。
pub fn gm_prompt_full_entries<'a>(
    scan: &'a WorldScan,
    events: &'a [TranscriptEvent],
) -> Vec<PromptEntry<'a>> {
    let split = gm_worldbook(scan);
    let mut entries: Vec<PromptEntry<'a>> = Vec::new();
    for group in [&split.system_front, &split.system_back, &split.tail] {
        entries.extend(
            full_text_members(group, &scan.outlets_used)
                .into_iter()
                .map(|entry| PromptEntry {
                    title: &entry.title,
                    content: &entry.content,
                }),
        );
    }
    entries.extend(events.iter().filter_map(|event| match &event.marker {
        Some(crate::data::EventMarker::PersonArrival { title }) => Some(PromptEntry {
            title,
            content: &event.text,
        }),
        _ => None,
    }));
    entries
}

/// GM 的 system prompt 本體：GM 指示＋world.md＋穩定的前組世界書＋全卡（含私設）＋玩家卡＋穩定的後組世界書。
/// assemble_gm_messages（單發）與 gm_lane_system（resume 續聊凍結快照）共用。文字都是 `prepare` 代換好的。
/// constant 人物條目改走名冊行，不進全文（包 4a）。`takeover`：介面接管桌（`data::is_interface_takeover`），
/// 開頭換成唯一敘事者版（名單上的角色也由 GM 演、不點名）。`opening` 的範例段接在登場角色之後（見 `GmOpening`）。
#[allow(clippy::too_many_arguments)]
pub(super) fn gm_system_prompt(
    texts: &PromptTexts,
    cards: &[CharacterCard],
    player: Option<&CharacterCard>,
    front: &[&Placed],
    back: &[&Placed],
    mechanism: &Mechanism,
    lang: &str,
    takeover: bool,
    opening: &GmOpening,
) -> String {
    let en = scaffold_en(lang);
    let intro = match (en, takeover) {
        // 介面接管桌只走 GM 一條敘事線（作者裁決 2026-10-27）：名單上的角色也由 GM 演，不點名
        (true, true) => {
            "You are the GM (director and narrator) of this tabletop RPG session, and its only narrator. You describe \
             scenes and how the world responds, drive the pacing of the story, and keep it from stalling or repeating. \
             Narration is public: everyone hears it. You play every character — supporting characters without a \
             character card and the characters on the \"Characters\" list alike — and may bring them in, have them \
             speak, and act freely; only the player plays themselves, so never speak or act for the player. Only you \
             know the full world setting and the characters' private profiles; do not reveal anything the story has \
             not uncovered yet."
        }
        (false, true) => {
            "你是這場桌上角色扮演的 GM（導演兼旁白），也是唯一的敘事者。你負責描述場景與世界反應、\
             推進劇情節奏，並防止劇情停滯或重複。\
             旁白是所有人都聽得到的公開敘事。所有角色——沒有角色卡的配角與「登場角色」名單上的角色——\
             都由你扮演，可自由出場、說話、行動；只有玩家由玩家自己扮演，不要替玩家說話或行動。\
             世界設定與角色私有設定只有你知道全貌，劇情尚未揭露的內容不要說破。"
        }
        (true, false) => {
            "You are the GM (director and narrator) of this multiplayer tabletop RPG session. You describe scenes and \
             how the world responds, drive the pacing of the story, decide who speaks next, and keep the conversation \
             from stalling or repeating. Narration is public: everyone hears it. You fully play supporting characters \
             who have no character card (passers-by, shopkeepers, villains, and so on) and may bring them in, have them \
             speak, and act freely; the characters on the \"Characters\" list and the player each have their own \
             players, so do not speak for them. Only you know the full world setting and the characters' private \
             profiles; do not reveal anything the story has not uncovered yet."
        }
        (false, false) => {
            "你是這場多人桌上角色扮演的 GM（導演兼旁白）。你負責描述場景與世界反應、\
             推進劇情節奏、決定下一位發言者，並防止對話停滯或重複。\
             旁白是所有人都聽得到的公開敘事。沒有角色卡的配角（路人、店主、反派等）\
             由你全權扮演，可自由出場、說話、行動；「登場角色」名單上的角色與玩家\
             各有扮演者，不要替他們代言。\
             世界設定與角色私有設定只有你知道全貌，劇情尚未揭露的內容不要說破。"
        }
    };
    let separator = if en { " " } else { "" };
    let mut system = format!("{intro}{separator}{}\n", language_rule(lang));
    if !texts.world_md.trim().is_empty() {
        let heading = match en {
            true => {
                "## World setting (only in your context; characters know only what you say aloud)"
            }
            false => "## 世界設定（只進你的上下文，角色只知道你說出口的內容）",
        };
        system.push_str(&format!("\n{heading}\n{}\n", texts.world_md.trim()));
    }
    let front = section_body(front, lang);
    if !front.is_empty() {
        system.push_str(worldbook_heading(lang));
        system.push_str(&front);
    }
    // 舞台卡（stage-card-in-speaker-list）：場景／世界設定，另起「舞台」段，不說它有扮演者
    let (stage, cast): (Vec<&CharacterCard>, Vec<&CharacterCard>) =
        cards.iter().partition(|card| card.stage);
    let (public_label, private_label) = match en {
        true => (
            "Public profile:",
            "Private profile (known only to you and that character):",
        ),
        false => ("公開設定：", "私有設定（僅你與該角色知道）："),
    };
    let stage_labels = match en {
        true => ("Public setting:", "Private setting (known only to you):"),
        false => ("公開設定：", "私有設定（只有你知道）："),
    };
    for (heading, group, (public_label, private_label)) in [
        (stage_heading(lang), &stage, stage_labels),
        (
            match en {
                true => "\n## Characters\n",
                false => "\n## 登場角色\n",
            },
            &cast,
            (public_label, private_label),
        ),
    ] {
        if group.is_empty() {
            continue;
        }
        system.push_str(heading);
        for card in group {
            let card_texts = texts.card(card);
            system.push_str(&format!("### {}\n", card.name));
            if !card_texts.public.trim().is_empty() {
                system.push_str(&format!("{public_label}\n{}\n", card_texts.public.trim()));
            }
            if !card_texts.private.trim().is_empty() {
                system.push_str(&format!("{private_label}\n{}\n", card_texts.private.trim()));
            }
        }
    }
    system.push_str(&example_section(opening, lang));
    if let Some(player) = player {
        system.push_str(&player_heading(&player.name, lang));
        if !texts.player.public.trim().is_empty() {
            system.push_str(&format!("\n{}\n", texts.player.public.trim()));
        }
    }
    let back = section_body(back, lang);
    if !back.is_empty() {
        system.push_str(match en {
            true => "\n## Worldbook, continued (only in your context)\n",
            false => "\n## 世界書（續）（只進你的上下文）\n",
        });
        system.push_str(&back);
    }
    if mechanism.incremental {
        system.push('\n');
        system.push_str(&mechanism_protocol(lang, mechanism.numeric_update));
        // 卡專屬規矩接在通用協定後面：這張卡哪些欄位每回合必報、哪些只在變動時報，
        // 由重構照卡原文產出，通用協定的「只寫變動的欄位」到這裡以卡的規定為準。
        // 介面歸屬聲明壓在最後——卡原文往往要求模型每回合重印整包狀態才畫得出畫面，
        // 這裡畫面由 App 組，不講清楚模型會照卡的老規矩再印一份；而且要壓在欄位說明
        // 之後，否則模型會照說明的 markdown 排版把狀態寫進正文（兩者實測都踩過）。
        if !mechanism.guide.trim().is_empty() {
            system.push_str("\n\n");
            system.push_str(mechanism.guide.trim());
            if let Some(example) = table_update_example(mechanism, lang) {
                system.push_str("\n\n");
                system.push_str(&example);
            }
            push_list_protocols(&mut system, mechanism, lang);
            system.push_str("\n\n");
            system.push_str(interface_owned_notice(lang));
        } else if mechanism.numeric_update == NumericUpdate::DeltaOnly {
            // 無介面的增量桌（重構過、或非 MVU 來源）：沒有範例 GM 只看得到通用協定的
            // /Heroes、/World，會照那個樣子自創路徑（DongeonMaster 重構桌實測踩過）。
            // 未重構 MVU 桌走 Upstream，提示詞逐字不變。
            if let Some(example) = table_update_example(mechanism, lang) {
                system.push_str("\n\n");
                system.push_str(&example);
            }
            // 退場桌開場補齊（statusbar-only-fields）：逐格說明追蹤什麼＋同回合更新提示；只讀套用時存下的資料
            if mechanism.opening.tracked {
                system.push_str("\n\n");
                system.push_str(&super::field_notes::section(mechanism, lang));
            }
            push_list_protocols(&mut system, mechanism, lang);
        }
    }
    system
}

/// 舞台卡段標（GM 與角色線共用），前後各帶換行：舞台卡提供場景與世界設定，不是角色、不被點名發言。
pub(super) fn stage_heading(lang: &str) -> &'static str {
    match scaffold_en(lang) {
        true => "\n## Stage (scene and world setting; not a character, never called on to speak)\n",
        false => "\n## 舞台（場景與世界設定，不是角色，不會被點名發言）\n",
    }
}

/// GM 側「世界書」段標（system 的 constant 條目與回合尾的 keyword 條目共用），前後各帶換行。
pub(super) fn worldbook_heading(lang: &str) -> &'static str {
    match scaffold_en(lang) {
        true => "\n## Worldbook (only in your context)\n",
        false => "\n## 世界書（只進你的上下文）\n",
    }
}

/// 玩家卡段標（GM 與 chars 兩條 system 共用），開頭帶換行、結尾不帶。
pub(super) fn player_heading(name: &str, lang: &str) -> String {
    match scaffold_en(lang) {
        true => format!(
            "\n## Player character (played by a real person; \"{name}\" in the transcript is them)"
        ),
        false => format!("\n## 玩家角色（真人扮演，逐字稿裡的「{name}」就是他）"),
    }
}

/// 增量桌統一協定聲明，依數字欄更新策略分版：DeltaOnly 就是原文；Upstream（沒重構的 MVU 卡）把講
/// 「只報變動量」「數字欄只收 delta」「上下限由系統把關」的句子換成照上游 MVU 收 replace、replace 不夾值的說法，
/// 其餘原文照舊。每桌策略固定，凍結快照照樣穩定。
fn mechanism_protocol(lang: &str, numeric: NumericUpdate) -> String {
    let base = delta_only_protocol(lang);
    if numeric == NumericUpdate::DeltaOnly {
        return base.to_owned();
    }
    let swaps: [(&str, &str); 3] = if scaffold_en(lang) {
        [
            (
                "Numbers are computed and tracked locally by the system — you only need to say \
                 \"how much changed this scene.\" End every reply with an update block:",
                "Numbers are tracked by the system — you only report which fields changed this \
                 scene: delta for the change amount, or replace to write the new value. End every \
                 reply with an update block:",
            ),
            (
                "- Number fields always use delta for the change amount (e.g. 5, -10), never an \
                 absolute value — absolute values will be rejected.\n\
                 - \"current/max\" fields (e.g. \"480/500\"): delta moves the current value; only use \
                 replace (e.g. \"480/600\") when the max changes (a level-up).",
                "- Number fields: use delta for the change amount (e.g. 5, -10), or replace with the new \
                 value (e.g. 85).\n\
                 - \"current/max\" fields (e.g. \"480/500\"): delta moves the current value; replace \
                 rewrites the whole value as \"current/max\" (e.g. \"450/600\").",
            ),
            (
                "- Bounds and rejections are enforced by the system; rejected fields will tell you the \
                 current value next turn.",
                "- replace stores exactly what you write — the system does not clamp it to any bounds, \
                 so write a sensible value. Updates to dice and read-only fields are rejected; rejected \
                 fields will tell you the current value next turn.",
            ),
        ]
    } else {
        [
            (
                "數值由系統本地計算與記帳，你只要說「這一幕變動了多少」。每次回覆的最後附上更新區塊：",
                "數值由系統記帳，你只要回報這一幕哪些欄位變了：用 delta 給增減量，或用 replace 直接寫新值。每次回覆的最後附上更新區塊：",
            ),
            (
                "- 數字欄一律用 delta 給增減量（例 5、-10），不要給絕對值——給絕對值會被系統擋下。\n\
                 - 「現值/上限」欄（例 \"480/500\"）：delta 動現值；只有上限改變（升級）才用 replace 寫成 \"480/600\"。",
                "- 數字欄可用 delta 給增減量（例 5、-10），也可以用 replace 直接寫新值（例 85）。\n\
                 - 「現值/上限」欄（例 \"480/500\"）：delta 動現值；用 replace 就整個寫成「現值/上限」（例 \"450/600\"）。",
            ),
            (
                "- 上下限與拒收由系統把關，被擋下的欄位會在下一輪告訴你目前值。",
                "- replace 寫什麼就存什麼，系統不會幫你夾在上下限內，請自己寫合理的值；骰值欄與唯讀欄的更新會被擋下，被擋下的欄位會在下一輪告訴你目前值。",
            ),
        ]
    };
    let mut text = base.to_owned();
    for (from, to) in swaps {
        debug_assert!(text.contains(from), "{from}");
        text = text.replace(from, to);
    }
    text
}

/// 增量桌統一協定聲明：數值由系統本地記帳，模型只回報這一幕的變動量。
/// 原文照貼進凍結快照，不要改寫措辭——改一字整條快取全滅。
fn delta_only_protocol(lang: &str) -> &'static str {
    if scaffold_en(lang) {
        "## State Update Protocol v1 (this table's numbers are system-managed)\n\n\
         Numbers are computed and tracked locally by the system — you only need to say \
         \"how much changed this scene.\" End every reply with an update block:\n\n\
         <UpdateVariable>\n\
         <JSONPatch>\n\
         [\n\
         \x20 { \"op\": \"delta\", \"path\": \"/Heroes/Arthur/Affection\", \"value\": 5 },\n\
         \x20 { \"op\": \"replace\", \"path\": \"/World/Location\", \"value\": \"Dawnport Docks\" }\n\
         ]\n\
         </JSONPatch>\n\
         </UpdateVariable>\n\n\
         - Paths are `/`-separated; only include fields that actually changed this scene.\n\
         - Number fields always use delta for the change amount (e.g. 5, -10), never an \
         absolute value — absolute values will be rejected.\n\
         - \"current/max\" fields (e.g. \"480/500\"): delta moves the current value; only use \
         replace (e.g. \"480/600\") when the max changes (a level-up).\n\
         - Text fields use replace with the new value; use insert to add, remove to delete, \
         move to relocate.\n\
         - Dice fields are rolled locally by the system each turn — you only read them, never \
         write them.\n\
         - Bounds and rejections are enforced by the system; rejected fields will tell you the \
         current value next turn."
    } else {
        "## 狀態更新協定 v1（這桌的數值由系統保管）\n\n\
         數值由系統本地計算與記帳，你只要說「這一幕變動了多少」。每次回覆的最後附上更新區塊：\n\n\
         <UpdateVariable>\n\
         <JSONPatch>\n\
         [\n\
         \x20 { \"op\": \"delta\", \"path\": \"/Heroes/亞瑟/Affection\", \"value\": 5 },\n\
         \x20 { \"op\": \"replace\", \"path\": \"/World/Location\", \"value\": \"晨港碼頭\" }\n\
         ]\n\
         </JSONPatch>\n\
         </UpdateVariable>\n\n\
         - path 用 `/` 分層，只寫這一幕真的變動的欄位，沒變的不要寫。\n\
         - 數字欄一律用 delta 給增減量（例 5、-10），不要給絕對值——給絕對值會被系統擋下。\n\
         - 「現值/上限」欄（例 \"480/500\"）：delta 動現值；只有上限改變（升級）才用 replace 寫成 \"480/600\"。\n\
         - 文字欄用 replace 寫新值；新增項目用 insert、刪除用 remove、搬移用 move。\n\
         - 骰值欄由系統每回合擲，你只讀不寫。\n\
         - 上下限與拒收由系統把關，被擋下的欄位會在下一輪告訴你目前值。"
    }
}

/// 本桌專屬的更新區塊範例：通用協定的範例路徑（/Heroes、/World）是示意，模型會照抄（實測 Haiku 踩過），
/// 這裡用本桌欄位規則的真實路徑示範：delta 取排序最前的數字類欄、replace 取排序最前的文字欄，
/// 含 `*` 的萬用路徑與清單欄不取（清單欄與集合的報法另由 `list_protocol::push_list_protocols` 附上），
/// 找不到就略過那一行。跟著 guide（介面接管桌）或無 guide 的 DeltaOnly 增量桌出現；
/// 規則只在套用時變，凍結快照照樣穩定。
fn table_update_example(mechanism: &Mechanism, lang: &str) -> Option<String> {
    let pointer = |path: &str| -> String {
        path.split('.')
            .map(|segment| format!("/{}", segment.replace('~', "~0").replace('/', "~1")))
            .collect()
    };
    let concrete = |path: &str| !path.split('.').any(|segment| segment == "*");
    let delta = mechanism
        .rules
        .iter()
        .find(|(path, rule)| {
            concrete(path)
                && rule.update == UpdateMode::Delta
                && matches!(
                    rule.kind,
                    FieldKind::Number | FieldKind::Pair | FieldKind::Counter
                )
        })
        .map(|(path, _)| {
            serde_json::json!({ "op": "delta", "path": pointer(path), "value": -1 }).to_string()
        });
    let placeholder = if scaffold_en(lang) {
        "(new value)"
    } else {
        "（新的值）"
    };
    let replace = mechanism
        .rules
        .iter()
        .find(|(path, rule)| {
            concrete(path) && rule.update == UpdateMode::Replace && rule.kind == FieldKind::Text
        })
        .map(|(path, _)| {
            serde_json::json!({ "op": "replace", "path": pointer(path), "value": placeholder })
                .to_string()
        });
    let lines: Vec<String> = [replace, delta]
        .into_iter()
        .flatten()
        .map(|line| format!("  {line}"))
        .collect();
    if lines.is_empty() {
        return None;
    }
    let heading = if scaffold_en(lang) {
        "This table's update block looks like this (paths follow this table's state tree; do not copy the /Heroes or /World paths from the generic protocol example above):"
    } else {
        "本桌的更新區塊長這樣（路徑照本桌狀態樹；不要照抄上面通用協定範例裡的 /Heroes、/World 路徑）："
    };
    Some(format!(
        "{heading}\n<UpdateVariable>\n<JSONPatch>\n[\n{}\n]\n</JSONPatch>\n</UpdateVariable>",
        lines.join(",\n")
    ))
}

/// 介面歸屬聲明：只有介面被 App 接管的桌（mechanism.guide 非空）才附，而且排在欄位說明之後——
/// 模型會模仿最後讀到的排版，欄位說明擺最後它就照那份說明的 markdown 逐條寫在正文裡（實測踩過）。
/// 與 mechanism_protocol 一樣是凍結快照的一部分，措辭不要隨手改。
fn interface_owned_notice(lang: &str) -> &'static str {
    if scaffold_en(lang) {
        "## Who draws the interface\n\n\
         The field spec above is a specification of what each value looks like — its layout is not \
         your output format.\n\
         This table's game interface (status panels, maps, lists) is rendered by the app from the \
         state tree it keeps. If the card's own rules tell you to reprint a whole interface block \
         every turn (a multi-module `<...UI>` dump), do not — the app draws that itself.\n\
         Your reply is exactly two things: this turn's story, then the `<UpdateVariable>` update \
         block. That block is bookkeeping for the system, not interface output — every state value \
         goes inside it as JSONPatch. Never report state as prose, lists, or bold headings in the \
         story body."
    } else {
        "## 介面由誰畫\n\n\
         上面的欄位說明是規格書，講的是每個欄位的值長什麼樣；**它的排版不是你的輸出格式**。\n\
         這桌的遊戲介面（狀態面板、地圖、清單）由 App 拿系統帳上的狀態樹組裝並渲染。卡原文若要求\
         你每回合重印整包介面（`<...UI>` 那種多模块資料塊），一律不要照做——那些 App 自己會畫。\n\
         你的回覆就兩樣東西：這一回合的劇情正文，然後是 `<UpdateVariable>` 更新區塊。那個區塊不是\
         介面輸出、是給系統記帳用的，**每一個狀態值都寫在裡面**（照上面協定的 JSONPatch 寫法）。\
         不要改用條列、粗體標題或任何其他形式把狀態寫在劇情正文裡。"
    }
}

#[cfg(test)]
mod tests;
