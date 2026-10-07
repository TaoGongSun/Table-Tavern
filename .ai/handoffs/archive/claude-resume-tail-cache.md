> 結案 2026-10-07：已進 main，Sol、Grok 驗收通過。

# claude 續聊每輪重寫對話尾端

## Summary
claude CLI `--resume` 續聊只命中 system 段，前幾輪對話每輪重寫；釘 1h 後寫入是 2 倍價。作者 2026-10-07 同意立案。

## 結論
成因有兩個：
- CLI 的 total_tokens 提醒只在 resume 時渲染，即時送出的 request 不帶，前後兩次前綴對不上。
- app 每輪抹機密段，作廢了唯一可重用的快取項。

處理方式：
- 所有 claude 呼叫都關掉提醒。
- claude 單角色線不抹，私設提進 system。以 `unerased_owner` 確保含未抹內容的 session 只給同一角色、單角色模式續用；換人、在場變多、狀態區塊整塊消失時一律開新線。
- 狀態區塊加上「以本區為準」標題。
- 對 session 檔做只讀偵測，看到提醒就記診斷。

查證、裁決、規格與實測數字見 [plans/claude-resume-tail-cache.md](../../plans/claude-resume-tail-cache.md)。開線後第 2 輪仍有一次不中，原因在 CLI 端。

## 未驗
- 卡重構線不和角色線混用：只有結構證據（`refactor_ai` 不經 `run_turn`、不讀寫 lanes.json），沒有假 CLI 測試。
