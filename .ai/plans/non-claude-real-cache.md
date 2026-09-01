# non-claude-real-cache — 三條無續聊路的快取

## 根因（2026-08-22 實證定案）

**`stealth/ox-alpha`（OpenRouter）只在「上一次請求是這一次的逐字前綴」時給快取。**
app 每輪把尾端動態塊（世界書 keyword 條目、目前狀態、導演指示）換成新的，上一輪的請求就不再是這一輪的前綴，於是全滅——29 筆只有 2 筆真中，其餘停在供應商白送的 64。

決定性對照（真實桌 body 重放，兩組皆用未打過的 events 區段、間隔皆 60 秒、中間靜置 300 秒）：

```
組 A（app 現形態：尾端動態塊逐輪換掉）   A1 in=7453 cached=64 → A2 in=8124 cached=  64  (1%)
組 B（嚴格前綴：拿掉 A1 尾端那則）        B1 in=9380 cached=64 → B2 in=9522 cached=8000 (84%)
```

佐證：同一份 body 反覆打會中且能活 9 分鐘以上（64 → 8576 → 11264）；換一份、只是共享 10200 tokens 前綴的，一個都拿不到。

claude 線不受影響：Anthropic 走 block 級快取＋顯式 `cache_control` 斷點，任意前綴都能匹配。
`prompt-cache-optimization` 當初把動態塊移到尾端「保前綴」，對這條線正好是致命的一步——system 保住了，但每輪的**請求結尾**都不一樣。

措辭上只宣稱「此端點呈現嚴格延續行為」，不斷言它內部只存上一次請求（Sol 指出）。

## 證偽清單（都實測過，別再走回頭路）

| 假說 | 反證 |
|---|---|
| 缺 `session_id`／sticky routing | 帶與不帶各五輪，30／60 秒都中 99%，無差別 |
| 請求形狀、串流、輪次間隔 | 真實形狀（追加 1087 tokens、`stream:true`、134 秒間隔）六輪全中 |
| 前綴被打散 | 離線重算相鄰兩輪 byte-LCP 97.8–99.0%，斷點每次都落在動態塊之前 |
| 傳輸層（reqwest 標頭／client） | 真實 body 換 python 原樣重放，一樣只有 64 |
| 單輪長輸出 | 僅 >4000 tokens 會讓下一輪掉光，只解釋得了 29 筆裡的 2 筆 |

## 拍板（Sol 三輪討論定案）

**動態塊併入歷史，讓輪 n 的請求成為輪 n+1 的逐字前綴。**

1. **尾巴重播**：`TranscriptEvent` 加選填欄位存「上一輪實際送出的完整尾巴」（含導演指示，不只 `gm_dynamic_block`），
   下一輪組裝時先重播尾巴、再放該則回覆。不新增看得見的 System 事件——undo／restore／revert 因此天然一起回滾。
2. **標記法**：每輪尾巴包成 `<turn-context seq="N" valid-for="next-response">`，內分 `<active-lore>`／`<table-state>`／`<director-instruction>`；
   system 只寫一次規則「多個 turn-context 依時間排列，只有最後一個有效，先前區塊不得覆蓋最後區塊」。判定依「最後一個」，不要求模型比較序號。
3. **舊塊不改寫**（一改寫就破壞前綴）。用 **chain epoch** 控制累積：換幕、角色卡／世界書／mechanism／語言／模型變更、undo／restore 時重開一次，接受一輪冷啟動。
4. **不做自動探針**（連兩次 miss 會誤判 TTL、短 prompt、不支援快取）。改用明確的 `CacheStrategy::{NormalPrefix, StrictExtension}`，
   預設 `NormalPrefix`，目前只把「OpenRouter ＋ `stealth/ox-alpha`」列為已驗證 quirk，帳本記 strategy。日後有同型證據再擴成 registry。
5. **套用範圍＝`PromptShape::Turn` ＋ `StrictExtension`**：GM 線與角色線都要改（不是只有 GM）；
   翻譯、換幕摘要、卡重構、生圖等 oneshot 一律排除；HTTP API 裡的 Anthropic 系維持現行 `cache_control` 路線；claude／grok lane 不動。

風險：歷史累積多份過期狀態，數值、在場與位置最容易被舊值污染（Sol 評估中等，隨鏈長放大）——這是 epoch 上限存在的理由。
世界書 keyword 條目留在各輪 `<active-lore>` 裡，只有最後一輪清單 active；不改變「觸發過就永久有效」以外的既有契約。

## 分包

- **包 1**：`CacheStrategy` 判定（端點＋模型）＋帳本欄位。
- **包 2**：尾巴重播——`TranscriptEvent` 新欄位、GM 線與角色線組裝改寫、`<turn-context>` 包裝與 system 規則、十語系文案。
- **包 3**：chain epoch 的重開條件與冷啟動一輪。

## 驗收

1. 離線重算：`StrictExtension` 下相鄰兩輪的 byte-LCP ＝ 100%（輪 n 是輪 n+1 的逐字前綴）。
2. 實跑三輪以上：第二輪起 `cached_tokens` ≥ 上一輪 `prompt_tokens` 的 80%（扣掉底線 64）。
3. `cargo test`／`vitest`／`npm run build`／`npm run check:i18n` 全綠。
4. Sol 驗收。

## 其餘三條路

codex 與 agy 都有續聊旗標（`codex exec resume <id> / --last`、`agy --conversation <ID> / --continue`），
動手前先照本案的方法量一次：它們的上游是不是也只認逐字前綴。
