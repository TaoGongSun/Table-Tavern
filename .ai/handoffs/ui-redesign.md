# Task
Task-ID: ui-redesign
Title: 介面整體重新設計
Status: in-progress
Created: 2026-09-30T21:30:00+08:00
Updated: 2026-10-02T14:40:00+08:00

## Summary
現行介面是 Opus 5.5 推出前做的，整體陽春。本案用新模型的設計能力重新設計整個 App 介面，並一併重新規劃各功能的顯示位置。

## 已拍板
- 整體設計一次定（全 app 功能位置＋視覺，HTML 靜態稿給使用者看），拍板後分畫面施工：遊玩主畫面→側欄→編輯頁→設定→對話窗〔作者裁決 2026-10-02〕。
- 大廳（桌子卡片牆）與牌桌拆兩個畫面〔作者裁決 2026-10-02〕。
- 桌卡有封面（該桌可見角色圖拼貼）〔作者裁決 2026-10-02〕。
- 陣容欄依可見角色卡數自動寬窄（≥2 寬、0–1 窄，手動拖過也照切）；封面有幾張圖拼幾張、單張鋪滿〔作者裁決 2026-10-02〕。
- 保留 Emblem 的 token 層（顏色／字級 token 名、七套主題）與三條骨架規則；元件與版面全部重做。Emblem 剩下的實聊驗收併進本案〔作者裁決 2026-10-02〕。

## 進度
- 分包 1（共用元件＋牌桌主畫面）與分包 2（大廳＋陣容欄）已完成並 commit 在本分支，施工規格與實作驗收都經 Opus／Sol／Grok 三方共識；定案重點在 [plans](../plans/ui-redesign.md)「分包 2 施工定案」。
- 分包 2 程式落點：大廳 `src/features/lobby/`（Lobby、TableCard、封面懶載、進出桌互斥鎖 useTableOp）、陣容欄 `src/views/Cast{Rail,Cards,Archive}.tsx`、故事貼底 `src/features/story-scroll/`；舊 TableSidebar 與前幕浮層已刪。
- 分包 2 已實機驗（macOS release 800×600／1280×800）：開機進大廳（含零桌首開、英文＋最大字級）、直式封面 2:3 與多圖上限、懶載、桌卡改名／刪桌、空桌回收、進桌／回大廳、窄欄↔寬欄即時切換（窄欄封存面板還原）、幕晶片選單、新增選單往上開、編輯頁點卡過未儲存守門、視窗變寬故事貼底。
- 未實機驗：唯讀／需修復桌從大廳進入與徽章、尚未設定 AI 引導在大廳的樣子（需空白設定）、窄欄封存面板＋最大字級、寬欄拖到上限＋最大字級、十語系長字逐一看、分包 1 遺留項（唯讀桌、齒輪紅點、換幕提醒＋錯誤＋狀態同時、⋯ 鍵盤、Windows）。
- 前幕面板已改成幕晶片的下拉選單（分包 2 完成）。

## 下一步
施工分包 3（編輯頁三種＋世界書工具列，見 plans「編輯頁」與「施工分包」第 3 條）。流程同前：主線擬施工做法→送 /sol 與 /grok 審到三方共識→派子代理實作→主線 verify＋release 包實機→兩邊驗收。上列未實機驗項目在分包 3 實機時順帶看。

## 實機測試備忘
- 背景自動化點不到 aria-haspopup 的按鈕（⋯），要請使用者代點；使用者點過 app 後背景點擊會失效，先 `open -a` 該 .app。
- 改語言／字級：關 app→備份 `~/Library/Application Support/TableTavern/config.json`→改 `preferences.language`／`text_size`（xs–xl）→開 app；測完原樣還原。觸發 AI 錯誤可暫改 `tier_models.best/balanced/fast` 為不存在的模型 id。
- 側欄寬度存在 WebView 本地儲存，不隨 config 還原，拖過要拖回 224。
- 實測前把 `~/Documents/TableTavern/worlds` 與 config.json 整份備份，測完 `diff -rq` 比對還原；要多圖／窄欄情境就複製一桌到合法 ULID 新目錄（26 字、Crockford 字元，改 state.json 的 id／name）再改角色 md 的 archived。
- app 不在前景時背景點擊常失效：先 `osascript -e 'tell application "Table Tavern" to activate'`。

## Constraints
- 排程靠後，先立案備忘〔作者裁決 2026-09-30〕。
- 功能位置遵循既有原則：放對位置不靠說明文字、編輯畫面按鈕置頂、玩家面一鍵完成＋可展開逐項決定。
