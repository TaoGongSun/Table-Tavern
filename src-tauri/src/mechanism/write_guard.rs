//! 合法值與受保護根（vn-mode 包 4 §2.3）：模型的 `<UpdateVariable>`、面板手改、觸發表一次性旗標三個寫入入口
//! 共用這一道檢查。受保護根底下只收表內欄位逐葉更新——模型整支刪掉後補種會把進度重置成初值。
use crate::data::Mechanism;

use super::rules::rule_for;

/// 一筆寫入的形狀：取代成某值、增減，或其他（新增表外鍵、刪除、搬移、整支替換）。
#[derive(Debug, Clone, Copy)]
pub enum Write<'a> {
    Replace(&'a str),
    Delta,
    Other,
}

/// 這筆寫入不合法時回要回饋的拒收句；合法回 None。
pub fn write_refusal(mechanism: &Mechanism, path: &[String], write: Write<'_>) -> Option<String> {
    let shown = path.join(".");
    let root = path.first()?;
    if mechanism.locked_roots.contains(root) {
        if !mechanism.rules.contains_key(&shown) {
            return Some(format!(
                "{shown} 不是表內欄位；{root} 底下只能逐項更新表內已有的欄位，不能新增、整支替換或刪除。"
            ));
        }
        if matches!(write, Write::Other) {
            return Some(format!("{shown} 不能刪除或搬移，只能更新它的值。"));
        }
    }
    let allowed = rule_for(mechanism, path, None).allowed;
    if allowed.is_empty() {
        return None;
    }
    let choices = allowed.join("、");
    match write {
        Write::Replace(value) if allowed.iter().any(|item| item == value) => None,
        Write::Replace(value) => Some(format!(
            "{shown} 的值「{value}」不在可用值內。可用值：{choices}。"
        )),
        Write::Delta | Write::Other => Some(format!(
            "{shown} 只能用 replace 寫成可用值之一。可用值：{choices}。"
        )),
    }
}

/// JSON 值換成比對用的文字（布林與數字照字面）；物件、陣列、null 回 None（必不在清單內）。
pub(super) fn value_text(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Bool(flag) => Some(flag.to_string()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// ```state 圍欄同一道檢查（圍欄的巢狀欄位直接寫樹、清單整支替換，不經 `apply_one`）：拒收的容器與欄位
/// 落地前拿掉、保留原值，記拒收回饋。受保護根底下的唯讀欄（`chapter`）一併擋。
pub(super) fn guard_fence(
    mechanism: &Mechanism,
    aligned: &mut super::block_paths::Aligned,
) -> Vec<super::types::Record> {
    use super::types::{Record, RecordKind};
    let refuse = |path: &[String], write: Write<'_>| {
        let readonly = path
            .first()
            .is_some_and(|root| mechanism.locked_roots.contains(root))
            && rule_for(mechanism, path, None).update == crate::data::UpdateMode::Reject;
        write_refusal(mechanism, path, write)
            .or_else(|| readonly.then(|| format!("{} 是唯讀欄位，不接受更新。", path.join("."))))
    };
    let mut records = Vec::new();
    let mut reject = |path: &[String], detail: String| {
        records.push(Record::new(RecordKind::Rejected, path.join("."), detail));
    };
    let block = &mut aligned.block;
    for containers in [&mut block.lists, &mut block.maps] {
        containers.retain(|path| match refuse(path, Write::Other) {
            Some(detail) => {
                reject(path, detail);
                false
            }
            None => true,
        });
    }
    let fields = std::mem::take(&mut block.fields);
    let targets = std::mem::take(&mut aligned.targets);
    for (field, target) in fields.into_iter().zip(targets) {
        match refuse(&field.0, Write::Replace(&field.1)) {
            Some(detail) => reject(&field.0, detail),
            None => {
                block.fields.push(field);
                aligned.targets.push(target);
            }
        }
    }
    records
}

// ---------------------------------------------------------------------
// 預設狀態欄（table-status-panel §2.3）：`characters`／`tracking` 底下只收有規則的欄位。只管模型的兩個寫入
// 入口（補丁、圍欄）；面板手改走 `write_refusal`，玩家怎麼改都行。開格是 app 的事，萬用規則
// `characters.*.status` 擋不住新格，所以格存不存在另查 `panel_cells`。
// ---------------------------------------------------------------------

use crate::data::default_panel::{CHARACTERS, TRACKING};
use crate::data::{FieldKind, FieldRule};

fn panel_root(path: &[String]) -> bool {
    path.first()
        .is_some_and(|root| root == CHARACTERS || root == TRACKING)
}

fn cell_exists(mechanism: &Mechanism, key: &str) -> bool {
    mechanism.panel_cells.values().any(|cell| cell == key)
}

/// 清單欄逐項寫的拒收句（`parent`＝清單欄本身）。
fn list_item_refusal(parent: &[String]) -> String {
    let shown = super::note::shown_pointer(parent);
    format!("{shown} 是清單欄，請用 replace 在 {shown} 寫整串字串陣列（加一項也重寫整串、沒變的照抄），不要逐項寫或用 insert、remove。")
}

/// 預設桌這條路徑能不能寫：不在預設桌或不在兩支底下回 None（照一般流程）；可寫回這欄的規則，否則回拒收句。
pub(super) fn panel_target(
    mechanism: &Mechanism,
    path: &[String],
) -> Option<Result<FieldRule, String>> {
    if !mechanism.default_panel || !panel_root(path) {
        return None;
    }
    let shown = super::note::shown_pointer(path);
    let refusal = match path {
        [_] => format!("{shown} 是整組狀態欄，不能整支替換、刪除或搬移；只能更新底下已有的欄位。"),
        [root, key, ..] if root == CHARACTERS && !cell_exists(mechanism, key) => format!(
            "{} 底下的角色格由系統建立，沒有「{key}」這一格；只能寫已有角色格的欄位。",
            super::note::shown_pointer(&path[..1])
        ),
        [root, _] if root == CHARACTERS => format!(
            "{shown} 是整個角色格，不能整支替換、刪除或搬移；只能更新底下的欄位（例 {shown}/status）。"
        ),
        _ if super::rules::has_rule(mechanism, path) => {
            return Some(Ok(super::rules::rule_for(mechanism, path, None)));
        }
        [parent @ .., last]
            if (last == "-" || (!last.is_empty() && last.bytes().all(|b| b.is_ascii_digit())))
                && super::rules::has_rule(mechanism, parent)
                && super::rules::rule_for(mechanism, parent, None).kind == FieldKind::List =>
        {
            list_item_refusal(parent)
        }
        _ => format!(
            "{shown} 不是狀態欄的欄位；{} 底下只能更新已有的欄位，不能新增。",
            super::note::shown_pointer(&path[..1])
        ),
    };
    Some(Err(refusal))
}

/// 補丁入口：預設桌 `characters`／`tracking` 底下只收 replace（insert 打到有規則的欄位照 replace 算），
/// 刪除、搬移、清單 delta 一律拒收。通過回這欄的規則（交給預算夾值），不歸這裡管回 None。
pub(super) fn panel_patch(
    mechanism: &Mechanism,
    patch: &super::types::Patch,
) -> Option<Result<FieldRule, super::types::Record>> {
    use super::types::{PatchOp, Record, RecordKind};
    let reject =
        |path: &[String], detail: String| Record::new(RecordKind::Rejected, path.join("."), detail);
    if patch.op == PatchOp::Move && panel_target(mechanism, &patch.from).is_some() {
        let detail = format!(
            "{} 不能搬移，只能用 replace 寫新值。",
            super::note::shown_pointer(&patch.from)
        );
        return Some(Err(reject(&patch.from, detail)));
    }
    let rule = match panel_target(mechanism, &patch.path)? {
        Ok(rule) => rule,
        Err(detail) => return Some(Err(reject(&patch.path, detail))),
    };
    let shown = super::note::shown_pointer(&patch.path);
    match patch.op {
        PatchOp::Replace | PatchOp::Insert => Some(Ok(rule)),
        PatchOp::Delta if rule.kind == FieldKind::List => {
            Some(Err(reject(&patch.path, list_item_refusal(&patch.path))))
        }
        // 文字欄 delta 交給既有的拒收句
        PatchOp::Delta => None,
        PatchOp::Remove | PatchOp::Move => Some(Err(reject(
            &patch.path,
            format!("{shown} 不能刪除或搬移，只能用 replace 寫新值（清單欄寫整串陣列）。"),
        ))),
    }
}

/// 圍欄入口：清單容器只收有 list 規則的欄位；具名容器只收兩支本身與既有角色格（其餘連同底下欄位拿掉）；
/// 不在清單容器底下的欄位只收有文字規則的路徑。清單容器底下的欄位交給預算驗形狀。
pub(super) fn panel_fence(
    mechanism: &Mechanism,
    aligned: &mut super::block_paths::Aligned,
) -> Vec<super::types::Record> {
    use super::types::{Record, RecordKind};
    let mut records = Vec::new();
    let mut dropped: Vec<Vec<String>> = Vec::new();
    let block = &mut aligned.block;
    // 清單形狀先驗（拿掉任何子容器之前）：任一項是物件、巢狀清單或空容器（解析器把 `- {}` 記成子容器）就整欄
    // 拒收、保留舊值——不能先剝掉壞項再把剩下的當整串寫回去，會蓋掉舊資料
    let malformed: Vec<Vec<String>> = block
        .lists
        .iter()
        .filter(|list| matches!(panel_target(mechanism, list), Some(Ok(rule)) if rule.kind == FieldKind::List))
        .filter(|list| {
            let under = |path: &Vec<String>| path.len() > list.len() && path.starts_with(list);
            block.gapped.contains(list)
                || block.lists.iter().chain(&block.maps).any(under)
                || block
                    .fields
                    .iter()
                    .any(|(path, _)| under(path) && path.len() != list.len() + 1)
        })
        .cloned()
        .collect();
    for list in &malformed {
        records.push(super::budget::list_shape(list));
        dropped.push(list.clone());
    }
    let inside = |path: &Vec<String>| {
        malformed
            .iter()
            .any(|list| path.len() >= list.len() && path.starts_with(list))
    };
    block.lists.retain(|path| !inside(path));
    block.maps.retain(|path| !inside(path));
    block.lists.retain(|path| {
        let refusal = match panel_target(mechanism, path) {
            None => return true,
            Some(Ok(rule)) if rule.kind == FieldKind::List => return true,
            Some(Ok(_)) => format!(
                "{} 是文字欄，不是清單；請寫一句文字。",
                super::note::shown_pointer(path)
            ),
            Some(Err(detail)) => detail,
        };
        records.push(Record::new(RecordKind::Rejected, path.join("."), refusal));
        dropped.push(path.clone());
        false
    });
    block.maps.retain(|path| {
        let container = match path.as_slice() {
            [root] => root == CHARACTERS || root == TRACKING,
            [root, key] => root == CHARACTERS && cell_exists(mechanism, key),
            _ => false,
        };
        let refusal = match panel_target(mechanism, path) {
            None => return true,
            _ if container => return true,
            Some(Ok(rule)) if rule.kind == FieldKind::List => list_item_refusal(path),
            Some(Ok(_)) => format!(
                "{} 是文字欄，不是一組欄位；請寫一句文字。",
                super::note::shown_pointer(path)
            ),
            Some(Err(detail)) => detail,
        };
        records.push(Record::new(RecordKind::Rejected, path.join("."), refusal));
        dropped.push(path.clone());
        false
    });
    let lists = block.lists.clone();
    let fields = std::mem::take(&mut block.fields);
    let targets = std::mem::take(&mut aligned.targets);
    for (field, target) in fields.into_iter().zip(targets) {
        let path = &field.0;
        if dropped.iter().any(|bad| path.starts_with(bad)) {
            continue;
        }
        let in_list = lists
            .iter()
            .any(|list| list.len() < path.len() && path.starts_with(list));
        let refusal = match panel_target(mechanism, path) {
            _ if in_list => None,
            None => None,
            Some(Ok(rule)) if rule.kind == FieldKind::List => Some(list_item_refusal(path)),
            Some(Ok(_)) => None,
            Some(Err(detail)) => Some(detail),
        };
        match refusal {
            Some(detail) => records.push(Record::new(RecordKind::Rejected, path.join("."), detail)),
            None => {
                block.fields.push(field);
                aligned.targets.push(target);
            }
        }
    }
    records
}
