# Handoff: cli-connected-badge

## Current state
Mac 全數實測通過（2026-07-28 DMG 0.2.0 本機第六版）。只剩 Windows 端的新探針與 pre_probe 沒有機器可驗。

## Completed
- 連結旗標 `preferences["cli_connected:<id>"]`：安裝進度 done→true／error→false（src/App.tsx:287-321，監聽器掛一次、config/onSaved 走 ref 防重掛掉事件與舊閉包）；CLI 實聊成功自動標 true（角色回覆與 GM 旁白收尾處呼叫 markCliConnectedFromChat，src/App.tsx:1556-1575，走 chatConfigRef 防蓋掉串流中剛存的設定）。
- 設定頁已連結列：「已連結 ✓」＋小顆「重新驗證」，未連結維持原按鈕（src/App.tsx:456-482）。
- i18n zh／en：cliConnectedBadge／cliReverifyBtn（src/i18n.ts:60-61、257-258）。
- Windows agy `pre_probe` 維持 false 並保留 OAuth 副作用註解（src-tauri/src/install.rs:223、241）——codex 外包曾誤翻 true 並刪註解，主線審查退回。grok 已於 2026-07-28 換探針後翻 true（見下）。

### 2026-07-28 追加：Mac 的驗證回傳通道
- 症狀：Mac 按「登入／驗證」，終端機明明印出驗證成功，app 那列仍是「登入／驗證」。根因是 Mac 的 `install_cli` 只 `open -a Terminal` 丟腳本就撒手（src-tauri/src/lib.rs:117），`cli-install-progress` 事件僅 Windows 分支發送，旗標實際上只有「實聊成功」一條路會寫。
- 修法：腳本驗證通過後 `touch "$(dirname "$0")/.verified-<id>"`（src-tauri/src/lib.rs:103）；新增 `cli_verified` 指令讀印記（src-tauri/src/lib.rs:118）；`install_cli` 開工前先刪舊印記避免讀到上一輪（src-tauri/src/lib.rs:129）；Windows 於 done 階段寫同一個印記，兩平台停止條件一致（src-tauri/src/lib.rs:158）。前端輪詢從「偵測到執行檔就停」改成「讀到印記才停」並直接寫入旗標（src/App.tsx:437-467）。
- grok 探針換掉：`grok -p "ok"` 實測 26.0 秒（真跑一次 grok-4.5 推理，逼近 install.rs 的 30 秒探針上限），改用 `grok models` 0.8 秒。Mac 走 shell 故用 `grep '^You are logged in'`（src-tauri/src/lib.rs:66）；Windows 無 shell，改在 Rust 端比對——`InstallSpec.probe_expect`（src-tauri/src/install.rs:18、421），grok spec 換成 `grok models` ＋ expect ＋ `pre_probe: true`（src-tauri/src/install.rs:245-265）。
- `grok models` 無 OAuth 副作用，故 grok 的 pre_probe 可翻 true，Constraints 那條「已登入仍被要求重登」對 grok 解除；agy 維持 false（探針沒有等價的無副作用指令）。
- 設定頁「已連結 ✓」對齊：版本字串長短不一導致 badge 位置飄移、勾勾還會斷到下一行。`.cli-version` 吃掉中間空白讓 badge 與按鈕成組貼右，badge 本身 `white-space: nowrap` ＋ `flex-shrink: 0`（src/App.css:1508、1519）。

## Verification
- `cargo test`：77 passed; 0 failed。`npm run build`：✓ built in 413ms。
- diff read-back 逐條核過；Mac 腳本流程親讀確認本就 pre-probe（src-tauri/src/lib.rs:87）。
- 2026-07-28 追加：`cargo test` 99 passed; 0 failed（新增 3 個：印記只在驗證通過後 touch、probe_expect 不符強制走登入、probe_expect 命中則跳過登入）；`npm run build` ✓；`npx tsc --noEmit` 無錯。
- 探針耗時實測（本機 `/usr/bin/time -p`）：`grok -p ok` 26.01s、`claude -p ok` 1.93s、`grok models` 0.82s。
- 打包：`npm run tauri build` rc=0，`Table Tavern_0.2.0_aarch64.dmg` 4.7MB；`codesign -dv` 顯示 `Signature=adhoc`、`flags=0x10002(adhoc,runtime)`；`codesign --verify --deep --strict` 通過。
- 使用者實機實測（Mac，2026-07-28）：四家 CLI 按「登入／驗證」後該列自動變「已連結 ✓」（不必先實聊）；Grok 點「重新驗證」瞬間完成——這同時證明印記通道是通的，通道若斷按鈕會卡在「驗證中」直到 10 分鐘逾時；四列 badge 與勾勾對齊，不再斷行。

## Remaining / Next action
- **只剩 Windows 實機驗證**（本機無 Windows）：grok 已登入時按鈕應直接打勾、不彈登入視窗；未登入時仍正常走登入流程。併 [cli-install-windows](cli-install-windows.md) 那輪測試者回報一起看。
- 潛在後續（未拍板）：Windows agy 改用憑證檔存在性判斷登入狀態（無副作用），需先查證憑證檔路徑。grok 已用 `grok models` 解決，不需要這條。

## Constraints
同 tasks 檔。
