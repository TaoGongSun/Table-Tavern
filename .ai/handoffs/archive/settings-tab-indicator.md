> 結案 2026-10-04：squash 進 main；Sol 兩輪驗收通過。Windows 上 iframe 防護（`window !== window.top`）未實跑。

# settings-tab-indicator — 設定視窗切到 AI 連線後，分頁底線仍停在「外觀」

## 現象（2026-10-04 測試通道實測觀察）
開設定視窗、點「AI 連線」分頁：內容已切成 AI 連線，但分頁列的選中底線仍在「外觀」。

## 根因（已查清）
不是 app bug：`aria-current` 已正確移到 AI 連線，真 WebKit 真滑鼠點擊也不重現。測試通道的視窗被遮住或不在目前桌面空間時頁面是 `hidden`，WebKit 凍結動畫時間軸；`base.css` 給 button 的 `transition: border-color 0.15s` 永遠停在起點（`getAnimations()` 顯示 running、currentTime 0），`js` 讀樣式與 `shot` 截圖都是舊畫面。用 System Events 隱藏視窗可穩定重現。視窗設定 `backgroundThrottling: "disabled"` 實測無效。

## 修正
- `src-tauri/src/harness/motion.rs`：測試包 plugin 以 `js_init_script` 注入樣式，把 transition／animation 的時長與延遲歸零；`lib.rs` 只在 `test-harness` feature 掛上，正式包不含。腳本開頭 `window !== window.top` 就跳過：Windows 的 Wry 0.57 不理 for_main_frame_only，靠這行守住卡片介面 iframe 不注入（Sol 驗收必改）。
- `scripts/harness-e2e.mjs`：開設定後切 AI 連線，斷言底線只在 AI 連線、各分頁 transition-duration 為 0s。
- `SettingsWindow.webkit.test.tsx`：先斷言分頁的過渡時長不是 0s，再保留過渡、真滑鼠點 AI 連線，底線移過去（證明 app 本身正常）。

## 驗證
已 rebase 到含 stable-free-failover 包 A／B 的 main：verify 10 步全過（vitest 964、cargo 963、harness 29）；test:webkit 22/22；harness e2e 53 步全過；隱藏視窗條件下修正前底線卡外觀、修正後在 AI 連線（截圖在 `src-tauri/target/harness-shots/`，不進 git）。

