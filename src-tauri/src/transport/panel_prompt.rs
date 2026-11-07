//! 預設狀態欄的 GM 指引與本桌範例（table-status-panel §2.4）：只在 `mechanism.default_panel` 桌附，接在通用協定後面、
//! 進凍結快照。指引是靜態文字，措辭定稿後不改（改一字整條快取全滅）；範例只讀規則與角色格對照，格變動才變。
//! 上限數字與 `data::default_panel` 的常數同一份。
use crate::data::default_panel::{
    CHARACTERS, ITEMS_MAX, ITEM_CHARS, STATUS_CHARS, TRACKING, TRACKING_CHARS, TRACKING_MAX,
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
             not change. Anything over a limit is cut off by the system."
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
             - 角色格由系統建立，不要新增角色名或其他鍵；沒變的欄位不要寫。超過上限的部分系統會直接截掉。"
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
