//! 預設狀態欄的 GM 指引與本桌範例（table-status-panel §2.4）：只在 `mechanism.default_panel` 桌附，接在通用協定後面、
//! 進凍結快照。指引是靜態文字，措辭定稿後不改（改一字整條快取全滅）；範例只讀規則與角色格對照，格變動才變。
//! 上限數字與 `data::default_panel` 的常數同一份。
use crate::data::default_panel::{
    CHARACTERS, CUSTOM_CHARS, ITEMS_MAX, ITEM_CHARS, STATUS_CHARS, TRACKING, TRACKING_CHARS,
    TRACKING_MAX,
};
use crate::data::Mechanism;

use super::messages::scaffold_en;

/// 預設狀態欄指引：三組欄位是什麼、status 是現況不是日誌、清單一律整串 replace、各欄上限、沒變不寫。
pub(super) fn guide(lang: &str) -> String {
    if scaffold_en(lang) {
        format!(
            "## This table's status panel (kept by the system)\n\n\
             The state tree has two groups of fields. Paths always use these English keys (the current state shows them \
             as \"Display name (key)\"):\n\
             - `/characters/<name>/status` (Status): that character's situation right now, in one sentence. Each replace \
             overwrites the whole sentence — it is a snapshot, not a log. At most {STATUS_CHARS} characters.\n\
             - `/characters/<name>/items` (Items): only the items that matter. At most {ITEMS_MAX} items, each at most \
             {ITEM_CHARS} characters.\n\
             - `/tracking/goals` (Goals), `/tracking/promises` (Promises), `/tracking/clues` (Clues): what is still in \
             play. At most {TRACKING_MAX} items each, each at most {TRACKING_CHARS} characters; once something is \
             resolved, leave it out.\n\
             - List fields always take replace with the whole array of strings: to add one item, rewrite the whole \
             array (copy the unchanged items). Never use insert, remove, or per-item paths like `/0` or `/-`.\n\
             - Character entries are created by the system: do not add new names or other keys. Skip fields that did \
             not change. Anything over a limit is cut off by the system.\n\
             - Fields the player added (keys other than the ones above, already in the state tree) are yours to keep up \
             too: if one is empty, fill it from the story; when it changes, replace the whole value. At most \
             {CUSTOM_CHARS} characters."
        )
    } else {
        format!(
            "## 本桌狀態欄（由系統保管）\n\n\
             狀態樹有兩組欄位，path 一律用這些英文鍵（目前狀態裡顯示成「中文名（英文鍵）」）：\n\
             - `/characters/<角色名>/status`（狀態）：這個角色此刻的處境，一句話。每次 replace 整句取代，是現況快照、不是日誌；\
             最多 {STATUS_CHARS} 字。\n\
             - `/characters/<角色名>/items`（持有）：只列重要的持有物，最多 {ITEMS_MAX} 項、每項最多 {ITEM_CHARS} 字。\n\
             - `/tracking/goals`（目標）、`/tracking/promises`（承諾）、`/tracking/clues`（線索）：還在進行中的事，各最多 \
             {TRACKING_MAX} 項、每項最多 {TRACKING_CHARS} 字；解決了就不再寫進去。\n\
             - 清單欄一律用 replace 寫整串字串陣列：加一項也重寫整串（沒變的項照抄），不要用 insert、remove 或 `/0`、`/-` 這類逐項路徑。\n\
             - 角色格由系統建立，不要新增角色名或其他鍵；沒變的欄位不要寫。超過上限的部分系統會直接截掉。\n\
             - 玩家自己加的欄位（上面沒列的鍵，已在狀態樹裡）也由你維護：空著就依劇情填上，之後有變就 replace 整句；最多 \
             {CUSTOM_CHARS} 字。"
        )
    }
}

/// 本桌更新區塊的固定範例：`replace /characters/<第一格>/status`（格鍵排序第一；沒有格就略過這行）與
/// `replace /tracking/clues` 整串陣列。
pub(super) fn example(mechanism: &Mechanism, lang: &str) -> String {
    let en = scaffold_en(lang);
    let pointer = |segments: &[&str]| -> String {
        segments
            .iter()
            .map(|segment| format!("/{}", segment.replace('~', "~0").replace('/', "~1")))
            .collect()
    };
    let (status, clues) = if en {
        ("(new value)", ["(clue one)", "(clue two)"])
    } else {
        ("（新的值）", ["（線索一）", "（線索二）"])
    };
    let first = mechanism.panel_cells.values().min();
    let lines: Vec<String> = first
        .map(|cell| {
            serde_json::json!({
                "op": "replace",
                "path": pointer(&[CHARACTERS, cell, "status"]),
                "value": status,
            })
        })
        .into_iter()
        .chain(std::iter::once(serde_json::json!({
            "op": "replace",
            "path": pointer(&[TRACKING, "clues"]),
            "value": clues,
        })))
        .map(|line| format!("  {line}"))
        .collect();
    let heading = if en {
        "This table's update block looks like this (paths follow this table's state tree; do not copy the /Heroes or /World paths from the generic protocol example above):"
    } else {
        "本桌的更新區塊長這樣（路徑照本桌狀態樹；不要照抄上面通用協定範例裡的 /Heroes、/World 路徑）："
    };
    format!(
        "{heading}\n<UpdateVariable>\n<JSONPatch>\n[\n{}\n]\n</JSONPatch>\n</UpdateVariable>",
        lines.join(",\n")
    )
}

/// 戰鬥欄指引（ttrpg-combat-tracker §2.9）：只在 `combat_panel` 桌接在 `guide` 之後，靜態文字、進凍結快照。
pub(super) fn combat_guide(lang: &str) -> String {
    use crate::data::default_panel::combat::{
        CONDITIONS_MAX, CONDITION_CHARS, FOES_MAX, HP_MAX, NAME_CHARS, ORDER_MAX,
    };
    if scaffold_en(lang) {
        format!(
            "## Combat fields (arithmetic done by the system)\n\n\
             - `/characters/<name>/hp` and `/combat/foes/<foe>/hp` (HP): \"current/max\" as two whole numbers, max at most \
             {HP_MAX}. The first time, replace it with the full value (e.g. \"24/24\"); if a character's HP is still empty, pick a \
             fitting value from their description and write it yourself instead of asking the player. After that, always report the change \
             with delta (damage negative, healing positive); the system adds it up and keeps it between 0 and max. Never \
             replace HP with a result. To change only the max, replace with the current value copied and the new max.\n\
             - When the amount comes from this turn's dice pool, add `\"dice\"` with the dice references and any constant, \
             e.g. {{\"op\":\"delta\",\"path\":\"/combat/foes/Goblin A/hp\",\"value\":-7,\"dice\":\"d8#1+3\"}}. The system \
             takes the faces from the pool; if your number differs, the dice total is applied. Each die is used once. Report \
             one delta per hit; never merge several hits into one. If the damage is halved or otherwise adjusted, leave \
             out `dice`.\n\
             - `/characters/<name>/conditions`, `/combat/foes/<foe>/conditions` (Conditions): whole array of strings, at \
             most {CONDITIONS_MAX} items of {CONDITION_CHARS} characters; remove ended effects yourself.\n\
             - When a fight starts: replace `/combat/order` with everyone in turn order (decide the order the way this table's rulebook says; only when it says nothing, \
             roll a d20 each, highest first. Any roll comes from the pool, citing dice in the order rolled, not the sorted order; at most {ORDER_MAX} names of {NAME_CHARS} characters), and add each foe without a character entry with \
             insert `/combat/foes/<foe>` value {{\"hp\":\"7/7\"}} (at most {FOES_MAX}). Give foes of the same kind \
             different names (Goblin A, Goblin B); an existing name cannot be inserted again. Remove a foe that falls or \
             flees with remove.\n\
             - Each reply, replace `/combat/turn` with the name in the order whose turn it is now.\n\
             - When the fight ends, replace `/combat/order` with []; the system clears the turn and the foes."
        )
    } else {
        format!(
            "## 戰鬥欄（數字由系統算）\n\n\
             - `/characters/<角色名>/hp`、`/combat/foes/<敵人>/hp`（HP）：「現值/上限」兩個整數，上限最多 {HP_MAX}。第一次用 \
             replace 寫完整值（例 \"24/24\"）；角色的 HP 還是空的就依角色設定自己定一個合理值寫入，不要停下來要玩家補；之後一律用 delta 報增減量（扣血負數、補血正數），系統加總並夾在 0 到上限之間。\
             不要用 replace 報結果；只改上限時現值照抄、寫新上限。\n\
             - 增減量來自本輪骰池時加 `\"dice\"` 寫骰號與常數，例 {{\"op\":\"delta\",\"path\":\"/combat/foes/哥布林A/hp\",\"value\":-7,\"dice\":\"d8#1+3\"}}。\
             骰面由系統從池裡取，數字對不上時以骰面總和為準；每顆骰只能用一次。每次命中各報一筆 delta，不要把幾次命中併成一筆。傷害有減半等調整時不要帶 dice。\n\
             - `/characters/<角色名>/conditions`、`/combat/foes/<敵人>/conditions`（狀態效果）：整串字串陣列，最多 \
             {CONDITIONS_MAX} 項、每項 {CONDITION_CHARS} 字；效果結束自己拿掉。\n\
             - 開打時：用 replace 把 `/combat/order` 寫成照行動順序排好的全部名字（順序怎麼定照本桌規則書；規則書沒寫才各擲 d20 比大小。有擲骰就從骰池取，旁白照擲的順序標骰號，不要照排好的順序；最多 {ORDER_MAX} 個、每個 \
             {NAME_CHARS} 字），沒有角色格的敵人逐一 insert `/combat/foes/<敵人>`，值 {{\"hp\":\"7/7\"}}（最多 {FOES_MAX} 筆）。\
             同種敵人要取不同名字（哥布林A、哥布林B），已有的名字不能再 insert；倒下或逃走的敵人用 remove。\n\
             - 每則回覆用 replace 把 `/combat/turn` 寫成順序裡現在輪到的人。\n\
             - 戰鬥結束時把 `/combat/order` replace 成 []，系統會清掉輪到誰與敵人。"
        )
    }
}

/// 戰鬥欄的固定範例：第一格角色的 HP 增量帶骰號（沒有格就略過）。
pub(super) fn combat_example(mechanism: &Mechanism, lang: &str) -> Option<String> {
    let cell = mechanism.panel_cells.values().min()?;
    let line = serde_json::json!({
        "op": "delta",
        "path": format!("/{CHARACTERS}/{}/hp", cell.replace('~', "~0").replace('/', "~1")),
        "value": -7,
        "dice": "d8#1+3",
    });
    let heading = if scaffold_en(lang) {
        "Damage with a dice reference looks like this:"
    } else {
        "帶骰號的扣血長這樣："
    };
    Some(format!(
        "{heading}\n<UpdateVariable>\n<JSONPatch>\n[\n  {line}\n]\n</JSONPatch>\n</UpdateVariable>"
    ))
}
