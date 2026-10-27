# vn-prestructure-split — VN v1 開工前的結構拆檔

## Summary
2026-10-27 排定，VN v1 開工前先做，避免在肥檔上再疊功能。三個超大檔（2026-10-27 實測行數）：
- `src-tauri/src/commands/chat.rs` 1525 行：commands 層應只做邊界（參數驗證、轉呼叫），業務邏輯搬到對應模組；在場規則搬出 chat.rs 跟著 [character-presence](character-presence.md) 做。
- `src-tauri/src/transport/response.rs` 1114 行。
- `src-tauri/src/data/scene/transcript.rs` 1257 行。

另列同階段候選：`src/App.tsx` 1049、`src/features/play/useChatController.ts` 1017、`src/features/play/` 23 檔、`src-tauri/src/transport/` 25 檔。三個 Rust 檔可同案也可分包，開工時依耦合判斷。

## Next action
未排程。開工前重量行數與資料夾檔數，依 [docs/STRUCTURE.md](../../docs/STRUCTURE.md) 決定拆法；依功能拆，不為行數硬切。

## Constraints
- 純結構搬遷，不改行為；`npm run verify` 全綠、驗收回報附前後行數。
