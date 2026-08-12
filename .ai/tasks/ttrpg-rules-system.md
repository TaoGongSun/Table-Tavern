# Task
Task-ID: ttrpg-rules-system
Title: 跑團規則系統：規則書引入＋擲骰＋角色紙（規則中立引擎，零內建內容）
Status: todo
Created: 2026-08-13T00:30:02.109574+08:00
Updated: 2026-08-13T00:30:02.109574+08:00

## Summary
2026-08-02 與使用者五題拍板。定位：app 是規則中立引擎——規則書＝世界書的一種，內建零規則內容（含 D&D 5e SRD），版權責任在玩家側，WoD 等無授權系統自動被通用匯入覆蓋。既有地基全部沿用：關鍵字觸發（`transport.rs` `active_worldbook_entries`，掃最近 4 則）、ST 世界書 JSON 匯入（`data.rs` `import_worldbook`，物件／陣列兩式皆吃）、匯入預設 visibility=GM。與 st-ecosystem 第四項的邊界已互寫（見該檔二期段）。

規格細節（拍板結論、分期、驗收（v1））見 [plans/ttrpg-rules-system.md](../plans/ttrpg-rules-system.md)。

## Next action
- 五題拍板完成（2026-08-02），排程晚於 st-ecosystem；v1（指南＋骰池＋骰鈕＋注入實測）不依賴狀態欄，v2 等狀態欄二期後細拍

## Constraints
- app 永不內建、永不散布規則內容；指南只指開放授權來源與社群檔。
- 不替玩家花錢：任何燒額度的功能（AI 整理精靈等）先報價、玩家確認才跑。
- 骰子誠實底線：骰面一律 app 真隨機產生，模型只能取用；模型自報永不做。
