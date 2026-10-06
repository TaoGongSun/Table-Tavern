# 免費層不顯示生圖模型選單：做法

已定〔作者裁決 2026-10-06〕：依 `is_free_tier` 判斷，不看換沒換 key；查不到照常顯示；清單不縮短；生圖視窗「API」來源維持現狀。

## Rust
- `transport/client.rs`：`key_tier(base, key) -> Option<bool>`（`Some(true)` 免費、`Some(false)` 非免費、`None` 不確定），判定規則沿用前案：只有 2xx 且 `data.is_free_tier` 是布林才有值；逾時（整段含讀 body，5 秒）、連不上、非 2xx、JSON 壞、欄位缺或 null、型別錯都是 `None`。建 client 放在逾時外（首次載入憑證不吃逾時）。`generate_image` 以 `== Some(true)` 判免費，行為不變。`key_tier_for(config, key)` 讀已存檔 config 的 `base_url(config)`，key trim 後空白不發請求。不碰 `http_error`、`stream_chat`。
- 新檔 `commands/openrouter_key.rs`：`openrouter_key_tier(api_key) -> { tier: "free"|"paid"|"unknown", base }`，base 是 Rust 實際打的（正規化只在 Rust）。只收草稿 key，不收前端 base〔作者裁決 2026-10-06〕。`commands/mod.rs` 宣告、`lib.rs` 登記並歸入 NOT_WORLD。

## 前端〔作者裁決 2026-10-06；細節為 Sol 審查共識〕
- 新檔 `settings/key-tier.ts` 的 `useKeyTier(draftKey, transport, savedBaseRaw)`，`SettingsForm` 只接結果。
- 查詢身分＝（trim 後草稿 key、存檔 `preferences.base_url` 原值）。開頁與存檔 base 變動立即查；改草稿 key 等 500ms。改草稿 base 不查；CLI、key 空白不查。
- 模組快取 `{ key, savedBaseRaw, base, tier }`：開頁時身分相符就同步當初始值（首屏即可藏），背景照樣重查。
- 作廢：換 key、清空 key、切 CLI、存檔 base 改變、unmount 都清 timer 並標 stale；換身分立刻停用舊 tier（不等 debounce）；stale 的查詢回來不動 UI 也不動快取；失敗時清掉該身分的快取並當 unknown。
- 只有 `free` 才藏 `ImageModelField`。藏著時存檔明確略過 `image_model` 的 patch 與 dirty 計數，草稿 state 保留，欄位重新出現才恢復比對（防「查詢中改模型→回 free 被藏→存其他設定」把改動寫進去）。

## 測試
- Rust（`client/tests.rs` mock server）：free／paid；欄位缺、null、`data` 為 null、字串 "true"、壞 JSON、404、延遲逾時、連線失敗、標頭已回 body 停住（整段逾時 <5 秒）都 `None`；空白 key 不發請求、存檔 base 正規化。既有 `generate_image` 測試照綠。
- 前端 `SettingsForm.free-tier.test.tsx`（fake timer＋controlled promise）：free 藏、查詢中／paid／unknown／失敗顯示；CLI 與空 key 不查；改 key 立刻停用舊結果、500ms 才重查、舊結果晚回丟掉；改草稿 base 不重查；快取命中首屏即藏、存檔 base 變立即作廢重查、換 key 不命中；舊頁關→新頁 paid→舊頁 free 晚回不影響畫面與快取；同 key 由 free 變 paid；快取 free 重查 unknown／失敗恢復顯示；查詢中改模型→回 free→存其他設定不寫 `image_model`、欄位回來後草稿還在。
- `npm run verify` 全綠。
- 測試通道：存檔 base 指向本機 mock（`/key` 回 free／paid／404），看欄位藏／顯示、藏著存檔後 `image_model` 原值還在。
