> 結案 2026-10-07：已進 main，Sol、Grok 驗收通過。

# 額度快取紀錄整理＋角色線抹寫丟線

Status: done

規格、四家對帳報告與驗收紀錄：[plans/usage-cache-audit.md](../../plans/usage-cache-audit.md)。

## Summary
- **丟線落帳**：`drop-lane` 事件帶 `stage`＋`detail`（遮路徑、截斷），`transport` 記實際 provider；claude 與 grok 共用。grok 預定重開（換幕、改卡等）不記丟線。測試包另有 `usage-raw` 原始用量與抹寫失敗留證。
- **claude 角色線每輪丟線已修**：根因是 CLI 在 session 檔插入 attachment 行。attachment 納入 parentUuid 鏈（白名單）、uuid 全檔唯一、不認得的帶 uuid 型別拒絕；未改動行逐位元組寫回；保溫改記原文、事後整份還原，在每桌 lane_lock 內。
- **顯示第 2、5 項**：讀 0 的原因只留給 claude 續聊線、值為 0 顯示「這次沒有快取」、首輪「新桌／新線」，十語系。
- **四家對帳**（claude／codex／agy／OpenRouter 10/06，grok 10/07）：帳本、usage_report、額度分頁一致。

## 未驗
- Windows。
- 真實 grok 抹寫失敗的落帳（只有 cargo 假 grok 測試）。
- 前端保溫節奏與 12 分鐘離開提醒：測試通道驗不了，排[實測佇列](../../reference/verification-queue.md)梯 2 第 23 項；claude-1h-cache 若停掉保溫，此項撤掉。

## 不處理
- grok 換 session 自救：本次樣本下暫不做〔模型判斷·未裁決〕，重算規則與數據見計畫檔「四」。
- 衍生問題已另立案、本案只留證據：API 串流停滯逾時（api-stream-stall-timeout）、claude CLI 1 小時快取（claude-1h-cache）、CLI 自帶底線算成「已省」（vendor-prefix-floor，含 grok 角色線首輪讀 128 算成「有中」）。
