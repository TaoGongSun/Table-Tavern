# vn-mode-image-spike — VN 立繪／CG 生圖實測（2026-10-27）

[vn-mode](../tasks/vn-mode.md) 前置實測 a／b／c 的結果。原創角色 Mira Ashwell，三家（Codex／agy／Grok）同一份提示詞與規格塊，全走 CLI 訂閱、付費 API 0 張，共 20 張（Codex 8、agy 5、Grok 7，含代理自行重試）。比較圖（三列＝Codex／agy／Grok；欄＝定稿／喜／怒／透明或去背／CG）不入版控，在本機 `/private/tmp/claude-501/-Users-<user>--claude-spares-spare-yfWgps/853b218d-0b3d-4f00-b3b6-0b81bbbd6996/scratchpad/vn-image-spike/comparison.png`（暫存目錄，可能被清掉）。

## 結論
- **a 去背**：可行。rembg 2.0.86＋官方權重。birefnet-general（928MB，12–19 秒）最乾淨；birefnet-general-lite（224MB，5–6 秒）接近，建議 app 內建（Rust 用 ort）；isnet-anime（0.9 秒）邊緣軟、髮尾滲底色。授權：BiRefNet MIT、isnet-anime Apache-2.0（實作前再核）；BRIA RMBG-2.0 不可商用。
- **b 原生透明**：只有 Codex 出真透明（RGBA）；透明定稿改表情仍透明，alpha IoU 0.992。agy 只出 JPG；Grok 自畫洋紅底再摳。
- **b 表情差分**（整張重繪）：脖子以下變動像素 agy 0.1%、Codex 0.5%、Grok 1.1%；品質 agy≈Codex>Grok（Grok 怒臉過度）。局部重繪三家 CLI 都不支援，以此漂移量用不到。
- **c CG**（定稿參考圖＋規格塊）：Codex、agy 通過（臉髮瞳色與服裝大項對上）；Grok 不通過（先生空場景再貼人、像拼貼、裙長跑掉）。
- **NAI**（僅官方文件，待作者 token）：`POST image.novelai.net/ai/generate-image` 預設回 zip、`Accept: application/json` 回 base64；Director Tools 走 `/ai/augment-image`（含 Emotion、Remove Background）。透明只有 V5，角色參考只有 V4.5（每張加扣 5 Anlas），無單一模型兼得；Opus 免費限 V4.5 以下且不帶參考圖。model ID 與 req_type 字串要實打確認。

## 定案管線〔作者裁決 2026-10-27〕
- v2 立繪：拍板 7 主路改為原生透明——Codex 生透明定稿→玩家重骰挑選→逐張編輯 4–5 張表情差分；非透明來源走素色底＋BiRefNet-lite 去背當後備。agy 為第二來源。
- v3 CG：與該桌定稿同一來源，優先 Codex；Grok 可當 CG 來源但排在 Codex 之後。定稿＋規格塊、玩家重骰把關。
- NAI 暫不測、不納入管線。

## 定案前還缺
2. app 提示詞要明令「只呼叫一次生圖工具、不得自行後製」（Grok 會自貼臉拼圖、Codex 會自重試）。
3. 權限做法：agy 不加 skip 旗標也能出圖（叫它的 image-generator 子代理並等做完），可取代 app 現行加旗標做法。
4. 測試環境與 app 不同（本機預設 HOME、Codex 未加 `--ignore-user-config`），app 隔離執行時行為可能不同。
5. 三家都會左右畫反，規格塊避免左右描述。
6. 樣本量：多角色、5 種表情、雙人 CG 各跑幾輪看重骰率。
7. Codex 透明定稿放進 v1 UI 主題實貼看觀感。

## 業界做法與差距
業界立繪是分層 PSD（身體共用、五官分層），差分＝換臉部圖層，引擎疊圖（Ren'Py LayeredImage、Live2D）；CG 由原畫依設定資料表畫。AI 圈標準是本地 SD＋角色 LoRA＋ControlNet／IP-Adapter，表情局部重繪，去背用 BiRefNet。我們差在：差分整張重繪（做不了眨眼、口型圖層動畫）；一致性靠雲端參考圖＋規格塊而非 LoRA，大量 CG 的長期一致性未驗；只走雲端 BYOK 是刻意取捨（玩家門檻低）；規格塊方向與設定資料表一致。
