# API 路徑改走 chars 共線：讓換角色不再打散前綴快取

## Summary

claude 以外的四條路（api／codex／agy／grok）每換一個角色就整包重算前綴快取。拍板結論、實測證據與 A→B→C 分包見 [.ai/plans/api-shared-lane.md](../plans/api-shared-lane.md)。

## Progress

- 2026-08-21 與 Sol 兩輪討論拍板；依 codex／grok／agy 實測資料重排成 A→B→C 三包。
- **包 A（agy stream-json）完成並自驗綠**：`agy_args` 改走 `--output-format stream-json`、`parse_agy_line` 改吃 NDJSON 事件、新增 `parse_agy_usage`、刪掉 `append_unreported` 死碼。cargo 511／vitest 157／build／i18n 十語系全綠。

## Next action

做包 B：共線組裝器，一支統一組裝器同時修好四條路——全部台詞改 assistant＋名字前綴，順帶解掉 `flatten_messages` 雙重前綴。包 C（anthropic block）已降為條件觸發，延後。

## Constraints

- 包 C 未完成前，`anthropic/*` 共線後只保證 system 快取，不能宣稱完整支援。
- 包 B 對沒有快取的模型是純增（deepseek +34%），自動退回另案 no-cache-model-optout。
