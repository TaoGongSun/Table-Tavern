# Task
Task-ID: ttrpg-rules-system
Title: 跑團規則系統：規則書引入＋擲骰＋角色紙（規則中立引擎，零內建內容）
Status: todo
Created: 2026-08-13
Updated: 2026-11-07
案別：v1 中案（核心估 200–300：骰池生成、骰鈕面板、骰記號偵測、指南文案）。

## Summary
app 是規則中立引擎：規則書＝世界書的一種，內建零規則內容，版權責任在玩家側。既有地基沿用：關鍵字觸發、ST 世界書匯入、匯入預設 visibility=GM。拍板結論、分期、驗收見 [plans/ttrpg-rules-system.md](../plans/ttrpg-rules-system.md)。

**跑團不是獨立桌種**，是聊天桌上的能力組（規則書＋骰池＋角色紙＋戰鬥數值面板），大廳以徽章標示；畫面仍是聊天桌加側欄，另立桌種會把引擎再叉一次（GM 接力已有兩份實作）。〔作者裁決 2026-11-07，採納主線建議〕

## 與相鄰案的邊界
- 狀態欄容器與無卡桌預設欄位歸 [table-status-panel](table-status-panel.md)；戰鬥數值（先攻、HP 加減）歸 [ttrpg-combat-tracker](ttrpg-combat-tracker.md)；本案管規則書、骰子、角色紙初始宣告。
- 大廳徽章與篩選歸 [lobby-table-badges](lobby-table-badges.md)。
- 原方案 v2 寫「等狀態欄二期」：二期已以 state-values-mvu 結案，前置改成 table-status-panel 的角色格。

## Next action
- v1 不依賴任何狀態欄工作，可直接開工；開工第一步照規約重讀現況（MVU 引擎、`TableState.characters` 佔位、多人房間協定），重估範圍。
- 開工前要拍：骰鈕在多人房間由誰按、擲果如何進房間對話史（v1 可先限單人桌，多人列 v2）〔模型判斷·未裁決〕。

## Constraints
- app 永不內建、永不散布規則內容；指南只指開放授權來源與社群檔。
- 不替玩家花錢：燒額度的功能先報價、玩家確認才跑。
- 骰子誠實底線：骰面一律 app 真隨機產生，模型只能取用；模型自報永不做。
