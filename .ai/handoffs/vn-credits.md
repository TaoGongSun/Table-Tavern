# vn-credits — VN 感謝名單（音訊素材署名）

Status: 已合 main（2026-11-07）、分支已刪；Sol 驗收 1 輪 DONE、截圖主線親看；等作者實機試玩（實測佇列梯 3）。規格與結果見 [plans/vn-credits.md](../plans/vn-credits.md)。

## 範圍〔作者裁決 2026-11-07〕
- 要署名的只有音訊：BGM（Incompetech／Kevin MacLeod，CC BY 4.0）與音效（Freesound 帳號，CC0／CC BY）。立繪、背景是玩家自己的 AI 生成，字型用系統字型，都不進名單。
- 完結畫面「達成 x／y」下方一到三行精簡署名，依來源各一行；不做捲動名單。
- 「美術一覽」改名「美術與音樂」，分頁立繪｜背景｜音樂；音樂頁列每首曲名、作者、授權、來源連結，就是完整名單，標題畫面與遊戲選單都進得去。不加標題鈕。
- 音效進作品包不在本案：`.ttvn` 是明文 zip，Freesound API 條款 4(a) 要 Content 只能從 Application 取得；作者裁決改做不透明容器後再帶音效，另立 [vn-pack-opaque](../tasks/vn-pack-opaque.md)。本案不加匯出提示。
- 資料來源：`vn/audio/audio.json`（kind、key、title、author、source_url、license），作品包 `assets/audio/audio.json` 已帶 BGM 筆，重播桌照樣顯示。後端不改。

案別：中案，核心估約 140 行〔模型判斷·未裁決〕；不動信任邊界，不送方案審，做完 Sol 驗收一次。

## 結果
核心約 185 行（`audio/credits.ts`、`gallery/MusicCredits.tsx`、完結畫面與面板分頁）、測試約 210、i18n 十語約 90；verify 17 步綠。音效進作品包交 [vn-pack-opaque](../tasks/vn-pack-opaque.md)。

## Next action
作者實機：標題→「美術與音樂」→音樂頁看署名與來源連結；完結畫面底下的一到三行署名。
