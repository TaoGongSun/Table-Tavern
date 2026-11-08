# Task
Task-ID: ttrpg-rules-system
Title: 跑團規則系統：規則書引入＋擲骰＋角色紙（規則中立引擎，零內建內容）
Status: in-progress
Created: 2026-08-13
Updated: 2026-11-08
案別：v1 大案，核心估 900–950 行（非測試、非 i18n 譯文、非純型別連帶）；含持久化對帳、單人骰鈕與房間骰池顯示。〔模型判斷·未裁決〕

## Summary
app 是規則中立引擎：規則書＝世界書的一種，內建零規則內容，版權責任在玩家側。既有地基沿用：關鍵字觸發、ST 世界書匯入、匯入預設 visibility=GM。拍板結論、分期、驗收見 [plans/ttrpg-rules-system.md](../plans/ttrpg-rules-system.md)。

**跑團不是獨立桌種**，是聊天桌上的能力組（規則書＋骰池＋角色紙＋戰鬥數值面板），大廳以徽章標示；畫面仍是聊天桌加側欄，另立桌種會把引擎再叉一次（目前 GM 接力已合一）。〔作者裁決 2026-11-07，採納主線建議〕

## 與相鄰案的邊界
- 狀態欄容器與無卡桌預設欄位歸 [table-status-panel](table-status-panel.md)；戰鬥數值（先攻、HP 加減）歸 [ttrpg-combat-tracker](../tasks/ttrpg-combat-tracker.md)；本案管規則書、骰子、角色紙初始宣告。
- 大廳徽章與篩選歸 [lobby-table-badges](lobby-table-badges.md)。
- 原方案 v2 寫「等狀態欄二期」：二期已以 state-values-mvu 結案，前置改成 table-status-panel 的角色格。

## 目前狀態與下一步
- 分支 `ttrpg-rules-system`，B 基底 `4e168b5d`；主線回報 A 已通過 opus-review 與本機完整 verify。B 修正已施工待主線複審，未提交／合併；Git 仍全部由主線操作。〔模型判斷·未裁決〕
- v1 維持骰池＋單人骰鈕＋指南及常識審修正；房間骰鈕、角色紙結構化、嚴格模式與整理精靈留 v2。〔作者裁決 2026-11-08〕
- B 含 Rust 唯一骰式解析／真亂數、鎖內幕號／回合／接力檢查、無資料 dice_roll marker 與獨立樓層、composer 候選／自訂／玩家時段與停用原因、空白沿原送出模式續敘／有草稿等待、桌設定與兩路匯入提示、官方來源小指南。刪掉 A 不可達的池沿用分支與 getter。〔模型判斷·未裁決〕
- 擲骰只送骰式與幕號；成功收據先顯示，失敗只重讀不補擲。GM 續敘失敗保留結果；每個 await 後核對檢視世代，草稿完整保留。開關與匯入會重讀對話顯示公開說明；曾開房間的桌回單人仍可擲，擲果進房間既有 seed／sync 供重開對帳。〔模型判斷·未裁決〕
- B 包目前核心 557、測試 351 行，另有純型別連帶 26 行；計算新增非空白程式行，扣註解、譯文與純型別連帶，測試小於核心兩倍。骰鈕與設定改由 tableState 共享開關；玩家時段只由 UI 停用、自訂範圍只由 Rust 驗證。沒有玩家卡時本文省略名字，由標頭表達擲骰。〔模型判斷·未裁決〕
- 下一步：主線審 diff、在本機重跑 verify 並驗原生互動，再討論注入實測；本輪未打模型、未下載／轉檔規則內容。三份他人檔案未碰，不得納入提交。〔模型判斷·未裁決〕

## 驗證與環境限制
- 最新骰子相關 Rust 16 項通過，含接力段間空窗、過期幕、邊界解析、兩種條目格式、CLI／API 公開歷史與收回復原與房間 seed。〔模型判斷·未裁決〕
- 完整 cargo test：2706 通過／86 失敗／7 忽略；逐項確認 86 項皆有 Operation not permitted。不能將完整 cargo test 稱為綠。〔模型判斷·未裁決〕
- 最終 npm run verify 第 1–11 步通過：桌面 2015、網頁 878 通過／1 跳過，結構／版本／隔離／個資／fmt／i18n／建置皆過；第 12 步 party 225 通過／5 項 listen EPERM 失敗，第 13–17 步未執行。其後所需檢查另跑：最新前端相關 71 項及 party typecheck／build 均通過。〔模型判斷·未裁決〕
- 原生互動仍需主線本機驗：候選與自訂、草稿雙軌、失敗不補擲、設定後通知、兩路匯入提示及指南。沙盒既有原生 harness 控制鎖 EPERM，本輪未取得原生截圖。〔模型判斷·未裁決〕
- 日誌與行數分類：`/tmp/ttrpg-b-review-*.log`、`/tmp/ttrpg-b-review-metrics.json`；不包含任何真模型測量。〔模型判斷·未裁決〕

## 本輪提交清單
僅暫存下列檔案；不納入 `.ai/BACKLOG.md`、`.ai/tasks/ttrpg-combat-tracker.md`、`.ai/tasks/mobile-room-host.md`。〔模型判斷·未裁決〕

- `.ai/handoffs/ttrpg-rules-system.md`
- `.ai/plans/ttrpg-rules-system.md`
- `src-tauri/src/dice.rs`
- `src-tauri/src/dice_tests.rs`
- `src/App.tsx`
- `src/features/dice/DiceControls.test.tsx`
- `src/features/dice/DiceControls.tsx`
- `src/features/dice/DiceSettings.tsx`
- `src/features/dice/dice-ui.ts`
- `src/features/table-state/useTableStateController.test.ts`
- `src/features/table-state/useTableStateController.ts`
- `src/features/worldbook/WorldEditor.tsx`
- `src/views/AppWorkspace.tsx`
- `src/views/MainView.tsx`
