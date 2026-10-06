# 免費層不顯示生圖模型選單

## Summary
2026-10-06 自 image-model-picker 衍生。玩家照預設授權拿到的 OpenRouter 免費 key 不能生圖（實測 402），設定頁卻顯示 60 項生圖模型下拉，沒有意義。拍板〔作者裁決 2026-10-06〕：
- 依帳號是否免費層決定顯示：沿用 image-model-picker 的 `GET /key` 補查（`is_free_tier`，不花額度）。確定是免費層就不顯示生圖模型選單；不是免費層或查不到（離線、逾時）一律顯示。不以「玩家換過 key」判斷，因為同一把 key 儲值後就能生圖。
- 開設定頁或換 key 時查一次並暫存；玩家儲值後下次開設定頁就會出現。
- 清單不縮短、不人工挑模型，60 項照列。
- 生圖視窗的「API」來源維持現狀，免費 key 照樣列出，按下去顯示專屬提示。

## 現況
已進 main（做法見 [plans/image-model-free-tier-hide.md](../plans/image-model-free-tier-hide.md)）：verify 10 步綠；測試通道接本機假 `/key` 看過 free 藏、paid／404 顯示、改草稿 base 不查、藏著存檔 `image_model` 原值保留、快取首屏即藏、換 key 立即恢復顯示；Sol 驗收通過。

## 等實機驗收
排在[實測佇列](../reference/verification-queue.md)梯 2 第 22 項：真免費／付費 key 各看一次設定頁；整個關掉設定視窗再開確認首屏不閃。都過就結案歸檔。
