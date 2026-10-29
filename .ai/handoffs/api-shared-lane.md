# API 路徑改走 chars 共線：讓換角色不再打散前綴快取

## Summary

claude 以外的四條路（api／codex／agy／grok）每換一個角色就整包重算前綴快取。拍板結論、實測證據與 A→B→C 分包見 [.ai/plans/api-shared-lane.md](../plans/api-shared-lane.md)。

## Progress

- 2026-08-21 與 Sol 討論拍板，依 codex／grok／agy 實測資料重排成 A→B→C 三包。
- **包 A（agy stream-json）完成**：`--output-format stream-json`、`parse_agy_usage`（`total_tokens` 三分支判別契約）、`result.response` 零增量 fallback、1.1.8 版本閘；刪 `append_unreported` 死碼。
- **包 B（共線組裝器）完成**：`assemble_shared_messages`；`LaneTurn.hoisted_private` 讓單角色桌只把穩定的 `private_md` 提進 system；`flatten_messages` 加「空字串＝已自足」語意；刪被取代的 `assemble_messages`（114 行）。
- **Sol 靜態驗收通過**。他抓到並已修正：單角色桌原本把含 keyword／state 的整包 confidential 提進 system，等於每輪打散前綴。測試覆蓋落差也已補上。
- **CLI 側角色辨識實跑通過**：codex／agy／grok 三家的串角與私設洩漏都乾淨，三家都是「用了私設卻不說破」。同批輸出驗到包 A 的解析器在真實資料上正確。
- 自驗：cargo 518／vitest 157／build／i18n 全綠，clippy 32（與改動前一致）。
- **包 C 降為條件觸發**，延後。
- **API 路徑角色辨識實機過**（2026-10-29 測試通道，OpenRouter `nvidia/nemotron-3-ultra-550b-a55b:free`，迷霧酒館 3 角色 7 通）：錯認前言者、串角、跨角色私設洩漏皆 0/7，三通專門誘發錯認（「接著說」、問誰說的、誘導「你是不是說過」）都答對歸屬；role 維持「全部台詞 assistant＋名字前綴」，不啟用全 user fallback〔模型判斷·未裁決〕。細節見[實測佇列](../reference/verification-queue.md)第 14 項。

## Next action

只剩 api 路徑的快取成對測試（同角色／換角色 × 冷／暖，記絕對 cached tokens）：免費模型量不到——nemotron 免費端點 7 通都報 cached 0，連同角色連續兩通也是 0，分不出沒快取還是沒命中。要用會回報快取的付費模型再測；目前列缺數據、不排程〔作者裁決 2026-10-29〕。codex 那路已在實測佇列第 14 項記過（底線 11,008）。

## Constraints

- 包 C 未完成前，`anthropic/*` 共線後只保證 system 快取，不能宣稱完整支援。
- 包 B 對沒有快取的模型是純增（輸入最小的 deepseek 最壞 +34%）；**哪個模型真的沒快取尚無實測**（2026-08-22 查證：那 27 筆缺 `cache_reporting` 欄，0 不可判讀）。不做自動退回、維持共線〔作者裁決 2026-10-28〕，見 [no-cache-model-optout](../plans/no-cache-model-optout.md)。
- 同名兩張卡在共線逐字稿裡不再可分（都是「名字：」），與 claude lane 行為一致。
- agy、grok 已不走本案共線：agy 2026-09-09 起續聊一角一線（5d2177ad），grok 2026-08-22 起續聊、10-07 起改 grok-shared-lane 共線；詳見規格「現況」節。agy 小桌（prompt 低於約 16k–20k）快取為 0 屬預期（觀測推論）。
