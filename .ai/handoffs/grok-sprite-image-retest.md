# grok-sprite-image-retest — Grok 加入 VN 立繪來源

狀態：已合 main（2026-11-14），等作者實機。方案、實測數字與已知限制見 [plans/grok-sprite-image-retest.md](../plans/grok-sprite-image-retest.md)。

案別：大案（立案估 0 行只實測；重測通過後接進立繪來源，核心約 300 行、測試約 270 行）。Sol 方案審 1 輪、驗收 1 輪，皆無必改；Fable 常識審由主線兼，截圖主線親看。

## 現況

- 立繪來源選單多一項 Grok CLI：灰底定稿 → 內建去背 → 臉框（頂端正方形＋刻度格線）→ 裁頭改表情 → 羽化貼回；有卡圖的角色先把卡圖貼到 2:3 灰畫布。
- 表情對齊門檻改成 2 px 內平移補正（codex／agy 共用，只會少重骰）。
- Grok 生圖（含既有角色卡生圖）只留兩個生圖工具、效能固定 medium；Grok 測過的版本改成 1.0.77。

## 下一步

- 作者實機：用 Grok 當來源開一桌 VN，看定稿、四表情與換裝（排在[實測佇列](../reference/verification-queue.md)）。Grok 訂閱 2026-11-20 到期，之後這條來源無法再回歸測試。
- 沒實測的範圍：人臉／動漫臉的表情改圖、Windows、背景生圖。
