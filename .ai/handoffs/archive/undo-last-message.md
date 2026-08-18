# Handoff: undo-last-message

## Current state
實作與實機驗收皆完成，2026-08-18 結案。

## Completed
- `pop_transcript`：讀出→去尾→整檔重寫，回傳是否真的刪了（空幕＝false，不建檔也不倒退）（src-tauri/src/data.rs:1271）。指令與註冊：src-tauri/src/lib.rs:481、1304。
- 前端收回：`undoLast` 砍一則並把它疊進復原疊（src/controllers/useChatController.ts:162）；按鈕在 AI 動作組最左，虛線邊框與生成鈕區隔（src/views/PlayView.tsx:274-282、src/App.css:1399-1403）。
- 可連按復原：`undone` 存一疊（後收的在最上面）＋當時的桌與幕，`canRestore` 比對相符才顯示（useChatController.ts:99-124）；復原一次消耗疊頂一則，疊空才收鈕（useChatController.ts:184）。提示長在訊息串底部被收掉的位置（PlayView.tsx:187-193）。
- 復原刻意不走 `appendEvent`：那條路徑會清掉整疊（新內容一寫入就作廢，useChatController.ts:131-141），放回舊句只該消耗疊頂那一則。
- 連按重入防護：`undoBusy` ref 讓收回／復原同一時間只跑一次，否則前一次寫檔未回就再按會讀到同一份舊狀態、重複收回或重複放回同一則（useChatController.ts:106）。
- i18n 十語系三鍵 undoLast／undoLastHint／undoRestore（src/i18n/*.ts）。de／fr／pt-BR 的 undoRestore 依 check:i18n 寬度上限改用單字（Wiederherstellen／Restaurer／Restaurar）。

## Verification
自驗（2026-08-01）：
- `cargo test`：127 passed; 0 failed，含新測 `pop_transcript_removes_last_event_until_scene_is_empty`（src-tauri/src/data.rs:2894-2932，涵蓋去尾後行數對齊、連按到空回 false、未開始的幕不建檔）。
- `npm run build`：✓ built in 497ms（tsc 綠）。`npm run check:i18n`：九個非正典語系全 OK。

實機驗收（2026-08-18，測試桌 01KZ3NJT2C8DPKN0983X7EJ11P 第 13 幕，六項全過、零 bug、無程式碼變更）：
1. 收回鈕虛線邊框，與實線的請X發言／GM 旁白／GM 推進明顯分群。
2. 連按一次一則：3→2→1→0，收到空時 jsonl 變 0 bytes、鈕自動停用，前一幕檔案行數未動。
3. 復原提示長在訊息串底部剛被收掉的位置。
4. 連按復原逐則倒回，順序還原、三則放完鈕自消；`ts`／`state` 快照／`raw` 欄位原封不動（不是重建的）。
5. 整疊失效兩條都成立：換桌再回來鈕不見；生成新旁白後鈕也沒回來。
6. 生成中收回鈕淡出停用（送出與三顆 GM 鈕同步停用）。
- 連按重入防護補測：快點三下收回，兩則收乾淨、第三下落空，檔案無錯亂。

## Remaining / Next action
無。

## Constraints
同 tasks 檔。實機確認收回鈕的虛線邊框足以與「請 X 發言」區隔，不需改成確認框。
