# vn-bgm-multi-mood-tag：VN 配樂標記寫兩個情緒時另抓一首

案別：小案（核心約 15 行）。分支 `vn-bgm-multi-mood-tag`。

## 目標（主線裁定 2026-11-15）
- `[bgm: A, B]` 另抓一首是原設計（兩個情緒＝一組，見 [plans/vn-audio.md](../plans/vn-audio.md)），不是解析錯誤。改提示詞：中英兩條都只請 GM 寫一個情緒；解析與挑曲不動，舊桌與已匯出作品包裡的 `bgm-a+b.mp3` 照樣播。
- 同案：VN 旁白與對白裡的 `<br>`（含 `<br/>`、`<BR />`）在 `parse_cells` 產出文字時換成換行，緊跟的一個換行併進去、結尾不留空行；只有 `<br>` 的格丟掉。

## 現況
- 已改：`src-tauri/src/transport/vn_format.rs`（中英提示詞）、`src-tauri/src/chat_turn/vn_script.rs`（`br_to_newline`）、測試 `vn_script_tests.rs` 一條、方案檔 vn-audio 相關行。
- 下一步：主線讀 diff、重跑 verify、送驗收、結案合 main。

## 已知限制
- GM 偶爾不照做仍寫兩個情緒時，照原設計另抓組合曲（低階模型遵守度不修）。

> 結案（2026-11-15）：已合 main。
