# Handoff: character-card-avatar-issues

## Current state
五項＋建卡新流程＋刪除入口＋GM 書皮卡全部實作完成，使用者實測通過（GM 書皮含七主題逐一驗收，commit 7597061）；僅剩改名提示複驗與一個先擱置的圖庫路徑 bug。

## Completed
見 [archive/character-card-avatar-issues-completed.md](archive/character-card-avatar-issues-completed.md)（含使用者原文與驗證證據）。

## Remaining
2026-07-27 使用者實測結果：桌列表分隔線、角色圖示 emoji 欄位、桌刪除鈕、角色刪除鈕、建卡新流程、GM 書皮卡（七主題）—— **全部通過**。
改名補了「劇情正文裡的舊名不會更動」提示（commit 8cbcc71）待複驗。

## Next action
1. **生成圖庫目錄放錯層**：`gallery_dir` 落在 `{data_root}/{world}/gen-gallery/{角色}`，但世界資料夾是 `{data_root}/worlds/{world}`——圖庫是 `worlds/` 的兄弟目錄，不在世界裡。刪桌與刪角色都已各自補上清圖庫，但 `rename_world` 仍只搬 `worlds/{名}`＝改桌名後整個生成圖庫失聯。修法是移到世界資料夾內並加一次性搬移（舊路徑存在且新路徑沒有就搬），15 分鐘的事。**但先別動**：使用者提案改用代碼定址（見 [stable-id-storage](../tasks/stable-id-storage.md)），若採用則此 bug 自動消失，現在修是白工。等那個提案拍板後再決定。

## Constraints
- 沿用 character-image-avatar 既有拍板：頭像存正方形 PNG、圓框走 CSS；移除全身圖不連動刪頭像；刪角色清兩檔。
- 編輯畫面按鈕列一律置頂（全 app 統一），例外只有生圖對話框的主要動作放右下。
- **側欄卡片高度差是刻意的**（2026-07-27 使用者拍板）：有全身圖的角色卡 69px、只有 emoji 或圓頭像的 44px。有圖的卡比較高＝比較顯眼，用來讓玩家想要角色圖、進而想用 AI 生圖。程式碼上它看起來像 `height: 100%` 解析不出來的意外（App.css `.tcard-image` 有註記），**不要當 bug 修掉**；GM 卡是用 `min-height: 4.3125rem` 明確對齊到 69px 那一檔。
