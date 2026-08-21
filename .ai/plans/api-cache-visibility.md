# api-cache-visibility — API 路快取看得見

## 根因（2026-08-21 實測取證）

額度分頁對 API 路一律顯示「命中率 0.0%」，實測發現是**讀錯欄位名**，不是供應商不回報。

直接打 `https://api.tokenrouter.com/v1`（使用者當前 base_url，模型 `deepseek/deepseek-v4-pro-0813-free`）拿到的串流尾塊：

```json
"usage": {"prompt_tokens": 2495, "completion_tokens": 16,
          "prompt_cache_hit_tokens": 0, "prompt_cache_miss_tokens": 2495}
```

回的是 DeepSeek 原生欄位名，`prompt_tokens_details` 那層**不存在**；`extract_usage()` 只讀 OpenRouter 的 `prompt_tokens_details.cached_tokens`，`unwrap_or(0)` 每次吞掉。

同一份實測另兩項發現：
- 該站後端是自架 vLLM（`system_fingerprint: vllm-0.27.1-tp8-ep-43f8f3d6`），非 DeepSeek 官方 API；逐字相同的 2495-token 前綴連打兩次仍 `hit=0`，**這條線真的沒有快取命中**（讀對欄位後也一樣是 0）。
- OpenRouter 官方文件已把 `usage:{include:true}` 與 `stream_options:{include_usage:true}` 標為 deprecated 且無作用，完整 usage 一律自動回報 → `include_usage` 那段判斷是死碼。

## 拍板結論

**1. 欄位認四組**（有哪組抓哪組，讀寫分開）

| 來源 | 讀 | 寫 |
|---|---|---|
| OpenRouter | `prompt_tokens_details.cached_tokens` | `prompt_tokens_details.cache_write_tokens` |
| DeepSeek 原生（含 tokenrouter） | `prompt_cache_hit_tokens` | 無（有 `prompt_cache_miss_tokens` 佐證欄位存在） |
| Anthropic 原生 | `cache_read_input_tokens` | `cache_creation_input_tokens` |

不認 Gemini 原生欄位：agy 走 CLI 路徑且完全不回報用量，經 OpenAI 相容代理時回的也是 OpenAI schema，寫了是死碼。

**2. `cached_tokens`／`created_tokens` 改 `Option<u64>`**：「欄位不存在」與「欄位是 0」必須分開——前者是量不到，後者是量到了沒中。`unwrap_or(0)` 把兩者壓平正是本案根因。

**3. 舊 log 相容規則**：沒有 `cache_reporting` 欄位的舊行，`transport == "api"` 判 unknown（正是讀錯欄位那條路），其餘判 reported（CLI 路徑的欄位一直是對的）。不用「cached>0 才算 reported」——那會把 claude 真實的 warmup／expired 零命中輪誤判成量不到。

**4. 顯示口徑**：命中率的分母只算可判讀輪次，未知輪次照計輸入輸出與花費但不稀釋命中率。
- 全部可判讀 → `命中率 42.1%`
- 混合 → `命中率 42.1%（可判讀 7/10 輪）`
- 全部未知 → `命中率 —`＋一句「這條連線未回報快取資料」

**5. `single` 語意收窄**：原文案「這次是單發呼叫（換幕摘要、開桌生成，或這條連線本來就不支援續聊），整包重新計費」把「呼叫用途」與「快取可觀測性」混成一句。兩者正交——真單發也可能命中共用前綴，續聊也可能抓不到欄位。`single` 只保留「單發呼叫」語意，可觀測性由 `cache_reporting` 獨立表達。

**6. 保溫不在本案**：本案只做量測與顯示。保溫脫離 claude 需要按供應商 TTL 分策略（Anthropic 5 分鐘值得 ping、DeepSeek 數小時不必、Gemini 隱式快取讀取不延長壽命所以 ping 無效），且 Anthropic 官方預熱手段 `max_tokens:0` 與 `stream:true` 互斥、需另開非串流路徑——等接上一條真有快取的線再做。

## 分包

- **包 A**：`extract_usage` 多來源 adapter；`PromptCacheUsage` 的讀寫欄位 Option 化；刪 `include_usage` 死碼。連帶 `cli.rs`（claude／codex／grok 共用同一個結構）。
- **包 B**：`usage_log` 落 `cache_reporting`；`usage_report` 分母改可判讀輪次、新增 `observed_rounds`；`UsageTab` 顯示口徑；十語系文案。

## 驗收

1. `cargo test`／`vitest`／`npm run build`／`npm run check:i18n` 全綠。
2. 舊 `prompt-cache.jsonl`（598 筆）讀進報表：claude 那 569 筆命中率與改動前一致，api 那 24 筆從「0.0%」變成「—」。
3. 真跑一輪 api 對話：新行帶 `cache_reporting`，tokenrouter 應為 `reported` 且 `cached_tokens: 0`（欄位在、值是 0），額度分頁顯示 `命中率 0.0%`。
4. Sol 驗收。
