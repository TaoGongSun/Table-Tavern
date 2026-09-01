# codex／agy／OpenRouter 沒有續聊，快取到底有沒有真的抓到

## Summary

只有 claude 與 grok 走 lane 續聊，codex／agy／OpenRouter 三條路都是每輪重送全量的單發，命中全看服務端自己願不願意做前綴快取。

OpenRouter 那條的根因 2026-08-22 實證定案：`stealth/ox-alpha` **只在「上一次請求是這一次的逐字前綴」時給快取**，而 app 每輪把尾端動態塊（世界書 keyword、目前狀態、導演指示）換成新的，上一輪的請求就不再是這一輪的前綴，於是全滅（29 筆只有 2 筆真中）。決定性對照：同一桌內容、同一間隔，尾端換掉＝1%、嚴格前綴＝84%。修法拍板走「動態塊併入歷史」，規格、證偽清單、分包與驗收見 [plans/non-claude-real-cache.md](../plans/non-claude-real-cache.md)。

與 [vendor-prefix-floor](vendor-prefix-floor.md) 分開立案：那案是顯示層（把「只中到白送那段」標出來，不動組裝），本案是機制層（怎麼讓我方內容真的中）。

## Next action

照規格檔實作三包：包 1 `CacheStrategy` 判定與帳本欄位；包 2 尾巴重播（`TranscriptEvent` 新欄位、GM 線與角色線組裝改寫、`<turn-context>` 包裝與 system 規則、十語系文案）；包 3 chain epoch 的重開條件。驗收看離線重算的 byte-LCP 要等於 100%，再實跑三輪看 `cached_tokens` 是否跟著上一輪的 `prompt_tokens` 走。

## Constraints

- 驗收不能只看 `cached_tokens > 0`，要扣掉供應商白送的底線（codex 9,984／grok 128／api 64）。
- 套用範圍只限 `PromptShape::Turn` ＋ `StrictExtension`：oneshot（翻譯、換幕摘要、卡重構、生圖）一律排除，HTTP API 裡的 Anthropic 系維持 `cache_control` 路線，claude／grok lane 不動。
- 不做自動探針判定快取型態（會誤判 TTL、短 prompt、不支援快取），只認已驗證的 quirk 名單。
- agy 先確認 stream-json 那條有沒有真的把快取數字寫進帳本（api-shared-lane 包 A 改完之後還沒有實跑紀錄）。
