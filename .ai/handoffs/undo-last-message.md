# Handoff: undo-last-message

## Current state
實作完成、三項自驗綠，等使用者實機驗收後結案。

## Completed
- `pop_transcript`：讀出→去尾→整檔重寫，回傳是否真的刪了（空幕＝false，不建檔也不倒退）（src-tauri/src/data.rs:1271）。指令與註冊：src-tauri/src/lib.rs:481、1304。
- 前端收回：`undoLast` 砍一則並把它疊進復原疊（src/App.tsx:3108-3126）；按鈕在輸入區操作列最左，虛線邊框與生成鈕區隔（src/App.tsx:3852-3861、src/App.css:1399-1403）。
- 可連按復原：`undone` 存一疊（後收的在最上面）＋當時的桌與幕，`canRestore` 比對相符才顯示（src/App.tsx:2525-2531、3256）；復原一次消耗疊頂一則，疊空才收鈕（src/App.tsx:3128-3148）。提示長在訊息串底部被收掉的位置（src/App.tsx:3785-3791）。
- 復原刻意不走 `appendEvent`：那條路徑會清掉整疊（新內容一寫入就作廢，src/App.tsx:3100-3105），放回舊句只該消耗疊頂那一則。
- 連按重入防護：`undoBusy` ref 讓收回／復原同一時間只跑一次，否則前一次寫檔未回就再按會讀到同一份舊狀態、重複收回或重複放回同一則（src/App.tsx:2532-2534）。
- i18n 十語系三鍵 undoLast／undoLastHint／undoRestore（src/i18n/*.ts）。de／fr／pt-BR 的 undoRestore 依 check:i18n 寬度上限改用單字（Wiederherstellen／Restaurer／Restaurar）。

## Verification
- `cargo test`：127 passed; 0 failed，含新測 `pop_transcript_removes_last_event_until_scene_is_empty`（src-tauri/src/data.rs:2894-2932，涵蓋去尾後行數對齊、連按到空回 false、未開始的幕不建檔）。
- `npm run build`：✓ built in 497ms（tsc 綠）。
- `npm run check:i18n`：九個非正典語系全 OK。
- 復原順序以狀態推演核過：桌上 [A,B,C] 收兩次得 [A]、疊為 [C,B]，復原兩次依序放回 B、C 回到 [A,B,C]。
- 未實跑 app：Tauri 原生視窗這邊看不到畫面，UI 外觀與連按手感需使用者實機確認。

## Remaining / Next action
- 使用者實機驗收六項（見 tasks/undo-last-message.md Next action）。
- 不打包：依約 CI 打包等使用者說了才觸發。

## Constraints
同 tasks 檔。另注意：收回鈕與「請 X 發言」同列相鄰，若實機覺得容易誤按，優先調位置或樣式，別改成每次跳確認框（可連按的操作跳框太吵）。
