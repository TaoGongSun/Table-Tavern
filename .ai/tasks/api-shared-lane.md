# API 路徑改走 chars 共線：讓換角色不再打散前綴快取

## Summary

claude 以外的四條路（api／codex／agy／grok）每換一個角色就整包重算前綴快取。拍板結論、實測證據與 A→B→C 分包見 [.ai/plans/api-shared-lane.md](../plans/api-shared-lane.md)。

## Progress

- 2026-08-21 與 Sol 討論拍板，依 codex／grok／agy 實測資料重排成 A→B→C 三包。
- **包 A（agy stream-json）完成**：`--output-format stream-json`、`parse_agy_usage`（用 `total_tokens` 三分支判別契約）、`result.response` 零增量 fallback、1.1.8 版本閘；刪 `append_unreported` 死碼。
- **包 B（共線組裝器）完成**：`assemble_shared_messages`（全部台詞 assistant＋名字前綴、system 走 `chars_lane_system`、本輪指定與私設在尾端 user）；`LaneTurn.hoisted_private` 讓單角色桌只把穩定的 `private_md` 提進 system；`flatten_messages` 加「空字串＝已自足」語意；刪被取代的 `assemble_messages`（114 行）。
- **Sol 靜態驗收通過**（2026-08-21）。他抓到並已修正：單角色桌原本把含 keyword／state 的整包 confidential 提進 system，等於每輪打散前綴。覆蓋落差也已補上。
- 自驗：cargo 518／vitest 157／build／i18n 全綠，clippy 32（與改動前一致）。
- **包 C 降為條件觸發**，延後。

## Next action

實機 runtime 驗收（要使用者在電腦前）：真模型角色辨識三項（錯認前言者／串角／私設洩漏，只需在 API 路徑驗）＋四路成對測試（同角色／換角色 × 冷／暖），記絕對 cached tokens，codex 要先扣掉固定的 9,984。

## Constraints

- 包 C 未完成前，`anthropic/*` 共線後只保證 system 快取，不能宣稱完整支援。
- 包 B 對沒有快取的模型是純增（deepseek +34%），自動退回另案 no-cache-model-optout。
- 同名兩張卡在共線逐字稿裡不再可分（都是「名字：」），與 claude lane 行為一致。
