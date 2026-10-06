# 額度快取紀錄整理＋角色線抹寫丟線

Status: in-progress（分支 `usage-cache-audit`，未合併 main；四家通道都實跑〔作者裁決 2026-10-02〕）

規格、實跑對帳報告與驗收紀錄：[plans/usage-cache-audit.md](../plans/usage-cache-audit.md)。

## 現在成立的狀態

- **抹寫失敗落帳**（已施工、Sol 驗收過）：丟線事件帶 `stage`＋`detail`（遮路徑、截斷）。測試包另有 `usage-raw` 原始用量與抹寫失敗留證（只在測試包）。
- **角色線丟線已修**（Sol 驗收過）：
  - 根因：claude CLI 在 session 檔插入 attachment 行，舊驗證判失敗，每輪丟線。
  - 修法：attachment 納入 parentUuid 鏈（白名單）、節點 uuid 全檔唯一、不認得的帶 uuid 型別一律拒絕；未改動行逐位元組寫回；保溫改成記下原文、事後整份還原。
  - 測試通道實跑：角色線續聊、保溫還原都正常。
  - 保溫失敗路徑都有回歸測試：寫下半截就失敗、成功卻零／兩則保溫訊息、前段被改動。
- **對帳已跑 claude／codex／agy／OpenRouter**：結果見計畫檔「四、對帳結果」。
- **顯示第 2、5 項完成**（Sol 驗收通過）：讀 0 的原因只留給 claude 續聊線（agy／grok 不再誤標 skipped，舊帳本讀時濾除）、值為 0 顯示「這次沒有快取」、首輪「新桌／新線」，十語系；規格與實測見計畫檔「五、顯示施工」。
- **衍生問題由主線另立案**，本案只留證據：API 串流沒有停滯逾時、agy 零命中被標 `skipped`、claude CLI 改寫 1 小時快取（影響過期門檻、保溫、省下金額估算）。

## Next action

1. 2026-10-06 22:00 後補跑 grok（Grok 額度恢復前不送任何請求）。登入檔可複製進測試 root、跑完刪〔作者裁決 2026-10-06〕；步驟同計畫檔「三、實跑對帳」，結果補進「四」。
2. grok 跑完即結案：先合最新 main（long-prompt-scene-hint 也改 CLI 通道，可能衝突），壓成一筆再合併、push、刪分支（本地與遠端）。
3. 前端保溫節奏與 12 分鐘離開提醒不擋結案：測試通道驗不了，已排[實測佇列](../reference/verification-queue.md)梯 2 第 20 項；claude-1h-cache 若先停掉保溫，此項撤掉。

## Constraints

- 與 [non-claude-real-cache](../tasks/non-claude-real-cache.md)、[no-cache-model-optout](../tasks/no-cache-model-optout.md)、[vendor-prefix-floor](../tasks/vendor-prefix-floor.md)、[prompt-cache-optimization](prompt-cache-optimization.md)、[api-cache-visibility](archive/api-cache-visibility.md) 範圍相鄰，邊界表見計畫檔。
- 測試通道全機一把鎖，與其他子代理輪流用。
- 顯示只做「顯示拍板」第 2、5 項；第 1 項歸 vendor-prefix-floor、第 4 項歸 claude-1h-cache。
