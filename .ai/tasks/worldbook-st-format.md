# Task
Task-ID: worldbook-st-format
Title: 世界書 v2：ST 相容條目化＋一鍵匯入＋可見性資訊邊界
Status: completed
Created: 2026-07-24T19:10:00+08:00
Updated: 2026-07-24T23:40:00+08:00

## Summary
2026-07-24 與使用者拍板：世界書採 SillyTavern 獨立世界書 JSON 為內部格式（事實標準，Chub/RisuAI/Agnai 皆相容），檔案 `worlds/<world>/worldbook.json` 原樣保存、只解讀子集（key／comment／content／constant／order／disable），未知欄位不丟。ST 沒有可見性概念，資訊邊界走官方後門 `extensions.table_tavern.visibility`：`"gm"`（預設）／`"public"`／`{"characters":[...]}`。一鍵匯入收兩形（entries map＝獨立世界書；entries 陣列＝character_book 形，欄位名映射），append 合併、uid 續編、匯入預設 gm。觸發 v1：constant 直進；有關鍵字者掃最近 4 則訊息子字串（不分大小寫）；token budget／機率／遞迴不做。注入：GM 看全部觸發條目（world.md 之後「## 世界書」段）；角色只拿 public＋指定自己的條目。world.md 保留不動（世界總覽，整包進 GM）。研究原文與欄位對照存於 scratchpad（session dbb6d324）。

## Next action
- 無。2026-07-24 使用者實測全數通過（含資訊邊界實聊：指定角色 Knight 知情、未指定 Fox 不知情），結案。進階觸發（token budget／機率／遞迴）未排程，需要時另開任務。

## Constraints
worldbook.json 未知欄位原樣保留（匯入不掉資料、匯出可回 ST）；無 worldbook.json 的舊桌行為完全不變；角色一律看不到 gm 條目與他人專屬條目；transcript 不動。
