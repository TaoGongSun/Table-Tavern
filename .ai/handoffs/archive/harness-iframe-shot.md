> 結案 2026-10-07：已進 main，Sol、Grok 驗收通過。

# 測試通道截不到卡片介面 iframe

Status: done

## 結論
- 根因：視窗被別的視窗蓋住時頁面 `visibilityState=hidden`，WebKit 凍結 CSS 動畫並停掉 `requestAnimationFrame`，卡片介面 iframe 停在起點、`shot` 只見底色（iframe 本身拍得到）。
- 修法：`src-tauri/src/harness/occlusion.rs` 在 harness 啟動時對主 WKWebView 呼叫私有 SPI `_setWindowOcclusionDetectionEnabled:NO` 並 post 遮擋狀態通知；只在 test-harness＋macOS 編入，缺 selector 時記一行並維持現況，不搶前景。
- 邊界、實測數據與備援繞法見 [test-harness](test-harness.md) 已知限制：縮小／Cmd+H 無效，其他 Space 未驗證，私有 SPI 無相容保證。
