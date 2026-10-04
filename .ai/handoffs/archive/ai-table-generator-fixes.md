> 結案 2026-10-04：Sol 驗收無必改，主線 verify 10 步全過，已壓成一筆進 main；[ai-table-generator](ai-table-generator.md) 一併結案。

# 一句話開桌修三處（ai-table-generator-fixes）

首發前必做〔作者裁決 2026-10-04〕。來源：[ai-table-generator](ai-table-generator.md) 2026-10-04 代測。

## 定案做法
- A 改大綱白屏：`GenerateTableDialog.tsx` 大綱四欄 onChange 先取 `event.currentTarget.value` 再進 updater（updater 延到 render 才跑，那時 currentTarget 已是 null）。
- B 桌名照草稿：桌名只認後端收到、trim 過的草稿標題。`generate_table_expand` 多收 `title`，空白回 `EMPTY_TITLE`，擋在呼叫模型與寫檔前；`genesis::draft_title` 判定，`materialize(root, title, expanded)` 的桌名與重名補號都用它。`parse_expand` 接受空 WORLD 標題（世界內容空白仍拒絕），`parse_outline` 仍要求標題；展開提示預填標題只是輔助。
- C 錯誤走人話：對話窗改用共用 `AiErrorText`，transport 由 App 經 AppDialogs 傳入，外層 section 保留 role="alert"。`ai-error.ts` 的 `redactAiErrorDetail` 用正則遮 `"user_id"` 值（不解析 JSON、容忍截斷與跳脫引號），`AiErrorText` 兩條顯示路徑都套用，錯誤列與 TurnFailedDialog 也一起不露。

## 驗證
- `GenerateTableDialog.test.tsx`（StrictMode、逐鍵輸入，修前 3 紅：A 拋 `Cannot read properties of null (reading 'value')`）、遮罩 7 例、AiErrorText 命中／未命中、`genesis/tests.rs` 新增 5 例；`npm run verify` 10 步全過。
- 測試通道：gemma 被上游 429 正好實證 C（人話一句、原文一次、user_id 已遮）；dots `:free` 跑六項全過。重骰與介面切 en 相關程式未改、本輪未重跑。
