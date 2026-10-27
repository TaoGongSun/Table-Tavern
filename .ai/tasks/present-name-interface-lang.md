# present-name-interface-lang — 在場名單人名改照介面語言

## Summary
GM 狀態裡 present 的人名字形時穩時不穩（簡體寫成繁體或相反）。裁決：present 人名一律轉成介面語言〔作者裁決 2026-10-27〕。

## 已知線索
- 根因是兩條指示互相衝突：`src-tauri/src/transport/messages.rs:21` 要求輸出一律繁體；`src-tauri/src/transport/response.rs:53`（英文版 :45）要求 present 照原字抄。
- 比對端已做簡繁折疊（`transport/arrivals.rs:16-17`、`data/scene/presence.rs`、`transport/state_view.rs:324-381`），改字形不影響比對。
- 狀態路徑（JSONPatch 的鍵）要不要也照介面語言：作者沒有裁決〔模型判斷·未裁決〕，開工時先確認。

## Next action
未排程；開工先問狀態路徑那題，再改 response.rs 的 present 指示（十語系檢查）。

## 驗收方向
介面語言為繁體時 present 穩定寫繁體、為簡體時穩定寫簡體；換幕、登場、隱藏結算結果不變；cargo test 蓋指示文字與折疊比對。
