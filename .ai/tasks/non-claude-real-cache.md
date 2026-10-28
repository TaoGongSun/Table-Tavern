# codex／agy／OpenRouter 沒有續聊，快取到底有沒有真的抓到

## Summary

只有 claude 與 grok 走 lane 續聊，codex／agy／OpenRouter 三條路都是每輪重送全量的單發，命中全看服務端自己願不願意做前綴快取。

OpenRouter 那條的根因 2026-08-22 實證定案：`stealth/ox-alpha` **只在「上一次請求是這一次的逐字前綴」時給快取**，而 app 每輪把尾端動態塊（世界書 keyword、目前狀態、導演指示）換成新的，上一輪的請求就不再是這一輪的前綴，於是全滅（29 筆只有 2 筆真中）。決定性對照：同一桌內容、同一間隔，尾端換掉＝1%、嚴格前綴＝84%。修法拍板走「動態塊併入歷史」，規格、證偽清單、分包與驗收見 [plans/non-claude-real-cache.md](../plans/non-claude-real-cache.md)。

與 [vendor-prefix-floor](../handoffs/archive/vendor-prefix-floor.md) 分開立案：那案是顯示層（把「只中到白送那段」標出來，不動組裝），本案是機制層（怎麼讓我方內容真的中）。

## Next action

排在 multichar-next-speaker-silent 結案之後〔作者裁決 2026-10-28〕。OpenRouter 背後各家快取機制不同（嚴格前綴、自動前綴門檻、不做快取、不回報），單一模型的實測不能推到全部，驗收主力放在不靠供應商的部分：
1. 離線：同一桌連續幾輪的實際請求 byte-LCP（照規格要 100%），不花模型額度。
2. codex：測試 root PATH 放包裝腳本抓兩通實際輸入比對共同前綴（CLI 額度）。
3. OpenRouter 只用免費模型的每日額度（未儲值帳戶約 50 則／日）：確認各家回報欄位解析與 `cache_reporting`，並找一家會回報快取的嚴格前綴型實跑三輪，看 `cached_tokens` 是否跟著上一輪 `prompt_tokens` 走；免費端點不快取或不回報就記錄，不當成機制失敗。
證據站得住再照規格檔三包施工。

## Constraints

- 驗收不能只看 `cached_tokens > 0`，要扣掉供應商白送的底線（codex 9,984／grok 128／api 64）。
- 套用範圍只限 `PromptShape::Turn` ＋ `StrictExtension`：oneshot（翻譯、換幕摘要、卡重構、生圖）一律排除，HTTP API 裡的 Anthropic 系維持 `cache_control` 路線，claude／grok lane 不動。
- 不做自動探針判定快取型態（會誤判 TTL、短 prompt、不支援快取），只認已驗證的 quirk 名單。
- agy 先確認 stream-json 那條有沒有真的把快取數字寫進帳本（api-shared-lane 包 A 改完之後還沒有實跑紀錄）。
