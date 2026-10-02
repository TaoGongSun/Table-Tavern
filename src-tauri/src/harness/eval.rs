//! eval 往返：Tauri 的 `eval` 不回傳值，所以每次派一組 id＋一次性 nonce，
//! 頁面跑完用事件 `harness-reply`（經 `plugin:event|emit` IPC）送回，Rust 依 id 喚醒等待者。
//! nonce 只防偽造回報，不證明主頁腳本可信。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;
use tauri::{Listener, Manager};
use tokio::sync::oneshot;

const REPLY_EVENT: &str = "harness-reply";

type Waiter = (String, oneshot::Sender<Reply>);

static PENDING: Mutex<Option<HashMap<String, Waiter>>> = Mutex::new(None);

#[derive(Debug, Deserialize)]
struct Reply {
    id: String,
    nonce: String,
    ok: bool,
    /// 成功時是 JSON 字串（頁面端 stringify），失敗時為 None。
    json: Option<String>,
    error: Option<String>,
}

fn with_pending<T>(f: impl FnOnce(&mut HashMap<String, Waiter>) -> T) -> T {
    let mut guard = PENDING.lock().unwrap_or_else(|p| p.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}

pub(super) fn pending_count() -> usize {
    with_pending(|map| map.len())
}

pub(super) fn listen(app: &tauri::AppHandle) {
    app.listen_any(REPLY_EVENT, |event| {
        let Ok(reply) = serde_json::from_str::<Reply>(event.payload()) else {
            return;
        };
        if let Some((_, sender)) = take_waiter(&reply.id, &reply.nonce) {
            let _ = sender.send(reply);
        }
    });
}

/// id 與 nonce 都對才取出等待者；用過即刪，同一組不能回報第二次。
fn take_waiter(id: &str, nonce: &str) -> Option<Waiter> {
    with_pending(|map| match map.get(id) {
        Some((expected, _)) if expected == nonce => map.remove(id),
        _ => None,
    })
}

/// 包裝成頁面端腳本。使用者程式碼以 JSON 字串傳入，用 AsyncFunction 建構：
/// 先試成「回傳該運算式」，語法不通再當成函式本體（需自己寫 return）。
pub(super) fn wrap(id: &str, nonce: &str, source: &str) -> String {
    let src = serde_json::to_string(source).expect("字串必可序列化");
    format!(
        r#"(async () => {{
  const id = "{id}", nonce = "{nonce}";
  const send = (r) => window.__TAURI_INTERNALS__.invoke("plugin:event|emit", {{
    event: "{REPLY_EVENT}", payload: Object.assign({{ id, nonce }}, r)
  }});
  let r;
  try {{
    const AsyncFunction = (async () => {{}}).constructor;
    const src = {src};
    let fn;
    try {{ fn = new AsyncFunction("return (" + src + "\n);"); }} catch (_) {{ fn = new AsyncFunction(src); }}
    const value = await fn();
    try {{
      const json = JSON.stringify(value === undefined ? null : value, (k, v) => {{
        if (typeof Node !== "undefined" && v instanceof Node) throw new TypeError("回傳值含 DOM 節點，請改回傳文字或屬性");
        if (v === window) throw new TypeError("回傳值含 window");
        return v;
      }});
      r = {{ ok: true, json }};
    }} catch (e) {{
      r = {{ ok: false, error: "回傳值無法 JSON 化：" + (e && e.message || e) }};
    }}
  }} catch (e) {{
    r = {{ ok: false, error: String(e) + (e && e.stack ? "\n" + e.stack : "") }};
  }}
  try {{ await send(r); }} catch (e) {{ console.error("[harness] 回報失敗", e); }}
}})();"#
    )
}

pub(super) async fn run(
    app: &tauri::AppHandle,
    source: &str,
    timeout: Duration,
) -> Result<serde_json::Value, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到 main 視窗".to_owned())?;
    let id = super::random_hex(8);
    let nonce = super::random_hex(16);
    let (sender, receiver) = oneshot::channel();
    with_pending(|map| map.insert(id.clone(), (nonce.clone(), sender)));
    if let Err(error) = window.eval(wrap(&id, &nonce, source)) {
        with_pending(|map| map.remove(&id));
        return Err(format!("eval 派送失敗：{error}"));
    }
    match tokio::time::timeout(timeout, receiver).await {
        Ok(Ok(reply)) if reply.ok => {
            let json = reply.json.unwrap_or_else(|| "null".to_owned());
            serde_json::from_str(&json).map_err(|e| format!("回報 JSON 解析失敗：{e}"))
        }
        Ok(Ok(reply)) => Err(reply.error.unwrap_or_else(|| "頁面回報失敗".to_owned())),
        Ok(Err(_)) => Err("回報通道已關閉".to_owned()),
        Err(_) => {
            with_pending(|map| map.remove(&id));
            Err(format!(
                "逾時 {} ms：已清除等待，但已派出的 JS 及其觸發的後端操作不會被撤銷",
                timeout.as_millis()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_source_is_embedded_as_json_string() {
        let script = wrap("abc", "def", "\"); alert(1); (\"");
        assert!(script.contains(r#"const src = "\"); alert(1); (\"";"#));
        assert!(script.contains(r#"const id = "abc", nonce = "def";"#));
    }

    #[test]
    fn reply_needs_matching_nonce_and_is_single_use() {
        let (tx, mut rx) = oneshot::channel();
        with_pending(|m| m.insert("x1".into(), ("good".into(), tx)));
        assert!(take_waiter("x1", "bad").is_none());
        assert!(rx.try_recv().is_err());
        assert!(take_waiter("x1", "good").is_some());
        assert!(take_waiter("x1", "good").is_none(), "nonce 只能用一次");
    }
}
