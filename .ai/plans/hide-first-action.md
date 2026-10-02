# hide-first-action 計畫

## 定案〔作者裁決 2026-10-02〕
拿掉「轉成世界書條目」要先隱藏的前置：桌上的卡也直接轉，只走原本的轉換警告確認框（`convertCardConfirm`），跟刪除一致。前後端兩道擋都刪，不加參數。刪除不動（「刪除也一樣」是誤記）。

## 現況
- 前端擋：`src/features/characters/CardEditor.tsx` `convertCardToWorldbookEntry()` 的 `card.archived === false` 分支，跳 `convertCardInUse`＋「知道了」後中止。
- 後端擋：`src-tauri/src/data/worldbook.rs` `character_to_worldbook_entry()` 開頭 `!card.archived` → `UiMsg::CardStillOnTable`。
- 這道擋不是為了資料一致：轉換最後刪卡，善後（名單重載、發言對象改人、關編輯頁）走 `onConverted`→`finishRemoval`，與隱藏、刪除同一條。
- 回合中轉換（Sol 審查 2026-10-02）：轉換原本拿共用許可（讀鎖），跟聊天／換幕不互斥。GM 已載入名單後卡被轉走會點名已刪 ID；換幕讀到卡後轉換刪卡，`set_character_auto_hidden` 的最後寫入可能重建卡檔。刪卡也有同樣風險，但拿掉前置後轉換多了觸發路徑，本案一併擋。

## 回合中防護
- 前端：`CardEditor` 收 `isBusy: () => boolean`（MainView／AppWorkspace 傳 `chat.isBusy`）；確認框前與 invoke 前各查一次，忙碌就在訊息列顯示 `worldBusy`、不 invoke。
- 後端：`character_to_worldbook_entry` 改在資料層取 `try_world_exclusive`，拿不到回 `UiMsg::WorldBusy`（同 `delete_world`）；command 不再拿共用許可。

## 要改的檔
- `src/features/characters/CardEditor.tsx`：刪 `card.archived === false` 那段；加 `isBusy` 兩處檢查；其餘（未儲存擋、`convertCardConfirm`、invoke、`convertCardDone`、`onConverted`）不變。
- `src/views/MainView.tsx`、`src/views/AppWorkspace.tsx`：傳 `isBusy`。
- `src-tauri/src/data/worldbook.rs`：刪 archived 檢查與註解裡「封存」限定；加獨占鎖；玩家卡擋 `PlayerCardNotConvertible` 保留；「先寫條目再刪卡」順序不變。
- `src-tauri/src/commands/world.rs`：拿掉共用許可。
- `src-tauri/src/ui_msg.rs`：刪 `CardStillOnTable`。
- `src/i18n/features/backend-msg.ts`：刪 `card_still_on_table`。
- `src/i18n/features/backend-msg-table.ts`：刪十語系 `be_card_still_on_table`。
- 十語系主字典 `src/i18n/{zh-TW,zh-CN,en,ja,ko,es,pt-BR,de,fr,ru}.ts`：刪 `convertCardInUse`。不新增鍵。
- 測試：`src-tauri/src/data/worldbook/tests.rs`、`src/features/characters/CardEditor.test.tsx`。

## 測試
- cargo（`data/worldbook/tests.rs`）：
  - 改寫舊拒絕測試 → 桌上的卡（`archived=false`）直接轉成功：條目寫入、卡檔刪除；玩家卡仍擋且卡檔還在。
  - `auto_hidden=true`（未手動隱藏）的卡也能轉。
  - 忙碌：持共用許可時轉換回 `WorldBusy`，世界書沒寫、卡檔還在。
  - 轉換後換幕（`begin_next_scene`，present 名單仍含該卡名）不重建卡檔、`list_characters` 不再列出它。
  - 世界書實際寫入失敗（unix：世界書檔設唯讀）回錯、卡檔還在。
- vitest：
  - `CardEditor.test.tsx`：桌上的卡只問一次 `convertCardConfirm` 就 invoke、呼叫 `onConverted`；取消確認不 invoke；未儲存擋下不 invoke；確認前忙碌不跳確認框、確認期間轉忙不 invoke，兩者都顯示 `worldBusy`；invoke 失敗顯示錯誤、不呼叫 `onConverted`。
  - `app-navigation.test.tsx`（整個 App）：把目前發言對象轉成條目後，主區與隱藏區都沒有該卡、發言對象換成下一張可見卡、編輯頁關閉。
- 全域搜尋 `convertCardInUse`、`CardStillOnTable`、`card_still_on_table`、`be_card_still_on_table` 零殘留；`npm run verify` 全綠。
