# Task
Task-ID: ui-redesign
Title: 介面整體重新設計
Status: awaiting-verification
Created: 2026-09-30T21:30:00+08:00
Updated: 2026-10-02T19:52:00+08:00

## Summary
現行介面是 Opus 5.5 推出前做的，整體陽春。本案用新模型的設計能力重新設計整個 App 介面，並一併重新規劃各功能的顯示位置。

## 已拍板
- 整體設計一次定（全 app 功能位置＋視覺，HTML 靜態稿給使用者看），拍板後分畫面施工：遊玩主畫面→側欄→編輯頁→設定→對話窗〔作者裁決 2026-10-02〕。
- 大廳（桌子卡片牆）與牌桌拆兩個畫面〔作者裁決 2026-10-02〕。
- 桌卡有封面（該桌可見角色圖拼貼）〔作者裁決 2026-10-02〕。
- 陣容欄依可見角色卡數自動寬窄（≥2 寬、0–1 窄，手動拖過也照切）；封面有幾張圖拼幾張、單張鋪滿〔作者裁決 2026-10-02〕。
- 保留 Emblem 的 token 層（顏色／字級 token 名、七套主題）與三條骨架規則；元件與版面全部重做。Emblem 剩下的實聊驗收併進本案〔作者裁決 2026-10-02〕。

## 進度
- 分包 1–5 全部完成並已合併進 main；施工定案在 [plans](../plans/ui-redesign.md)「分包 2／3／4／5 施工定案」。分包 1–3 經 Opus／Sol／Grok 三方共識；分包 4 送審三方、驗收 Opus＋Sol；分包 5 送審與驗收都是 Opus＋Sol（Grok 額度用完）〔作者裁決 2026-10-02〕。
- 程式落點：大廳 `src/features/lobby/`、陣容欄 `src/features/characters/Cast{Rail,Cards,Archive}.tsx`、故事貼底 `src/features/story-scroll/`；編輯頁共用頁框 `src/shared/ui/EditPage.tsx`、世界書工具列 `src/features/worldbook/WorldbookSection.tsx`；設定外框 `src/features/settings/SettingsWindow.tsx`、AI 表單與儲存列 `src/features/settings/SettingsForm.tsx`、連線方式 `src/features/settings/TransportChoice.tsx`、外部指定分頁 `src/features/settings/useRequestedTab.ts`、外框樣式 `src/styles/settings-window.css`；對話窗共用外框 `src/shared/ui/Dialog.tsx`（原生 `<dialog>`）、裁切與 AI 生圖 `src/features/characters/CardImageDialogs.tsx`、對話窗樣式 `src/styles/dialogs.css`。
- 已實機驗（macOS release 800×600）：分包 1–4 見各自 commit 訊息；分包 5 俄文＋最大字級：一句話開桌、設定＋巢狀 CLI 權限提示（Esc 只關上層）、AI 生圖、裁切（框內按下框外放開不關）、燈箱、匯入身分／路由、開場白面板、介面卡關閉鈕，Esc／遮罩／焦點歸還照表；系統確認窗俄文按鈕（未儲存離開）不截字。
- 2026-10-02 實機過：範例桌詢問（en 三鈕 Cancel, pick a language again／No thanks／Create，取消後語言退回）。⋯ 鍵盤不正常，另立案 menu-keyboard-webkit 處理。
- 未實機驗：重構三窗（要 AI）、一句話開桌有綱要後的底列（梯 2）；格式轉換更新窗（要有含格式轉換的新版）、設定外部指定分頁與齒輪紅點（要有新版）、換幕提醒＋錯誤＋狀態同時（要真出錯）、WebView2 連按 Esc 與 Windows 外觀（無 Windows 機）（梯 3）。十語系長字只以俄文代表。
- 自 ui-overhaul 併入（未實機驗）：實聊時 dialogue 事件的名牌版式、串流中打字指示。
- 自 desktop-update-detect 移交（未處理）：畫面位置與為過按鈕寬度選的譯詞要重定——法文「更新」Installer、德文 Updaten／Nochmal、西葡「重試」Repetir〔作者裁決 2026-10-02〕。
- 觀察（範圍外、未處理）：牌桌工具列有「介面卡」鈕時，俄文 800px 下桌名 wedge 縮到只剩一字。

## 下一步
已結案進 main（分支已刪）〔作者裁決 2026-10-02：先結案、未實機驗項目之後補測〕。補測上列「未實機驗」各項（排在[實測佇列](../reference/verification-queue.md)），過了就結案移出索引。

## 實機測試備忘
- 背景自動化點不到 aria-haspopup 的按鈕（⋯、幕晶片、封存圖示鈕），要請使用者代點；全螢幕操控的點擊送不進這個 app、背景鍵盤移焦也看不到焦點，都不可行。使用者點過 app 後背景點擊會失效，先 `open -a` 該 .app；重開 app 後先截圖確認畫面再點，免得點擊落到輸入區的發言鈕。
- 造唯讀桌：複製一桌到新 ULID 目錄、加 `format.json` `{"format_version": 999}`；造需修復桌：只建 `.tt-staging-<ULID>/state.json`。測完刪掉。
- 改語言／字級：關 app→備份 `~/Library/Application Support/TableTavern/config.json`→改 `preferences.language`／`text_size`（xs–xl）→開 app；測完原樣還原。觸發 AI 錯誤可暫改 `tier_models.best/balanced/fast` 為不存在的模型 id。
- 側欄寬度存在 WebView 本地儲存，不隨 config 還原，拖過要拖回 224。
- 實測前把 `~/Documents/TableTavern/worlds` 與 config.json 整份備份，測完 `diff -rq` 比對還原；要多圖／窄欄情境就複製一桌到合法 ULID 新目錄（26 字、Crockford 字元，改 state.json 的 id／name）再改角色 md 的 archived。
- app 不在前景時背景點擊常失效：先 `osascript -e 'tell application "Table Tavern" to activate'`。

## Constraints
- 排程靠後，先立案備忘〔作者裁決 2026-09-30〕。
- 功能位置遵循既有原則：放對位置不靠說明文字、編輯畫面按鈕置頂、玩家面一鍵完成＋可展開逐項決定。
