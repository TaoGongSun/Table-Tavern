# no-cache-model-optout — 零命中的模型不走共線

**結論：不做自動退回，api 路徑維持共線組裝。**〔作者裁決 2026-10-28〕

作者原話：「no-cache-model-optout 案的基本原則是能抓就抓，不能就放棄抓快取。因為本來openRouter就有太多家，無法保證，有新廠商狀況也會改變。因此不強求覆蓋，所以我提議縮小規模，簡單驗證，沒有快取就放棄openRouter系的快取即可。」確認後的做法：維持共線，不做執行期判定、不退回單角色組裝、不補帳本欄位。

原規格的待拍板項一律不做，理由同上〔作者裁決 2026-10-28〕：solo 模式的 role 分配、退回要不要讓玩家看見、恢復探測的冷卻週期，以及「連續 3 次合格零命中」的判定規則。

## 簡單驗證（樣本小，不外推）

- **單發實測**（2026-10-28，OpenRouter 免費模型，見 [non-claude-real-cache](non-claude-real-cache.md)）：`nvidia/nemotron-3.5-super-vl`、`dots-studio/dots-3-note-preview` 會快取；Nvidia ultra、Poolside、Cohere 回 `cached_tokens: 0`。上述端點本次觀測回 0，不能據此區分不支援、冷啟、前綴或路由未命中。
- **作者真實帳本**（`prompt-cache.jsonl` 894 行、api 164 筆，api 最後一筆 2026-10-02；只算共線且輸入超過 4,336 的輪）：ox-alpha 零 7／中 48，原始 cached 非零 48 筆，其中 46 筆僅 64，不能據此認定共線內容多數命中；muse-spark 零 7／中 19。dots 零 2／中 0（同一 GM 線、69 秒內），帳本沒記實際 provider，不確定是不是測到會快取的 AtlasCloud；另有一筆 2,053 輸入中 2,048（33 秒內同內容重送）。glm-5.2:free 零 1（5,169 輸入）。樣本小，不推論。

## 已知限制

- 不快取的家每輪白送約 1,500 tokens（實際量隨卡數與卡長），內容是他角公開設定；換來的是角色資訊完整。包 B 之前的單角色組裝本來就不帶他角公開設定。
- 帳本的 api 路徑分不出 GM 輪與角色輪（GM 輪也記成 `mode: shared`），也沒有 system 雜湊與間隔，從帳本判斷「該不該中」會誤判。
- api 請求不鎖 provider，OpenRouter 有 sticky routing，但同一模型仍可能換家；帳本不記實際 provider。
