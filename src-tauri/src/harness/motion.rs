//! 測試包把 CSS 過渡與動畫時長歸零。
//! 視窗被遮住或不在目前桌面空間時頁面是 hidden，WebKit 凍結動畫時間軸：
//! 過渡停在起點（例：設定分頁切到 AI 連線，底線仍畫在外觀），`js` 讀到的計算樣式與 `shot`
//! 截圖都是舊畫面。歸零後樣式直接套到終值，與視窗是否可見無關。只注入主頁：Windows 的 Wry 不理 for_main_frame_only，腳本自己擋掉卡片介面 iframe。

use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;

const NO_MOTION_SCRIPT: &str = r#"(() => {
  if (window !== window.top) return;
  const add = () => {
    if (document.getElementById("tt-harness-no-motion")) return;
    const style = document.createElement("style");
    style.id = "tt-harness-no-motion";
    style.textContent = "*, *::before, *::after { transition-duration: 0s !important; transition-delay: 0s !important; animation-duration: 0s !important; animation-delay: 0s !important; }";
    (document.head || document.documentElement).appendChild(style);
  };
  if (document.documentElement) add();
  else document.addEventListener("DOMContentLoaded", add, { once: true });
})();"#;

pub(crate) fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("harness-motion")
        .js_init_script(NO_MOTION_SCRIPT.to_owned())
        .build()
}
