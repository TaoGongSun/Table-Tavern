//! 假 dialog plugin：測試包以同名 `dialog` 取代 tauri-plugin-dialog，前端照原樣呼叫
//! `plugin:dialog|message/save/open`。每次呼叫進待答佇列並掛起，直到主線經控制埠回答。
//! 參數與回傳形狀照 tauri-plugin-dialog 2.7 的 IPC 契約（型別直接沿用該 crate）。

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::Value;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;
use tauri_plugin_dialog::{MessageDialogButtons, MessageDialogResult};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PendingInfo {
    pub id: String,
    /// message / save / open
    pub kind: &'static str,
    pub title: Option<String>,
    pub message: Option<String>,
    /// message 的按鈕標籤（依顯示順序）；save/open 為空。
    pub buttons: Vec<String>,
    /// message 的 kind（info/warning/error）或 save/open 的 options 原文。
    pub detail: Value,
}

struct Pending {
    info: PendingInfo,
    sender: oneshot::Sender<String>,
}

static QUEUE: Mutex<VecDeque<Pending>> = Mutex::new(VecDeque::new());

fn queue() -> std::sync::MutexGuard<'static, VecDeque<Pending>> {
    QUEUE.lock().unwrap_or_else(|p| p.into_inner())
}

pub(super) fn pending_count() -> usize {
    queue().len()
}

pub(super) fn list() -> Vec<PendingInfo> {
    queue().iter().map(|p| p.info.clone()).collect()
}

/// 回答一個對話窗。`id` 可為 `next`（最早的那個）。`choice` 的意義依 kind：
/// message＝按鈕標籤或 ok/yes/no/cancel；save＝路徑或 cancel；open＝路徑、JSON 陣列或 cancel。
pub(super) fn answer(id: &str, choice: &str) -> Result<PendingInfo, String> {
    let pending = {
        let mut q = queue();
        let index = if id == "next" {
            if q.is_empty() {
                return Err("沒有掛起中的對話窗".to_owned());
            }
            0
        } else {
            q.iter()
                .position(|p| p.info.id == id)
                .ok_or_else(|| format!("找不到對話窗 {id}（可能已回答過）"))?
        };
        if q[index].info.kind == "message" {
            resolve_button(&q[index].info.buttons, choice)?;
        }
        q.remove(index).expect("index 有效")
    };
    let info = pending.info.clone();
    pending
        .sender
        .send(choice.to_owned())
        .map_err(|_| "對話窗呼叫端已不在".to_owned())?;
    Ok(info)
}

/// 全部回答取消（quit 用）。
pub(super) fn cancel_all() {
    let drained: Vec<Pending> = queue().drain(..).collect();
    for pending in drained {
        let _ = pending.sender.send("cancel".to_owned());
    }
}

async fn wait(info: PendingInfo) -> Result<String, String> {
    let (sender, receiver) = oneshot::channel();
    queue().push_back(Pending { info, sender });
    receiver.await.map_err(|_| "對話窗被放棄".to_owned())
}

/// (顯示標籤, 回傳值)。預設按鈕的回傳值是 enum（序列化為 "Ok" 等），自訂按鈕回傳標籤本身。
fn button_set(buttons: Option<&MessageDialogButtons>) -> Vec<(String, MessageDialogResult)> {
    use MessageDialogResult as R;
    let std_btn = |r: R| (format!("{r:?}"), r);
    let custom = |s: &String| (s.clone(), R::Custom(s.clone()));
    match buttons {
        None | Some(MessageDialogButtons::Ok) => vec![std_btn(R::Ok)],
        Some(MessageDialogButtons::OkCancel) => vec![std_btn(R::Ok), std_btn(R::Cancel)],
        Some(MessageDialogButtons::YesNo) => vec![std_btn(R::Yes), std_btn(R::No)],
        Some(MessageDialogButtons::YesNoCancel) => {
            vec![std_btn(R::Yes), std_btn(R::No), std_btn(R::Cancel)]
        }
        Some(MessageDialogButtons::OkCustom(ok)) => vec![custom(ok)],
        Some(MessageDialogButtons::OkCancelCustom(ok, cancel)) => vec![custom(ok), custom(cancel)],
        Some(MessageDialogButtons::YesNoCancelCustom(yes, no, cancel)) => {
            vec![custom(yes), custom(no), custom(cancel)]
        }
        Some(_) => vec![std_btn(R::Ok)],
    }
}

/// 把回答對應到按鈕索引：先比完整標籤，再認 ok/yes＝第一顆、no＝第二顆、cancel＝最後一顆。
fn resolve_button(labels: &[String], choice: &str) -> Result<usize, String> {
    if let Some(i) = labels.iter().position(|l| l == choice) {
        return Ok(i);
    }
    let last = labels.len().saturating_sub(1);
    match choice.to_ascii_lowercase().as_str() {
        "ok" | "yes" => Ok(0),
        "no" if labels.len() >= 2 => Ok(1),
        "cancel" => Ok(last),
        _ => Err(format!("{choice:?} 不是這個對話窗的按鈕：{labels:?}")),
    }
}

#[tauri::command]
async fn message(
    title: Option<String>,
    message: String,
    kind: Option<Value>,
    buttons: Option<MessageDialogButtons>,
) -> Result<MessageDialogResult, String> {
    let set = button_set(buttons.as_ref());
    let labels: Vec<String> = set.iter().map(|(l, _)| l.clone()).collect();
    let info = PendingInfo {
        id: super::random_hex(4),
        kind: "message",
        title,
        message: Some(message),
        buttons: labels.clone(),
        detail: kind.unwrap_or(Value::Null),
    };
    let choice = wait(info).await?;
    let index = resolve_button(&labels, &choice).unwrap_or(labels.len().saturating_sub(1));
    Ok(set[index].1.clone())
}

#[tauri::command]
async fn save(options: Value) -> Result<Option<PathBuf>, String> {
    let info = PendingInfo {
        id: super::random_hex(4),
        kind: "save",
        title: options
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned),
        message: None,
        buttons: Vec::new(),
        detail: options,
    };
    let choice = wait(info).await?;
    Ok(if choice == "cancel" {
        None
    } else {
        Some(PathBuf::from(choice))
    })
}

#[tauri::command]
async fn open(options: Value) -> Result<Value, String> {
    let multiple = options
        .get("multiple")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let info = PendingInfo {
        id: super::random_hex(4),
        kind: "open",
        title: options
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned),
        message: None,
        buttons: Vec::new(),
        detail: options,
    };
    let choice = wait(info).await?;
    Ok(open_response(multiple, &choice))
}

/// 單選回字串或 null；multiple 回陣列或 null。回答可給單一路徑或 JSON 陣列。
fn open_response(multiple: bool, choice: &str) -> Value {
    if choice == "cancel" {
        return Value::Null;
    }
    let paths: Vec<String> =
        serde_json::from_str::<Vec<String>>(choice).unwrap_or_else(|_| vec![choice.to_owned()]);
    if multiple {
        Value::from(paths)
    } else {
        paths
            .into_iter()
            .next()
            .map(Value::from)
            .unwrap_or(Value::Null)
    }
}

pub(crate) fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("dialog")
        .invoke_handler(tauri::generate_handler![message, save, open])
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(result: &MessageDialogResult) -> String {
        serde_json::to_value(result)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn default_buttons_serialize_like_the_real_plugin() {
        // plugin-dialog JS 的 confirm() 預設比對 'Ok'，ask() 比對 'Yes'。
        let set = button_set(Some(&MessageDialogButtons::OkCancel));
        assert_eq!(wire(&set[0].1), "Ok");
        assert_eq!(wire(&set[1].1), "Cancel");
        let set = button_set(Some(&MessageDialogButtons::YesNo));
        assert_eq!(wire(&set[0].1), "Yes");
        assert_eq!(wire(&set[1].1), "No");
        assert_eq!(wire(&button_set(None)[0].1), "Ok");
    }

    #[test]
    fn custom_buttons_return_their_label() {
        // confirm(msg, {okLabel, cancelLabel}) 送 OkCancelCustom，拿回傳值與 okLabel 比。
        let buttons: MessageDialogButtons =
            serde_json::from_value(serde_json::json!({ "OkCancelCustom": ["刪除", "取消"] }))
                .unwrap();
        let set = button_set(Some(&buttons));
        assert_eq!(wire(&set[0].1), "刪除");
        assert_eq!(wire(&set[1].1), "取消");
        let buttons: MessageDialogButtons =
            serde_json::from_value(serde_json::json!({ "OkCustom": "知道了" })).unwrap();
        assert_eq!(wire(&button_set(Some(&buttons))[0].1), "知道了");
    }

    #[test]
    fn choices_map_to_buttons() {
        let labels = vec!["刪除".to_owned(), "取消".to_owned()];
        assert_eq!(resolve_button(&labels, "刪除"), Ok(0));
        assert_eq!(resolve_button(&labels, "ok"), Ok(0));
        assert_eq!(resolve_button(&labels, "cancel"), Ok(1));
        assert!(resolve_button(&labels, "亂按").is_err());
        let three = vec!["Yes".to_owned(), "No".to_owned(), "Cancel".to_owned()];
        assert_eq!(resolve_button(&three, "no"), Ok(1));
        assert_eq!(resolve_button(&three, "cancel"), Ok(2));
    }

    #[test]
    fn open_response_shapes() {
        assert_eq!(open_response(false, "cancel"), Value::Null);
        assert_eq!(open_response(true, "cancel"), Value::Null);
        assert_eq!(open_response(false, "/a.png"), Value::from("/a.png"));
        assert_eq!(open_response(true, "/a.png"), serde_json::json!(["/a.png"]));
        assert_eq!(
            open_response(true, r#"["/a","/b"]"#),
            serde_json::json!(["/a", "/b"])
        );
    }
}
