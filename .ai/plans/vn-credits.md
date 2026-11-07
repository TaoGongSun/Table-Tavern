# vn-credits 方案（中案，核心估約 140；不碰信任邊界、不送方案審，做完 Sol 驗收）

立案見[交接檔](../handoffs/vn-credits.md)。拍板條目標〔作者裁決 2026-11-07〕，其餘為主線設計〔模型判斷·未裁決〕。常識審由主線（Fable）直接做，摘要在 §6。

## 1. 資料（後端不改）
- `vn_audio_read` 已回 `{ files, entries }`，`entries` 就是 `audio.json` 的筆（kind、key、title、author、source_url、license），只有落地的檔才有筆。`useVnAudio.ts` 本地的 `AudioView` 搬進 `backend-contracts.ts`，補上 `entries: AudioEntry[]`。
- 新檔 `src/features/vn/audio/credits.ts`（純函式，可測）：
  - `licenseLabel(license)`：`https://creativecommons.org/licenses/by/4.0/`→`CC BY 4.0`、`Attribution`→`CC BY`、`Creative Commons 0`→`CC0`，其餘原字串。
  - `sourceHost(url)`：`new URL` 取 hostname 去 `www.`，解析失敗回空字串。
  - `creditLines(entries)`：依 kind 分 bgm、sfx 兩組，各組作者去重保序、主機去重、授權標籤去重；作者超過 8 位截到 8 位並附 `more`。沒筆的組不出。
- 讀取：各畫面掛載時 `invoke("vn_audio_read")` 一次，不監聽 `vn-audio-status`（開著時剛落地的筆重開就有）。

## 2. 完結畫面精簡署名〔作者裁決〕
`EndingScreen.tsx` 在「達成 x／y」之下、按鈕之上加一段 `.vn-credits-lines`，每組一行，走 i18n 模板：
- `vnCreditsLineMusic`：「音樂　{authors}（{hosts}）· {licenses}」
- `vnCreditsLineSfx`：「音效　{authors}（{hosts}）· {licenses}」
- 作者以 `vnCreditsJoin`（「、」，en ", "）串接，超過 8 位加 `vnCreditsMore`「等 {n} 人」。
沒有任何筆就不出這段。不做捲動名單〔作者裁決〕。

## 3. 「美術與音樂」面板的音樂頁〔作者裁決〕
- `vnGallery` 十語改「美術與音樂」（en "Art & music"），分頁加第三個 `music`（`vnGalleryTabMusic`「音樂」）。
- 新檔 `src/features/vn/gallery/MusicCredits.tsx`：兩段小標（`vnCreditsMusic`「音樂」、`vnCreditsSfx`「音效」，該組沒筆就不出），每筆一列：曲名（粗體）、作者、授權標籤、「來源」連結鈕（`className="link"`、`openUrl(source_url)`，只在 `source_url` 以 `https://` 開頭時出；照 `SettingsForm.tsx` 的 Freesound 連結寫法）。全部沒筆：`vnCreditsEmpty`「還沒有音樂：故事走到的場景會自動下載。」
- 列樣式沿用 `.vn-gallery-row`，字段排版補 `.vn-credits-row`；CSS 進 `vn-gallery.css`。
- 不做試聽／播放：標題畫面主題曲已在播，v1 只列不播，列已知限制。

## 4. 匯出提示：不做〔作者裁決 2026-11-07〕
音效進作品包改由 [vn-pack-opaque](../tasks/vn-pack-opaque.md)（不透明容器）處理，提示做了就丟，本案不加；`ExportWorkDialog.tsx` 與後端匯出不動。

## 5. 驗收
- `credits.test.ts`：三個純函式，含去重、截 8 位、壞 URL。
- `EndingScreen` 測試：有 bgm＋sfx 筆出兩行、沒筆不出。
- `ArtGallery.test.tsx`：音樂分頁出現、空狀態、來源鈕呼叫 `openUrl`（mock `@tauri-apps/plugin-opener`）。
- `npm run verify` 17 步綠；測試通道截圖：標題→美術與音樂→音樂頁（harness 桌已有主題曲落地）；完結畫面署名能到就拍、到不了以 vitest 為準。
- i18n 新 key 十語全補：vnGalleryTabMusic、vnCreditsMusic、vnCreditsSfx、vnCreditsLineMusic、vnCreditsLineSfx、vnCreditsJoin、vnCreditsMore、vnCreditsEmpty、vnCreditsSource；`vnGallery` 改文。

## 6. 常識審（主線 Fable）
- 名單只有音訊，三行字做整頁 staff roll 很怪 → 完結字卡一到三行＋面板完整頁，不加標題鈕（已七顆）〔作者裁決〕。
- 「一覽」看不懂 → 「美術與音樂」〔作者裁決〕。
- 來源連結開外部瀏覽器，與設定頁一致。

## 結果（2026-11-07）
Grok 施工 1 包；Sol 驗收 1 輪無必改、採納 `unique()` 一行化；Sonnet 截圖 3 張主線親看（標題鈕、三分頁、音樂頁一筆署名）；verify 17 步綠（vitest 878／221、cargo 2732）。

## 已知限制
- 音樂頁不播放，不是音樂室。
- 名單不即時：面板開著時剛落地的筆要重開才看到。
- 音效暫不隨作品包，交 vn-pack-opaque；在那之前重播桌名單只有音樂。

## 觀察
- Freesound API 條款 1(b) 只准「必要的中間副本、用完即刪」，vn-audio 的每桌持久快取也在灰色地帶；授權本身（CC0／CC BY）無此限制。屬 vn-audio 範圍，記在這裡供日後評估。
