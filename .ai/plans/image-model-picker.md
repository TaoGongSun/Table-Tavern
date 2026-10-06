# image-model-picker 做法（Sol 兩輪審過，2026-10-06 定稿）

## 免費 key 實測（2026-10-06，curl 直打官方 API）
- 受測 key：玩家實際拿到的那種——測試通道全新 root 按 app 內拿 key 連結、OpenRouter 授權頁照預設送出。`GET /key`：`is_free_tier=true`、`limit=null`；`/credits` 總儲值 0。
- `POST /images`（照 `generate_image` 格式、預設模型）回 **402**，`error.message`「Insufficient credits. This account never purchased credits. …」，`error.metadata.limit_source="openrouter_credits"`。
- 同一把 key 打標價全 0 的 `inclusionai/ming-image-0.1-design`：一樣回 402、同一句訊息，可見免費 key 連標價 0 的生圖模型也不能用。
- 另一把手動設過上限 0 的舊 key 回的是 403「Key limit exceeded」，連標價 0 的生圖模型也一樣。
- 結論：免費 key 不能生圖。現行 402 會落到 `errQuotaApi`（「可能額度不足或限流，稍後再試」），會誤導玩家，所以第 3 項要做。測試以玩家預設拿到的 key 為準，不另測其他 key〔作者裁決 2026-10-06〕。

## 1. 來源下拉不列 claude
- `CardImageDialogs.tsx`：`sourceOptions` 與 `detectedSources` 都先濾掉 `NO_IMAGE_CLIS`。存檔 `image_source="claude"` 因此不在可選清單，沿用現行 `fallback`：聊天來源能生圖就跟它，不能才退 API。
- 「選了才灰掉＋跳說明」整段刪掉：`sourceCannotGenerate`、按鈕的 disabled 條件、`aiGenSourceNoImage` 提示與十個語系的這個 key。

## 2. 生圖模型改下拉
- **抓取層：前端**，接既有模型清單機制（`model-catalog-store.ts`）。`CATALOG_SOURCES` 加一支 `"api-image"`，抓 `https://openrouter.ai/api/v1/models?output_modalities=image`（公開、免 key），解析沿用 `parseOpenRouterModels`；排除 `openrouter/auto*` 這類路由器項目（不是固定的生圖模型）。清單照官方名稱原樣列出，不標「免費」〔作者裁決 2026-10-06〕。
- **何時抓**：跟其他清單一起在開 app 時預熱（先擺 `model_catalog.json` 快取、背景重抓）；不另加刷新時機。
- **抓失敗／離線**：`mergeCatalog` 抓到空的就保留上次快取；連快取都沒有時，下拉只有「預設（`DEFAULT_IMAGE_MODEL`）」與「自訂…」。
- **UI**：照設定頁 CLI 分級模型那套 `<select>`＋`__custom__` 模式。選項依序為：預設（存空字串）、清單模型、自訂…（選了出現文字框）。存檔值不在清單內（舊存檔或清單沒抓到）就自動顯示成「自訂…」並把原值帶進文字框，不改值也不丟值。
- `SettingsForm.tsx` 已 909 行，這個欄位抽成 `features/settings/ImageModelField.tsx`。

## 3. 「免費 key 不能生圖」專屬錯誤
- **認法（Rust，`generate_image`）**：`/images` 回 402 或 403 時補查 `GET {base}/key`（不花額度），短逾時（送出與讀 body 合計）。只有 2xx 且 `data.is_free_tier` 是布林 `true` 才換成穩定碼 `AI_IMAGE_FREE_KEY:`，後面照樣附 `status=… body=…` 原文；逾時、非 2xx、JSON 壞掉、欄位缺失或型別不對，一律保留原本 `/images` 的錯誤。不比對 body 字樣。
- `/key` 走 `base_url(config)`，測試用 mock server 注入。改動收在 `generate_image` 與新函式，不碰 `http_error`。
- **前端**：`ai-error.ts` 在 `HTTP_STATUS` 前面認 `AI_IMAGE_FREE_KEY:` 開頭，回 `errImageFreeKey`。文案只講事實與選項：這把 OpenRouter key 沒儲值過，不能用 OpenRouter 生圖；可以到 OpenRouter 儲值，或把生圖來源換成能生圖的 CLI。
- 不加 `UiMsg`，只加 i18n key `errImageFreeKey`×10 語系。

## 4. 生圖輸出一律存成 PNG
- API 與 CLI 兩路拿到圖都先轉成位元組（data URL 解 base64、本機路徑讀檔、遠端 URL 下載），再走同一條正規化：依 magic bytes 判斷格式，不信回應宣稱的 MIME。PNG 原樣保留；JPEG／WebP 用 `image` crate（`default-features = false, features = ["png","jpeg","webp"]`）解碼後重編 PNG，限制尺寸與配置量，放 `spawn_blocking`。之後存圖庫、回傳 `data:image/png;base64,…`。
- 遠端 URL：不帶 OpenRouter bearer、要 2xx、設逾時、限制實際讀到的位元組上限。
- 錯誤碼：SVG／無法辨識 → `AI_IMAGE_UNSUPPORTED_FORMAT:`；支援格式但解碼失敗 → `AI_IMAGE_DECODE_FAILED:`（附解碼錯誤原文）；下載失敗 → `AI_IMAGE_DOWNLOAD_FAILED:`（附狀態或原因）。前端各一句專屬文案，十個語系。任一步失敗都不寫進圖庫。
- 驗收標準是「統一成 PNG 格式」，不保證是有效 PNG；CRC、zlib、APNG 嚴格驗證歸 image-save-strict-validate 案。
- 核心規則放新 domain 單檔 `src-tauri/src/generated_image.rs`，`commands/image.rs` 只接線。

## 測試與驗證
- Rust 第 3 項（`transport/client/tests.rs`，mock server）：402／403＋`is_free_tier:true` → `AI_IMAGE_FREE_KEY`；反例保留原 `AI_HTTP_STATUS_*`：`is_free_tier:false`、字串 `"true"`、欄位缺失、JSON 壞掉、`/key` 非 2xx、`/key` 逾時；`/images` 500 不打 `/key`。
- Rust 第 4 項（真實小圖 fixture）：PNG 原樣、JPEG／WebP 轉 PNG（含 WebP 透明度）、宣稱 MIME 與實際位元組不符、SVG 與損壞圖拒收、URL 下載後進圖庫並能重讀、下載非 2xx／逾時／超過上限；失敗時圖庫沒有新檔。
- 前端：`ai-error.test.ts` 新碼對應、body 帶字樣不翻盤；model-catalog 測試加 `api-image` 解析與排除 `openrouter/auto*`；`ImageModelField` 測試涵蓋預設、清單值、清單外舊值顯示成自訂且原值不變、清單抓回來時自訂輸入值不變；model-catalog-store 測試：已有舊快取時背景抓取失敗，重開後清單仍在；`CardEditor.test.tsx`（或圖片對話窗測試）確認選單沒有 claude、存檔 claude 退回 API。
- `npm run verify` 全綠。
- 測試通道（`scripts/harness.mjs`）：設定頁看生圖模型下拉與自訂欄、存檔後重開還在；角色卡 AI 生圖窗看來源清單沒有 claude。真生圖不在測試通道跑（route.rs 攔截 api-image）。
