# Handoff: cli-connected-badge

## Current state
實作完成、驗證綠，等使用者以新 DMG 實測後結案。

## Completed
- 連結旗標 `preferences["cli_connected:<id>"]`：安裝進度 done→true／error→false（src/App.tsx:287-321，監聽器掛一次、config/onSaved 走 ref 防重掛掉事件與舊閉包）；CLI 實聊成功自動標 true（角色回覆與 GM 旁白收尾處呼叫 markCliConnectedFromChat，src/App.tsx:1556-1575，走 chatConfigRef 防蓋掉串流中剛存的設定）。
- 設定頁已連結列：「已連結 ✓」＋小顆「重新驗證」，未連結維持原按鈕（src/App.tsx:456-482）。
- i18n zh／en：cliConnectedBadge／cliReverifyBtn（src/i18n.ts:60-61、257-258）。
- Windows agy／grok `pre_probe` 維持 false 並保留 OAuth 副作用註解（src-tauri/src/install.rs:219、236、255）——codex 外包曾誤翻 true 並刪註解，主線審查退回。

## Verification
- `cargo test`：77 passed; 0 failed。`npm run build`：✓ built in 413ms。
- diff read-back 逐條核過；Mac 腳本流程親讀確認本就 pre-probe（src-tauri/src/lib.rs:87）。

## Remaining / Next action
- 使用者實測見 tasks/cli-connected-badge.md。測試包：Mac DMG（本機 0.2.0 第四版，含綠 badge）＋Windows run 30202162438（artifact `table-tavern-windows-unsigned` 7.45MB，https://github.com/TaoGongSun/Table-Tavern/actions/runs/30202162438 ）。
- 潛在後續（未拍板）：Windows agy／grok 改用憑證檔存在性判斷登入狀態（無副作用），需先查證兩家憑證檔路徑。

## Constraints
同 tasks 檔。
