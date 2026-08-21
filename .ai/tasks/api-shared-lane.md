# API 路徑改走 chars 共線：讓換角色不再打散前綴快取

## Summary

claude 以外的四條路（api／codex／agy／grok）每換一個角色就整包重算前綴快取。拍板結論、實測證據與 A→B→C 分包見 [.ai/plans/api-shared-lane.md](../plans/api-shared-lane.md)。

## Progress

- 2026-08-21 與 Sol 兩輪討論拍板；依 codex／grok／agy 實測資料重排成 A→B→C 三包。
- **包 A（agy stream-json）完成**：改 `--output-format stream-json`、新增 `parse_agy_usage`、刪 `append_unreported` 死碼。
- **包 B（共線組裝器）完成**：新增 `assemble_shared_messages`（全部台詞 assistant＋名字前綴、system 走 `chars_lane_system`、本輪指定與私設在尾端 user、單角色桌私設回 system）；`flatten_messages` 加「空字串＝messages 已自足」語意；刪掉被取代的 `assemble_messages`（114 行）。cargo 514／vitest 157／build／i18n 全綠，clippy 警告數與改動前一致（32）。
- **未驗**：只到單元層。真模型的角色辨識三項（錯認前言者／串角／私設洩漏）與四路快取成對測試都還沒跑。

## Next action

送 Sol 驗收 A＋B；接著在真 app 上跑成對測試（同角色／換角色 × 冷／暖），記絕對 cached tokens，codex 要先扣掉固定的 9,984。包 C 已降為條件觸發。

## Constraints

- 包 C 未完成前，`anthropic/*` 共線後只保證 system 快取，不能宣稱完整支援。
- 包 B 對沒有快取的模型是純增（deepseek +34%），自動退回另案 no-cache-model-optout。
- 同名兩張卡在共線逐字稿裡不再可分（都是「名字：」），與 claude lane 行為一致。
