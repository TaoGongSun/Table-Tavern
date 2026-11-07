# room-scene-capacity — 幕容量提醒

Status: 已合併 main（2026-11-06；Sol 方案審 1 輪＋驗收 1 輪 PASS、截圖主線親看；Grok 寫碼、Sonnet 截圖、主線設計與總審〔作者 2026-11-06 本輪指示〕）
Created: 2026-11-06
案別：小案偏中，核心估約 90 行（執行端 Rust 40、伺服器 20、客戶端 30）。規格見 [plans/room-scene-capacity.md](../plans/room-scene-capacity.md)。

## Summary
房間沒有單機版的換幕前容量提示，房主要等 GM 失敗才知道該換幕。執行端在 `gen.done`、換幕完成、連上時用既有 `scene_budget::compute` 算「到提醒門檻了沒」送 `capacity { hint }`；伺服器蓋上房間幕號存起來、投影成 `ExecutorView.capacityHint`；房主回合列在換幕鈕旁出一行提醒。

## 下一步
- 伺服器已部署、桌面版已隨 v0.5.2 發布（2026-11-07）。
- 作者實機看：真的玩到快滿時回合列提醒有沒有出現、換幕後消失。排在實測佇列。
