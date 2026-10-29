# vn-prestructure-split — VN v1 開工前的結構拆檔

## 目的
VN 模式開工前先把會疊功能的肥檔依功能拆好，避免在肥檔上再疊。commands 層只做 Tauri 邊界（接參數、呼叫 domain、整理回傳），業務邏輯搬到對應模組（[docs/STRUCTURE.md](../../docs/STRUCTURE.md)）。

## 案別與預算（立案凍結）
- 純結構搬遷、不改行為。新邏輯核心 0 行；只允許搬遷所需的 `pub(crate)`／`pub(super)`、use、mod 宣告、命令薄殼轉呼叫等連帶改動。
- 分包，一包一階段 commit；每包做完由主線送 Sol 驗收（重點：有沒有行為改動）。
- 驗收附：前後行數、`cargo test` 前後測試數相同、`git diff --color-moved` 確認原封搬移。

## 現況（2026-10-29 重量）
- `src-tauri/src/commands/chat.rs` 1839（本體 1129＋測試 710）
- `src-tauri/src/data/scene/transcript.rs` 1300（本體 728＋測試 572）
- `src-tauri/src/transport/turns.rs` 1149（本體 618＋測試 531）
- `src-tauri/src/transport/context.rs` 1018（本體 453＋測試 565）
- `src-tauri/src/transport/response.rs` 728、`src-tauri/src/refactor/apply.rs` 1001、`src/App.tsx` 1091、`src/features/play/chat/useChatController.ts` 905
- `src-tauri/src/transport/` 頂層 26 支 .rs＋5 個子資料夾；`src/features/play/` 頂層 16 項（含 chat/、scene/）

## 下一步
拆法已定（作者 2026-10-29 確認），見 [plans/vn-prestructure-split.md](../plans/vn-prestructure-split.md)。包 1（三檔測試搬出）已完成、待 Sol 驗收；過了再做包 2（commands/chat.rs → chat_turn）。包 2 commit 時補 BACKLOG character-presence 那行「在場程式已原封搬到 chat_turn/arrivals.rs」。分支 `vn-prestructure-split`。
