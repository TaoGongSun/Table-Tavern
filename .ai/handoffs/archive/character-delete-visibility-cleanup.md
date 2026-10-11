# 刪角色後世界書可見度名單沒清

Status: done〔作者裁決 2026-10-10：立案，列為網頁版公開前門檻〕。已合併 main（2026-10-11）。方案見 [plans/character-delete-visibility-cleanup.md](../../plans/character-delete-visibility-cleanup.md)。

## 現況
- 定案：刪角色時從世界書條目的 `visibility.characters` 名單與 `source_cards` 拿掉該 id，名單清空改給 GM〔作者裁決 2026-10-11〕。
- 落點：`src-tauri/src/data/worldbook/character_scrub.rs`（清理規則、整本原子寫）、`src-tauri/src/receipts/character_delete.rs`（刪角色／轉條目入口、收據可抵達值一致轉換）、`receipts.rs` 撤銷清掉自己刪的角色與收據原子寫；兩支 command 回 `worldbook_cleanup_failed`，前端提示一行。
- 測試：`data/worldbook/character_scrub_tests.rs`、`receipts/character_delete_tests.rs`、`src/features/characters/useCharacterController.test.ts`。
- 測試通道實測：刪角色、角色卡轉條目後世界書編輯器顯示「GM 專有」。

## 下一步
無。Sol 與 opus-review 三方驗收通過，已結案。
