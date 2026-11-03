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
