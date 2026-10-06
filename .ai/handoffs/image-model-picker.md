# 生圖來源與模型選單整理

## Summary
四項〔作者裁決 2026-10-06；第 4 項為 Sol 審查共識〕：生圖來源下拉不列 claude；設定頁生圖模型改 OpenRouter 官方生圖清單下拉；免費 key 生圖給專屬錯誤（實測玩家預設授權的 key 打 `/images` 回 402）；生圖輸出一律存成 PNG。

**不做原廠生圖 key**：持 OpenAI／Google 等原廠 key 的玩家走 OpenRouter BYOK，NovelAI 類吃到飽訂閱歸 [vn-cg-generation](../tasks/vn-cg-generation.md)〔作者裁決 2026-10-06〕。

## 現況
四項都已進 main（做法見 [plans/image-model-picker.md](../plans/image-model-picker.md)）：verify 10 步綠，測試通道看過下拉存讀、來源清單、免費 key 文案，Sol 驗收通過。

## 等實機驗收
排在[實測佇列](../reference/verification-queue.md)梯 2 第 21 項：
- 付費生圖模型回 JPEG／WebP／遠端 URL 時真打一次，確認圖進圖庫而且是 PNG。
- codex／agy／grok 真生圖一次，走新流程。
兩項都過就結案歸檔。
