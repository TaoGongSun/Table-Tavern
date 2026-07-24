# Task handoff
Task-ID: worldbook-st-format
Updated: 2026-07-24T04:07:22.610531+00:00
Status: in-progress

## Goal
世界書 v2：採 SillyTavern 世界書 JSON 為內部格式（條目化）、一鍵匯入（獨立世界書與 character_book 兩形）、以 extensions.table_tavern.visibility 實作 GM 專有／全體公開／指定角色可見的資訊邊界，含條目管理 UI。

## Current state
R1 後端＋R2 前端全部完成，主線複驗全綠並親讀兩輪 diff；已 commit push。剩使用者實測 UI 驗收即可結案。

## Completed
- 研究：ST World Info 機制與 31 欄位、character_book V2/V3 差異、可見性缺口確認，原文摘錄存 scratchpad（st-worldinfo-fields.md、character-book-spec.md、st-sample-worldinfo.json）。
- 拍板：worldbook.json 原樣保存只解讀子集；可見性走 extensions；world.md 保留不動；觸發 v1 只做 constant＋近 4 則關鍵字掃描。
- 任務檔 .ai/tasks/worldbook-st-format.md 已建、.ai/TASKS.md 已列 In progress。
- 交辦檔：scratchpad/task-worldbook-r1.md（後端，已發包）、task-worldbook-r2.md（前端草稿）。

## Verification
- R1 主線本機 cargo test：55 passed; 0 failed（新增 7 個世界書測試涵蓋規格全部驗收點）。
- 主線親讀：Visibility serde tagged 形（data.rs:149）、觸發函式（transport.rs:48）、雙注入段落（transport.rs:107–113、157–163）、五個 commands 薄包裝（lib.rs:60 起）皆符規格。
- 已知陷阱已寫進 R2 規格並實作正確：新增條目前端傳 Number.MAX_SAFE_INTEGER（App.tsx:605）；visibility 前後端交換用 {"type":...} tagged 形（App.tsx:595–603）。
- R2 主線複驗：npm run build ✓ built in 405ms 無 TS 錯誤；親讀 WorldEditor 全部新碼（App.tsx:517–815）：CRUD、匯入 FileReader→import_worldbook、匯出 save 對話框、刪除 confirm 皆符規格。

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main
- HEAD: 441976a0da53849fd4b8e19b828e5aeaae8b6cb0
- Dirty: true
- Dirty fingerprint: 9275af94d3a2234b744e347bff4529822554138be5c2e8af85654c4072d7df75

## Remaining
- 使用者實測：世界設定畫面新增條目（三種可見性）、匯入 ST JSON（可用 scratchpad 的 st-sample-worldinfo.json）、匯出、關鍵字觸發與 GM／角色資訊邊界實聊驗證。

## Next action
請使用者 npm run tauri dev 實測世界書 UI 與匯入，通過即結案。

## Constraints
worldbook.json 未知欄位原樣保留；無 worldbook.json 的舊桌行為完全不變；角色看不到 gm 條目與他人專屬條目；transcript 與 world.md 讀寫不動；實作一律外包 subagent，主線只做架構與審查。
