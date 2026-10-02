# Task
Task-ID: ui-redesign
Title: 介面整體重新設計
Status: in-progress
Created: 2026-09-30T21:30:00+08:00
Updated: 2026-10-02T13:00:00+08:00

## Summary
現行介面是 Opus 5.5 推出前做的，整體陽春。本案用新模型的設計能力重新設計整個 App 介面，並一併重新規劃各功能的顯示位置。

## 已拍板
- 整體設計一次定（全 app 功能位置＋視覺，HTML 靜態稿給使用者看），拍板後分畫面施工：遊玩主畫面→側欄→編輯頁→設定→對話窗〔作者裁決 2026-10-02〕。
- 大廳（桌子卡片牆）與牌桌拆兩個畫面〔作者裁決 2026-10-02〕。
- 桌卡有封面（該桌可見角色圖拼貼）〔作者裁決 2026-10-02〕。
- 陣容欄依可見角色卡數自動寬窄（≥2 寬、0–1 窄，手動拖過也照切）；封面有幾張圖拼幾張、單張鋪滿〔作者裁決 2026-10-02〕。
- 保留 Emblem 的 token 層（顏色／字級 token 名、七套主題）與三條骨架規則；元件與版面全部重做。Emblem 剩下的實聊驗收併進本案〔作者裁決 2026-10-02〕。

## 進度
- 分包 1（共用元件＋牌桌主畫面）已完成並 commit 在本分支：`src/shared/ui/{icons,MoreMenu}.tsx`、`src/styles/controls.css`、`src/views/{TableToolbar,StateBar,PlayView}.tsx`。施工規格經 Opus／Sol／Grok 三方共識，實作驗收也三方通過；verify 全綠；macOS release 包 800×600 實機已驗：七語系最大字級不換行、側欄拉寬＋最大字級降級為縮字／純圖示、幕晶片開前幕、狀態展開內捲、錯誤訊息在輸入框上方不推送出、串流中送出原位換停止且可中斷、收回與多層復原、⋯ 選單（使用者代點）。
- 分包 1 未實機驗：唯讀桌、齒輪更新紅點外觀與點擊開版本分頁（無新版可觸發）、換幕提醒＋錯誤＋狀態展開同時出現（靜態核算通過）、⋯ 選單鍵盤操作（macOS WKWebView 預設 Tab 不走按鈕）、Windows。分包 2 完成後重測牌桌時順帶看。
- 前幕面板（MainView 的 acts-flyout）仍是舊樣式，頂端一顆實色「隱藏」鈕；分包 2 動陣容欄／導航時一併換。

## 下一步
施工分包 2（大廳＋陣容欄，見 [plans](../plans/ui-redesign.md)「施工分包」第 2 條與「大廳」「左陣容欄」「窄欄細則」）。流程同分包 1：主線擬施工做法→送 /sol 與 /grok 審到三方共識→派子代理實作→主線 verify＋release 包實機→兩邊驗收。設計稿 https://claude.ai/artifact/KpS8dcMeaTfs8fE3rWQqJd（LobbyWide、TableSolo*、TableWorld* 等）。

## 實機測試備忘
- 背景自動化點不到 aria-haspopup 的按鈕（⋯），要請使用者代點；使用者點過 app 後背景點擊會失效，先 `open -a` 該 .app。
- 改語言／字級：關 app→備份 `~/Library/Application Support/TableTavern/config.json`→改 `preferences.language`／`text_size`（xs–xl）→開 app；測完原樣還原。觸發 AI 錯誤可暫改 `tier_models.best/balanced/fast` 為不存在的模型 id。
- 側欄寬度存在 WebView 本地儲存，不隨 config 還原，拖過要拖回 224。

## Constraints
- 排程靠後，先立案備忘〔作者裁決 2026-09-30〕。
- 功能位置遵循既有原則：放對位置不靠說明文字、編輯畫面按鈕置頂、玩家面一鍵完成＋可展開逐項決定。
