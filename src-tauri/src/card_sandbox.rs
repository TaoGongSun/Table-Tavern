//! 卡片介面沙盒外層（.ai/plans/desktop-csp.md）：自訂協定 `tt-sandbox` 回一份固定轉送文件，自帶網頁版沙盒同款寬政策，
//! 不經 Tauri 資產處理、不吃主視窗 CSP。卡片碼只跑在它裡面的內層 srcdoc：網址 about:srcdoc，Tauri 判 Remote、
//! 沒有 remote capability 一律拒；外層網址是註冊協定、Tauri 判 local，所以外層只准跑這份固定碼。

use tauri::http::{header, Response, StatusCode};

pub(crate) const SCHEME: &str = "tt-sandbox";
const DOCUMENT: &str = include_str!("card_sandbox.html");
const POLICY: &str = "default-src * data: blob: 'unsafe-inline' 'unsafe-eval'; form-action 'none'";

/// 只回根路徑那一份文件，其他路徑 404。
pub(crate) fn respond(path: &str) -> Response<Vec<u8>> {
    let builder = Response::builder();
    if path != "/" && !path.is_empty() {
        return builder
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new())
            .expect("固定回應");
    }
    builder
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CONTENT_SECURITY_POLICY, POLICY)
        .body(DOCUMENT.as_bytes().to_vec())
        .expect("固定回應")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
    use tauri::webview::InvokeRequest;

    #[tauri::command]
    fn probe() -> &'static str {
        "ok"
    }

    fn call(url: &str) -> Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
        let app = mock_builder()
            .register_uri_scheme_protocol(SCHEME, |_ctx, request| respond(request.uri().path()))
            .invoke_handler(tauri::generate_handler![probe])
            .build(mock_context(noop_assets()))
            .expect("mock app");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview");
        get_ipc_response(
            &webview,
            InvokeRequest {
                cmd: "probe".into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: url.parse().expect("url"),
                body: InvokeBody::default(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
    }

    fn sandbox_url() -> &'static str {
        if cfg!(windows) {
            "http://tt-sandbox.localhost/"
        } else {
            "tt-sandbox://localhost/"
        }
    }

    #[test]
    fn card_code_frame_is_rejected_as_remote_even_with_valid_key() {
        // 卡片碼所在的內層 srcdoc；Windows 上它拿得到 invoke key，擋它的是 Remote 拒絕
        let rejected = call("about:srcdoc").expect_err("about:srcdoc 必須被拒");
        assert!(rejected.to_string().contains("not allowed"), "{rejected}");
    }

    #[test]
    fn sandbox_outer_frame_counts_as_local() {
        // 這條成立正是外層絕不能跑卡片碼的理由；同一把 key 下 local 會通過，證明上一條是被 ACL 擋而不是 key 錯
        assert!(call(sandbox_url()).is_ok());
    }

    #[test]
    fn only_root_path_is_served_with_its_own_policy() {
        let page = respond("/");
        assert_eq!(page.status(), StatusCode::OK);
        assert_eq!(page.headers()[header::CONTENT_SECURITY_POLICY], POLICY);
        assert_eq!(respond("/other").status(), StatusCode::NOT_FOUND);
    }
}
