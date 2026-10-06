> 結案 2026-10-07：已進 main，Sol、Grok 驗收通過。

# grok 角色改走共線

Status: done

## Summary
grok 角色跟 claude 一樣共用 `chars:<模型>`：私設、限定條目與角色狀態走回合機密段，回合後由 `lanes/grok_session.rs` 抹掉 session 目錄 `chat_history.jsonl` 與 `updates.jsonl` 裡的機密段，拿掉該輪 reasoning，並補上名字前綴。形狀不符、壓縮、鎖逾時、有殘留時就撤銷整條線（store 先移除再刪目錄），下輪重開。同桌的 lane 呼叫與保溫以每桌 mutex 串行。Agy 維持一角一線。實驗、設計與驗收見 [plans/grok-shared-lane.md](../../plans/grok-shared-lane.md)。

## 未驗
- Windows：鎖 handle、刪目錄、覆蓋式 rename。
- 真實的自動壓縮（只驗了手動 `/compact`；壓縮後一律丟線）。

## 已知限制
- 回覆內文再出現「名字：」時會丟線，只損快取。

## 不處理〔模型判斷·未裁決〕
- 舊的 `chars:<模型>:<角色 id>` 鍵留在 lanes.json，不清理。
- `prompt_history.jsonl` 留有私設原文：不進 `-r` 上下文，不處理。
