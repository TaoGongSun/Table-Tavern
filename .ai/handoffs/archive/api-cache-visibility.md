> 結案 2026-10-02：額度分頁實機過——api 列真實命中率、有回報零命中顯示 0.0%、未回報舊列「—」，數字與帳本手算一致。

# API 路快取看得見：多來源 usage 欄位＋「抓不到」與「沒中」分開顯示

Status: done

## Summary
額度分頁對所有 API 呼叫顯示假的「命中率 0.0%」，2026-08-21 實測取證根因＝**讀錯欄位名**：app 讀 OpenRouter 的 `prompt_tokens_details.cached_tokens`，供應商實際回 DeepSeek 原生的 `prompt_cache_hit_tokens`，`unwrap_or(0)` 每次吞掉。包 A（多來源欄位 adapter＋Option 化）與包 B（兩軸資料模型＋報表顯示口徑＋十語系）實作完成並通過 Sol 兩輪驗收。規格與實測證據見 [.ai/plans/api-cache-visibility.md](../../plans/api-cache-visibility.md)。

## Progress
- 包 A／B 實作完成（commit `b8318c9`）：`extract_usage` 認三組欄位、讀寫分開；`cached_tokens`／`created_tokens` 改 `Option<u64>`；`usage_log` 落 `cache_reporting`；命中率分母只算可判讀輪次；`UsageTab` 出「—」與「（可判讀 N/M 輪）」；十語系新增 `usageHitObserved`／`usageCacheBlind`／`usageLatestBlind`。順帶刪掉失效的 `include_usage` 死碼。
- Sol 驗收抓到四個洞，全數修掉（commit `df2f9e0`）：①首頁進度條把「量不到」畫成「全額 100%」②最近一輪只講「單發」沒講量不到③舊 api 行一律判 absent 會誤殺真 OpenRouter 的歷史正命中（改成 `cached_tokens > 0` 採信）④`cache_tokens()` 整組提前返回會讓寫入數遮蔽另一組的讀取數。
- Sol 複驗通過（DONE），補完兩個小項（commit 第三筆）：混合 schema 讀寫選源測試、說明句去重。
- 自驗全綠：cargo 506／vitest 141／build／check:i18n 十語系。真實 log（598 行／585 輪）驗收：claude 各列命中率與改動前逐位相同，api 那 24 輪從假的 0.0% 變成「可判讀 0／命中率 —」。

## Constraints
保溫脫離 claude 不在本案：需按供應商 TTL 分策略（Anthropic 5 分鐘值得 ping、DeepSeek 數小時不必、Gemini 隱式快取讀取不延長壽命故 ping 無效），且 Anthropic 官方預熱手段 `max_tokens:0` 與 `stream:true` 互斥、要另開非串流路徑。理由見 plan 第 6 點。
