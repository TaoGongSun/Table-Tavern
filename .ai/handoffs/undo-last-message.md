# Handoff: undo-last-message

## Current state
實作完成、雙驗證綠，等使用者實機驗收後結案。

## Completed
- `pop_transcript`：讀出→去尾→整檔重寫，回傳是否真的刪了（空幕＝false，不建檔也不倒退）（src-tauri/src/data.rs:1271）。指令與註冊：src-tauri/src/lib.rs:481、1304。
- 前端收回：`undoLast` 砍一則並記下被砍的那句（src/App.tsx:3100-3112）；按鈕在輸入區操作列最左，虛線邊框與生成鈕區隔（src/App.tsx:3829-3838、src/App.css:1399-1403）。
- 單層復原：`undone` 帶桌與幕，`canRestore` 比對相符才顯示（src/App.tsx:2525-2527、3233）；提示長在訊息串底部被收掉的位置（src/App.tsx:3762-3768）。
- 失效規則：`appendEvent` 一寫新事件就清掉 `undone`（src/App.tsx:3096），避免復原把舊句插到新內容後面；復原失敗則把按鈕留著讓使用者再試（src/App.tsx:3117-3124）。
- i18n 十語系三鍵 undoLast／undoLastHint／undoRestore（src/i18n/*.ts）。de／fr／pt-BR 的 undoRestore 依 check:i18n 寬度上限改用單字（Wiederherstellen／Restaurer／Restaurar）。

## Verification
- `cargo test`：127 passed; 0 failed，含新測 `pop_transcript_removes_last_event_until_scene_is_empty`（src-tauri/src/data.rs:2894-2932，涵蓋去尾後行數對齊、連按到空回 false、未開始的幕不建檔）。
- `npm run build`：✓ built in 523ms（tsc 綠）。
- `npm run check:i18n`：九個非正典語系全 OK。
- 未實跑 app：Tauri 原生視窗這邊看不到畫面，UI 外觀與操作手感需使用者實機確認。

## Remaining / Next action
- 使用者實機驗收五項（見 tasks/undo-last-message.md Next action）。
- 不打包：依約 CI 打包等使用者說了才觸發。

## Constraints
同 tasks 檔。另注意：收回鈕與「請 X 發言」同列相鄰，若實機覺得容易誤按，優先調位置或樣式，別改成每次跳確認框（可連按的操作跳框太吵）。
