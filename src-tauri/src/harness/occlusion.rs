//! 測試包關掉 WKWebView 的視窗遮擋偵測：視窗被別的視窗蓋住時頁面仍當作 visible，
//! CSS 動畫與 requestAnimationFrame 照跑，`shot` 才拍得到卡片介面 iframe 的動畫／rAF 內容
//! （否則 hidden 時動畫凍結在起點，iframe 常是整片底色）。不搶前景。
//!
//! 用私有 SPI `_setWindowOcclusionDetectionEnabled:`，無相容保證：缺這個 selector 就記錄並維持現況。
//! 縮小、Cmd+H 時 `NSWindow.isVisible` 本身為 NO，此招救不了（實測 hidden）；其他 Space 未驗證。

#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;

#[cfg(target_os = "macos")]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Applied,
    SelectorMissing,
}

/// setter 只寫旗標不重算可見性：設完對視窗 post 公開的遮擋狀態通知讓它重跑。
/// SAFETY：`webview` 須是主執行緒上存活的 NSObject。
#[cfg(target_os = "macos")]
pub(crate) unsafe fn disable_detection(webview: *mut AnyObject) -> Outcome {
    use objc2::{class, msg_send, sel};
    let selector = sel!(_setWindowOcclusionDetectionEnabled:);
    let responds: bool = msg_send![webview, respondsToSelector: selector];
    if !responds {
        return Outcome::SelectorMissing;
    }
    let _: () = msg_send![webview, _setWindowOcclusionDetectionEnabled: false];
    let window: *mut AnyObject = msg_send![webview, window];
    if !window.is_null() {
        let name: *mut AnyObject = msg_send![
            class!(NSString),
            stringWithUTF8String: c"NSWindowDidChangeOcclusionStateNotification".as_ptr()
        ];
        let center: *mut AnyObject = msg_send![class!(NSNotificationCenter), defaultCenter];
        let _: () = msg_send![center, postNotificationName: name, object: window];
    }
    Outcome::Applied
}

/// setup 裡呼叫（主視窗已建立）。
#[cfg(target_os = "macos")]
pub(crate) fn apply(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Some(window) = app.get_webview_window("main") else {
        eprintln!("[test-harness] 找不到 main 視窗，遮擋偵測維持現況");
        return;
    };
    let result = window.with_webview(|webview| {
        // SAFETY：inner() 是這個視窗的 WKWebView；with_webview 在主執行緒執行。
        let outcome = unsafe { disable_detection(webview.inner() as *mut AnyObject) };
        if outcome == Outcome::SelectorMissing {
            eprintln!("[test-harness] 這版 WebKit 沒有 _setWindowOcclusionDetectionEnabled:，遮擋偵測維持現況（視窗被蓋住時 iframe 動畫／rAF 會凍結）");
        }
    });
    if let Err(error) = result {
        eprintln!("[test-harness] 遮擋偵測設定失敗：{error}");
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn apply(_app: &tauri::AppHandle) {}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use objc2::{class, msg_send};

    #[test]
    fn object_without_the_selector_is_left_alone() {
        // SAFETY：NSObject 實例，沒有該私有 selector；也沒有 window 方法，故只走 SelectorMissing 分支。
        let object: *mut AnyObject = unsafe { msg_send![class!(NSObject), new] };
        let outcome = unsafe { disable_detection(object) };
        // SAFETY：`new` 回傳 +1 持有，這裡是唯一持有者。
        let _: () = unsafe { msg_send![object, release] };
        assert_eq!(outcome, Outcome::SelectorMissing);
    }
}
