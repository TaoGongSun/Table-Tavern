> 結案 2026-10-06：GM 線與角色線實機驗收都已通過，使用者裁決直接結案〔作者裁決 2026-10-06〕。

# grok 改走續聊（grok-cache-miss）

Status: done

## Summary
grok 通道原本每輪開新 session、跨呼叫拿不到 prompt cache（根因：xAI 靠 `prompt_cache_key` 分流，grok CLI 新 session 就是隨機路由）。現在改走 lane 續聊：`-s` 開線、`-r` 續聊、只送增量。GM 是 `gm:<模型>`，角色一角一線 `chars:<模型>:<角色 id>`，私設提進該角色自己的凍結 system（目前未生效，見已知限制）；素材一漂移就整線重開。根因、實驗與拍板見 [plans/grok-cache-miss.md](../../plans/grok-cache-miss.md)。

## 驗收（prompt／cached tokens）
- GM 線（2026-08-22）：連四輪 8915→11560，第 3、4 輪命中 92–93%，每輪只長約 900。
- 角色線（2026-10-06，測試通道、迷霧酒館、grok-4.6）：
  - 連續接話：狐狸開線 8444 → 第 3、4 輪 8489／7936、8968／8448（93–94%），每輪只長約 450。
  - 換角色：騎士只開線一次，第 2 輪 92.5%；回到狐狸直接續聊 87.9%。
  - 改卡：受影響的線各重開一次（system-changed），下一輪 94.5%。
  - 換幕：各線各重開一次（scene-changed），騎士之後 92.4%／82.5%。
  - 私設：沒有角色輸出含別人的私設，私設事件都標 gm_only。

## 已知限制
- 偶發整條線不命中：狐狸線換幕重開後連四輪續聊只有 1–10%。前綴純追加、每輪一個 loop，原因在 xAI 伺服器端分流。等 usage-cache-audit 用更多資料判斷要不要換 session 自救〔作者裁決 2026-10-06〕。
- 凍結 system（含提進去的私設）在 grok 1.0.46＋app 獨立 GROK_HOME 下實際沒生效，另案 [grok-system-override](grok-system-override.md) 處理；本案驗的快取續聊行為不受影響。
