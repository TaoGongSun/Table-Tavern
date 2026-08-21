# API 路快取看得見：多來源 usage 欄位＋「抓不到」與「沒中」分開顯示

Status: in-progress

## Summary
額度分頁對所有 API 呼叫顯示假的「命中率 0.0%」，2026-08-21 實測取證根因＝**讀錯欄位名**：app 讀 OpenRouter 的 `prompt_tokens_details.cached_tokens`，供應商實際回 DeepSeek 原生的 `prompt_cache_hit_tokens`，`unwrap_or(0)` 每次吞掉。包 A（多來源欄位 adapter＋Option 化）與包 B（兩軸資料模型＋報表顯示口徑＋十語系）已實作完成。規格與實測證據見 [.ai/plans/api-cache-visibility.md](../plans/api-cache-visibility.md)。

## Progress
- 包 A 完成：`extract_usage` 認三組欄位（OpenRouter／DeepSeek 原生／Anthropic 原生），讀寫分開；`cached_tokens`／`created_tokens` 改 `Option<u64>`，`hit_rate()` 回 `Option<f64>`；刪掉失效的 `include_usage` 死碼（OpenRouter 官方已將 `usage:{include:true}` 標為 deprecated 且無作用）。cli.rs 三個 parser 同步（grok 的 `created_tokens` 從 0 改 None——它不回報寫入數）。
- 包 B 完成：`usage_log` 落 `cache_reporting`（reported／absent），沒回報的數字欄位一個都不寫；`usage_report` 的命中率分母改成只算可判讀輪次，新增 `observed_rounds`／`observed_prompt_tokens`，舊行相容規則＝無 `cache_reporting` 欄位時 api 判 absent、其餘判 reported；`UsageTab` 命中率可出「—」與「（可判讀 N/M 輪）」；十語系新增 `usageHitObserved`／`usageCacheBlind`，`usageWhySingle` 改寫掉「不支援續聊／整包重新計費」的誤導。
- 自驗全綠：cargo 506（新增 2 個測試）／vitest 141／build／check:i18n 十語系。
- 真實 log 驗收（598 行／585 輪）：claude 各列命中率照舊有數字（opus-4-7 70.4%、opus-4-6 77.1%、sonnet-4-6 67.6%、opus-4-8 80.8%），api 那 24 輪從假的 0.0% 變成「可判讀 0／命中率 None」，agy 4 輪同為 None。

## Next action
Sol 驗收；之後真跑一輪 api 對話，確認新行帶 `cache_reporting: "reported"`＋`cached_tokens: 0`，額度分頁顯示 0.0% 而非「—」（tokenrouter 有回欄位、值是 0）。

## Constraints
保溫脫離 claude 不在本案：需按供應商 TTL 分策略，且 Anthropic 官方預熱手段 `max_tokens:0` 與 `stream:true` 互斥、要另開非串流路徑——等接上一條真有快取的線再做。理由見 plan 第 6 點。
