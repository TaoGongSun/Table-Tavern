# cli-version-gate — 方案（第五版，待複審）

## 拍板
- A：收尾事件缺失一律判失敗〔作者裁決 2026-10-28〕。
- B：app 呼叫 CLI 時關掉 CLI 自動更新〔作者裁決 2026-10-28〕。理由：目標玩家不自己開 CLI，版本由 app 掌控；玩家自己在終端機升版是次要情境，不列為補新版的路徑。
- C：agy 太舊時給 app 內更新鈕，不叫玩家跑 `agy update`〔作者裁決 2026-10-28〕。
- D：優先保住「不更新也能玩」；只有模型退場／叫不到才出現更新鈕；能保證新版不會讓玩家玩不了，才可以有新版就提示〔作者裁決 2026-10-28〕。
- E：更新鈕先裝 app 已測過的最新版，裝完仍叫不到模型再給「裝最新版」；agy 只能裝最新版，列已知限制〔作者裁決 2026-10-28〕。
- F：app 更新後內建的已測版本比玩家裝的新時，claude、codex、grok 各提示一次；agy 不提示〔作者裁決 2026-10-28〕。
- app 內更新鈕是大宗玩家唯一的升版路徑，必須可靠：錯誤要辨識得出來並接到按鈕；更新失敗要有明確結果與退路，不能卡在半升級〔作者裁決 2026-10-28〕。

## 前提（查證結果）
- 四家串流事件都沒有 schema／protocol 版本欄位，也沒有相容性承諾。claude `system/init` 帶 `claude_code_version` 與 `capabilities`。
- **關閉自動更新，四家都做得到**（實測時都已帶關閉設定，作者的執行檔未再變動）：

| CLI | 自動更新何時發生 | app 呼叫時怎麼關 | 驗證 |
|---|---|---|---|
| claude | 原生安裝啟動時與執行中檢查 | env `DISABLE_AUTOUPDATER=1`（官方文件；不用 `DISABLE_UPDATES`，否則更新鈕的 `claude update` 也被擋） | `claude doctor` 顯示 `disabled (set by env: DISABLE_AUTOUPDATER)` |
| agy | 每次正式執行（含 `agy models`）都可能背景更新、原地換執行中那支檔；`--version` 不會觸發 | env `AGY_CLI_DISABLE_AUTO_UPDATE=true`（**值要是 `true`，`1` 無效**；官方文件未列） | 1.4.9 副本：`true` 時 log 印 `Auto-update disabled via environment variable`、不建 `updater/`；`1` 或空值照樣 spawn 更新程序；1.1.6 副本 `true` 同樣有效 |
| codex | updater 只跑在 app-server daemon 裡；daemon 只由互動式 `codex`（TUI）啟動，`exec`、`login`、`login status` 都不啟動 | app 呼叫本來就不觸發；另在 app 的安裝／更新指令帶 `CODEX_NON_INTERACTIVE=1`，讓官方安裝腳本不問「Start Codex now?」、不順手開 TUI 起 daemon | Sol 引 0.164.0 原始碼；本機查 `tui/src/startup_orchestration.rs:591` 是唯一啟動點、`install.sh:994-1055` 的開 TUI 提示 |
| grok | headless 時 stderr 非 TTY 會自動跳過 | 另帶 env `GROK_DISABLE_AUTOUPDATER=1` 雙保險（官方文件；不用 `--no-auto-update` 旗標，舊版可能不認） | 本機 `~/.grok/docs/user-guide/14-headless-mode.md` |

- **指定版本安裝**：claude `claude install <ver>`（help 與官方文件）；codex 官方 install.sh `--release <ver>`／`CODEX_RELEASE`；grok `grok update --version <ver>`；agy 只能裝最新（`agy update` 無參數、install.sh 只讀 latest manifest）。
- **模型叫不到時各家的輸出**（樣本已錄）：
  - claude：`assistant` 事件帶 `"error":"model_not_found"`，`result` 為 `is_error:true`、`terminal_reason:"api_error"`、`api_error_status:404`，退出 1。
  - codex：`turn.failed.error.message` 是內嵌 JSON，`error.code:"model_not_found"`（退役模型文字 `The '<id>' model is not supported when using Codex with a ChatGPT account.`），退出 1。
  - agy：`result.status:"ERROR"`、`result.error` 含 `is not recognized as a known model`，退出 1；舊版不認得新模型 id 時也是這句。
  - grok：只有一則 `{"type":"error","message":"Couldn't set model '<id>': Invalid params: \"unknown model id\" ..."}`，沒有 `end`，退出 1。
- **CLI 太舊（不認得 app 用的旗標）時各家的 stderr**：claude `error: unknown option '--x'`；codex 與 grok `error: unexpected argument '--x' found`；agy `flags provided but not defined: -x`。
- 關掉自動更新後 CLI 版本等於凍結：app 之後的版本若開始用新旗標，舊 CLI 會被拒，所以「CLI 太舊」的辨識不能只靠 agy 的 1.2.1。

## 盤點（現況，未變）
- 版本門檻只有 agy ≥ 1.2.1（`cli/request.rs` `agy_version_supported`），`transport/dispatch.rs` 兩處擋成 `AgyTooOld`，文案叫玩家跑 `agy update`。
- `cli/runner.rs` 收尾：有收尾且報錯→`CliReplyError`；沒收尾＋非 0→`CliCrashed`；沒收尾＋0＋有正文→當成功（codex-lane-resume 合併後 codex 例外）；沒收尾＋0＋沒正文→`CliNoReply`。正文事件改名與安靜的一輪分不出來；用量欄位改名悄悄變 0。
- 解析依賴表見 `cli/stream.rs`；各家下游另依賴：claude `modelUsage.<id>.contextWindow`（scene_budget）與 `total_cost_usd`、codex `thread.started.thread_id`、agy `conversation_id`、grok `end.stopReason`。

## 包 1：解析防線
1. **拆開 `CliProtocol`**：codex-lane-resume 的 `require_done` 實際兼管四件 codex 專屬行為。拆成：
   - `name`：錯誤訊息顯示名；
   - `require_done`：沒收尾事件就失敗，四家都設 true；
   - `sticky_failed_done`：已報失敗的收尾不被後到事件覆蓋，只給 codex（保住 grok「頂層 error 後接 end_turn 判成功」及其測試）；
   - rollout 存檔偵測與「完成但非 0 退出就丟線」改掛 codex 專屬欄位。
2. **錯誤優先序**（取消、失控、逾時維持原判定，在此之前返回）：收尾事件報的錯 > SoftError > `CliCrashed`（非 0 退出）> `CliOutputUnrecognized`（只在退出 0 時，附 stderr 末 5 行）> `CliNoReply`。
3. **認不得的行**：`CliProtocol` 帶已知型別表，範圍＝「所有夾具出現過的型別」∪「解析器明確處理的型別」，含頂層判別鍵（claude／codex／grok 的 `type`、agy 的 `event`）與解析器有分派的子型別（至少 codex `item.type`、claude `stream_event` 的 `delta.type`、agy `step_type`）。判別鍵缺失或不是字串、stdout 非 JSON 行，也算認不得。
   - 成功的輪遇到認不得的行：只記一次 log（行程記憶體內依 CLI＋版本＋型別去重，重開 app 重記），不打斷。
   - `CliOutputUnrecognized` 觸發條件：（沒收尾）或（成功收尾但正文空且收尾文字也空），而且這輪出現過認不得的行；退出碼為 0。
4. **上次正常版本**（`last_ok`，意思是「這版曾完整跑完一輪」，不代表所有欄位都相容）：只在 `drive_child` 最終回 `Completed` 之後寫，寫在資料根目錄的 `cli-last-ok.json`（`{cli: version}`）：行程內一把互斥鎖包住「讀 → 改該 CLI 的值 → 寫臨時檔 → rename」，四家的寫入排隊依序做；版本沒變就不寫、寫失敗只記 log。不用帳本反推：帳本在解析到用量那行就落帳，那時這輪還可能以失敗收場。文案四種：上次正常版本與目前不同（「{cli} 從 {last_ok} 換成 {version} 後回覆格式無法辨識，需要等本 app 更新支援」）、同一版本曾正常過（「{cli} {version} 這次的回覆格式無法辨識」，不寫「從 X 換成 X」）、從沒成功過（「{cli} {version} 的回覆格式無法辨識，需要等本 app 更新支援」）、版本不明（「{cli} 的回覆格式無法辨識，版本查不到」）。不寫「請回報」。
5. **程式位置**：認不得的行追蹤與訊息組裝放 `cli/runner/` 底下新子模組，`runner.rs` 不再長。
6. **夾具與契約測試**：
   - `src-tauri/src/cli/stream/fixtures/<cli>-<version>-<情境>.jsonl`＋同名 `.version`。每家至少：一般短答、帶思考、帶工具（走生圖那條旗標）、失敗輪（模型叫不到）。去識別化 cwd、sessionId、signature、使用者外掛清單。現有樣本：四家一般短答、四家模型叫不到、agy 1.1.6 模型叫不到。
   - 擷取腳本（開發者用）：解析出實際執行檔路徑、`--version` 與擷取用同一支；固定帶四家的關閉更新設定與獨立 HOME。
   - 契約測試逐檔斷言正文、收尾、用量數值與下游欄位（`modelUsage.contextWindow`、`total_cost_usd`、`thread_id`、`conversation_id`、`stopReason`）。
   - 行為測試四家都驗：有正文缺收尾、未知事件正常完成、未知事件加缺收尾、成功收尾但正文空加未知事件、明確失敗、取消、非 0 退出。
7. 不設版本上限。

## 包 2：更新控制與更新鈕
1. **關自動更新**：在 agy-home-cleanup 的 `cli/spawn.rs`（子程序唯一入口）依 CLI 注入上表的 env，所有呼叫點（聊天、旁白、重構、生圖、`agy models`、`grok models`、探針、`--version`）一律帶。測試：spawn 單元測試斷言 env。施工時補三項實測，一律用執行檔副本與測試 HOME，不動作者真實的執行檔：codex 舊版副本＋全新 CODEX_HOME 跑 `exec`、grok 1.0.46 副本帶 app 的 env 跑 headless、claude 副本（或測試 HOME）跑 `-p` 觀察是否更新；agy 已實測。
2. **辨識「需要更新 CLI」的錯誤**，新增錯誤碼並接更新鈕：
   - `CliModelUnavailable { cli, model }`：上列四家模型叫不到的輸出。claude、codex 認結構化欄位，agy、grok 認文字；每種都有夾具測試。
   - `CliTooOld { cli, version, required? }`：
     - 各家宣告的最低版本（agy 1.2.1 沿用；之後 app 開始用某旗標時同步加門檻）；
     - stderr 出現上列「不認得旗標」字樣；
     - 伺服器要求升級：claude 官方錯誤文件有 `Claude Code <ver> does not support this model; version <req> or newer is required`，認出後帶 `required`。codex、grok、agy 查無固定錯誤（Sol：codex 只把錯誤收成 `message`，426 是 WebSocket 退回 HTTP、不代表 CLI 太舊；Grok：文件沒有；agy：不知道，執行檔內也查無），由下一條兜底，施工時若錄得到就補成正式辨識。
     - 取代現有 `AgyTooOld`。
   - **無法分類的 CLI 錯誤**：訊息旁放「檢查 CLI 更新」，按下走同一套更新鈕流程（先裝已測版）〔作者裁決 2026-10-28〕。「無法分類」以白名單定義〔模型判斷·未裁決〕：沒有掛上任何既有穩定錯誤碼，也不屬於 `api_error_kind` 判出的暫時性錯誤、限流或額度錯誤。排除清單（這些一律不放按鈕，玩家斷網或額度用完時不能叫他換執行檔）：
     - 取消、失控、逾時（斷流）、登入失效；
     - `context_overflow` 系列；
     - `AI_INCOMPLETE_RESPONSE`；
     - 暫時性錯誤、限流、額度用盡；
     - `CliNotFound`、`CliUpdating`；
     - 已分類的 `CliModelUnavailable`、`CliTooOld`（它們有自己的按鈕）與 `CliOutputUnrecognized`（見下一條）。
     判定順序：先跑完上面的排除判定（含從 CLI 錯誤原文辨識出的登入失效、限流、額度、暫時性錯誤），都不符合才落到「無法分類」；`CliCrashed` 與 `CliReplyError`（CLI 自報原文）只有在通過排除判定後才放按鈕，不能蓋掉這些分類。測試逐項斷言排除清單不出按鈕、這兩種會出。
   - **`CliOutputUnrecognized` 的按鈕**〔模型判斷·未裁決〕：claude、codex、grok 已裝版本比已測版舊時，文案改成「CLI 版本比本 app 測過的舊」，旁邊放「檢查 CLI 更新」，按下只裝已測版（不給「裝最新版」；已測版下載或安裝失敗也不轉去裝最新版，照還原契約收尾並顯示更新失敗）；不比已測版舊、或 agy，不放按鈕，文案維持「需要等本 app 更新支援」。理由：已測版有契約測試支持（夾具保證不了所有執行情境），比照 D 的「能保證才提示」。
   - 玩家有自訂模型（`tier_models` 覆寫）時，模型叫不到的訊息同時給「改選模型」，因為換 CLI 版本不一定救得了指定的舊模型。
3. **更新鈕**（出現在上面三種錯誤旁，與第 6 點的一次性提示）：
   - **互斥與在途**：更新與一般呼叫共用同一把互斥鎖，鎖住的是「依 CLI 分開的在途 pid 表」與「更新擋板」。按下按鈕時在同一把鎖內：逐一確認表裡的 pid 還活著（死掉的直接移除，避免 `spawn.rs` 那種沒收屍的票券讓按鈕永久停用）→ 還有活的就回「等目前這輪結束」→ 沒有就立起擋板。一般呼叫那一側：`spawn.rs` 在同一把鎖內先檢查擋板、再寫入一筆佔位登記，出鎖後 spawn，成功就把 pid 補進那筆、失敗就移除佔位。佔位由 RAII 守衛持有：守衛 Drop 時 pid 若還沒回填（future 被取消、`select!` 走了別支、panic），就把這筆移除，不留永久佔位；這樣從「取得資格」到「child 登記完成」之間，更新端看得到這筆在途，不會趁空窗立擋板。擋板立起期間，所有經 `spawn.rs` 的該 CLI 呼叫（含 `--version`、models、登入探針）立即回 `CliUpdating`，不排隊。不走世界鎖（換的是全 app 共用的執行檔）。
   - **目標版本**〔作者裁決 2026-10-28〕：claude、codex、grok 先裝已測版本（清單版本比玩家目前的新才裝；不比較新，或 `CliTooOld.required` 高於清單版本，就跳過這一步直接給「裝最新版」）；裝完重送仍叫不到、或已測版下載安裝失敗（例如該版已下架），同一位置改給「裝最新版」。agy 一律裝最新版。
   - **指令**（mac 與 Windows 都用 CLI 自己的指令或官方安裝腳本，背景隱藏執行，走 `spawn.rs` 的「更新」模式，寫安裝 log）：

| CLI | 指定版 | 最新版 | Windows 備註 |
|---|---|---|---|
| claude | `claude install <ver>` | `claude update` | 同指令；install.ps1 也收 `$Target`（官方文件與腳本）。還原用官方腳本，見下 |
| codex | 官方安裝腳本帶 `CODEX_RELEASE=<ver>`，一律 `CODEX_NON_INTERACTIVE=1` | 同腳本不帶版本 | install.ps1 `-Release`／`CODEX_RELEASE`、`CODEX_NON_INTERACTIVE`（Sol 引 install.ps1:1-14）；新版放新 release 目錄再切 `current` junction，不覆寫執行中的 exe（Sol 依原始碼推論，**待 Windows 實機驗證**） |
| grok | `grok update --version <ver>`；首次安裝與還原用官方腳本 `bash -s <ver>`（https://x.ai/cli/install.sh 第 18 行 `TARGET="$1"`；施工時用測試 HOME 實裝一次確認） | `grok update` | install.ps1 `-Version`／`GROK_VERSION`，固定裝到 `%USERPROFILE%\.grok`；執行中 exe 複製失敗會改名 `.old` 再複製，再失敗改回原名（Grok 依 install.ps1 `Install-Exe` 原始碼推論，**待 Windows 實機驗證**）；`grok update` 本身遇到鎖檔怎麼做，文件沒寫 |
| agy | 無 | `agy update` | install.ps1 已安裝就直接結束、不收版本參數；`agy update` 在 Windows 的鎖檔行為 agy 答不知道 |

   - **更新模式的環境**：保留 `cli/proxy.rs` 套的系統代理變數；拿掉 claude、grok 的關閉自動更新變數；grok、codex 拿掉 app 的 `GROK_HOME`、`CODEX_HOME` 與假 HOME，用真家目錄（兩家都把新版裝進當下的 HOME，實測 1.0.46 副本在假 GROK_HOME 下 `grok update` 只裝進假目錄）；agy 帶 app 的 agy-home（`agy update` 換的是被呼叫的那支，且 `AGY_CLI_DISABLE_AUTO_UPDATE=true` 不擋手動更新，1.1.6 副本實測）。
   - **程序樹與逾時**：更新程序與還原程序都放進獨立的程序群組（unix process group；Windows 以 `CREATE_SUSPENDED` 建立、綁進 Job Object 才放行，綁定失敗就收掉、判更新失敗）。更新上限 5 分鐘、還原上限 5 分鐘；逾時殺整組。正常結束也一樣殺整組並等它消失，確認整組都停了才進下一步（判定、還原或撤擋板）；確認有 10 秒期限，從送出殺訊號起算，主程序收屍與群組清空共用這個期限，收屍遲遲不回來也算確認不了；確認不了就擋板不撤、回報「更新程序停不下來，請重新啟動 app」，重新啟動由啟動復原接手。
   - **驗證**：重新用 `find_binary` 找到 app 平常呼叫的那支，`--version` 跑得起來並解析出版本。指定版要等於目標版本；最新版要比原本新。只看退出碼不算。裝到別處而這支沒變，判失敗。更新前的 `--version` 跑得起來但版號解析不出來時照樣放行（比照 below_minimum），「沒變」改比 `--version` 原文。
   - **啟動復原標記**：依 CLI 分檔 `cli-update-pending-<cli>.json`，開始更新前寫入 CLI 名、原路徑、原版本、備份路徑、目標版本、更新模式（指定版／最新版），每一步 spawn 後再記下該步的程序群組（unix pgid＋組長啟動時間，防 pid 重用）；記不進去就立刻停掉整組、確認停了，這一步算失敗，照常驗證後判定（版本沒動＝更新失敗；動了就照契約還原，此時官方腳本那條還原也會因同樣原因被停掉，只剩本機連結指回能走）；確認不了停止就回報停不下來、擋板不撤。成功或還原成功後才刪；還原失敗就保留。app 啟動時看到它，在背景處理，處理期間該 CLI 的擋板保持立起：
     - 先確認上一輪那組已經停了：還活著就殺整組並確認；組長 pid 還在但啟動時間對不上＝被別人重用，不殺；確認不了就擋板不撤、設定頁顯示「停不下來，請重新啟動 app」；
     - `find_binary` 那支跑得起來、版本等於原版本：執行檔沒動過，直接刪標記，不還原；
     - 符合上面「驗證」的規則（指定版等於目標、最新版比原版新）：更新其實成功了，刪標記；
     - 其他：照下方契約還原，還原成功才刪標記；
     - 還原失敗：保留標記並顯示「CLI 無法執行，請按重新安裝」；之後啟動看到「標記在、上次已還原失敗」只顯示同一則提示，不再每次重跑還原（標記裡記一個 `restore_failed`）。判定 `restore_failed` 後撤掉該 CLI 的擋板，否則玩家按「重新安裝」會被 `CliUpdating` 擋住。
     - `install.rs` 重新安裝成功、且 `--version` 驗證通過後，刪掉該 CLI 的標記；設定頁偵測時 `find_binary` 那支 `--version` 跑得起來、版號解析得出來，也刪（mac 的安裝在 app 外的終端機跑）。
   - **四家還原契約**（新檔壞掉、驗證不符或逾時時）：
     - 還原一律不靠剛裝上、可能已經壞掉的 CLI 本身。
     - 本機還留著原版就先把連結指回去，不在或指回去驗證不過才跑官方腳本；每試一個都驗證，走了哪條路寫進安裝 log。
     - claude：mac 先把 `~/.local/bin/claude` 連結指回 `~/.local/share/claude/versions/<原版本>`；那個目錄不在就跑官方 install.sh `bash -s <原版本>`。Windows 用官方 install.ps1 帶 `$Target=<原版本>`。
     - codex：mac 先把 `~/.codex/packages/standalone/current` 指回 `releases/<原版本>-<平台>`；不在才跑官方安裝腳本帶 `CODEX_RELEASE=<原版本>`。
     - grok：mac 先把 `~/.grok/bin/grok`、`agent` 指回 `~/.grok/downloads` 的原版檔（`grok-<原版本>-<平台>`，再試官方腳本裝的不帶版號檔 `grok-<平台>`）；不行才跑官方安裝腳本，mac `bash -s <原版本>`、Windows `GROK_VERSION`／`-Version`。還原成功後刪掉下載目錄裡已沒有連結指向的新版檔。
     - agy：更新前把現行執行檔複製到 app 資料目錄，還原時複製回去（agy 自己留的 `agy.<ns>.old` 會被清掉，不能依賴）。
     - 還原後重新 `find_binary`，`--version` 要等於原版本，才算還原成功。
     - 還原也失敗，才顯示「CLI 無法執行，請按重新安裝」，接既有安裝流程。
   - **結果與文案**：
     - 已更新到 X →「請再送一次」（不自動重送，不替玩家花額度）；
     - 已測版不比目前新（或伺服器要求的版本比已測版高）：只代表跳過已測版這一步，文案「測過的版本解決不了這個問題，可以改裝最新版」，同一位置直接給「裝最新版」；
     - 最新版就是目前這版、或裝了最新版仍叫不到 →「更新沒有解決問題」，接著有自訂模型就給「改選模型」，否則說明要等本 app 更新（不推論成「問題不在 CLI」）；
     - 更新失敗、仍是原本的 Y 版（只有還原驗證過才這樣寫）；
     - CLI 無法執行，請按重新安裝；
     - 更新程序停不下來，請重新啟動 app。
   - **測試**：四家各自的假 CLI 腳本驗：成功、指定版裝錯版本、已測版下載失敗轉「裝最新版」、新檔壞掉後經官方腳本還原（不呼叫壞掉的 CLI）、還原也失敗（標記保留）、裝到別處而原路徑沒變、更新逾時殺整組（含孫程序）、還原逾時、已是最新版、有活的在途 pid 時拒絕、死掉的 pid 被清掉後放行、佔位登記期間按更新會被擋、佔位期間 future 被丟棄後按更新仍放行、擋板期間其他呼叫立即回 `CliUpdating`、啟動時看到標記依目標版本判斷並還原、兩家同時更新各自的標記不互蓋、標記為 `restore_failed` 時擋板已撤且按重新安裝成功後標記被刪、提示不再出現、確認不了上一輪那組停了時擋板不撤並顯示停不下來、啟動復原先殺掉上一輪還活著的那組、組長 pid 被重用時不殺、正常結束也收掉背景程序、本機留存原版先指回（指回不對才跑官方腳本）、版號解析不出照樣放行、主程序收屍遲遲不完成判確認不了、群組身分寫不進標記時立刻停掉並判失敗、擋板在 lane 預寫之後才立起時 lane 還原成嘗試之前且擋板撤掉後照常續聊；更新模式環境（各家拿掉／保留哪些變數）有單元測試。Windows 走 cargo check；執行中 exe 不能覆寫、失敗保住舊版、Job Object 殺整組，全部排進實機佇列，未驗證前方案一律視為待驗證。
4. **首次安裝也裝已測版**〔作者裁決 2026-10-28〕：`install.rs` 的一鍵安裝，claude、codex、grok 改裝已測版本（mac 安裝腳本與 Windows install.ps1 都依上表帶版本：claude `bash -s <ver>`／install.ps1 `$Target`、codex `CODEX_RELEASE`、grok `bash -s <ver>`／`GROK_VERSION`）；agy 只能裝最新版，維持現狀。安裝後同樣以 `--version` 驗證實際版本。已測版下載不到時自動改裝最新版（mac 的 `curl | bash` 一律 `pipefail`，下載失敗才觸發得了退路）；裝完比對實際版號，不是已測版就註明「非已測版本」。
5. **已測版本清單**：程式常數 `cli/tested_versions.rs`（每家一個版本字串，agy 也列但只供診斷），隨 app 出貨。維護：開發者用擷取腳本錄好新版夾具並跑過契約測試後，才改常數。綁定測試：
   - 擷取腳本依呼叫情境（lane 開線／續聊、單發、生圖、models）各錄一份，把當次的旗標與值寫進夾具旁的 `.args`；單元測試逐情境比對：現行該情境的參數組裝函式產生的旗標與固定值，全都出現在同情境夾具的 `.args` 裡，不拿各情境的聯集比，少一個或值不同就紅燈。
   - 讀夾具旁 `.version` 的內容，與常數逐字比對；並斷言常數不低於該 CLI 的最低版本門檻。
   - 常數裡每個版本在 `cli/stream/fixtures/` 都要有四種情境的夾具。
6. **提示一次**〔作者裁決 2026-10-28〕（claude、codex、grok）：偵測到已裝版本比清單舊、且這個清單版本還沒提示過時，在設定頁該 CLI 那列顯示「有測試過的新版本」與更新鈕，並記進設定 preferences 鍵 `cli_tested_prompted:<cli>`＝清單版本（設定頁實際顯示這則提示時才寫入，只在背景偵測到不算；之後同版本不再提示；app 更新帶來更新的清單版本才會再提示）。寫入失敗只記 log。實際畫面位置施工時先截圖給作者看。

## 施工時序
codex-lane-resume 與 agy-home-cleanup 都合進 main 後，從新的 main 開工。包 1 先、包 2 後，各自送審驗收。包 2 的啟動復原要排在 agy-home-cleanup 的啟動回收之前；或讓那個回收也經過更新擋板，擋板立著時等復原完成再跑。

## 已知限制
- agy 只能裝最新版，更新鈕無法保證新版相容；agy 也不做提示〔作者裁決 2026-10-28〕。
- agy 的關閉變數沒有官方文件，agy 改名時會悄悄恢復自動更新；包 2 的實測與夾具腳本會在升版時發現。
- 玩家自己在終端機或其他 Codex 介面用同一支 CLI 時照樣會升版；屬次要情境，壞了由包 1 的訊息說明。
- codex、grok、agy 被伺服器要求升級時沒有可辨識的固定錯誤，只能落在「無法分類的錯誤」旁的「檢查 CLI 更新」〔作者裁決 2026-10-28〕；訊息本身不會說是版本太舊。
- 玩家用 npm／brew 裝的 claude、codex：app 的一鍵安裝或更新會另外裝一份到 `~/.local/bin`，兩份並存，app 用的是 `find_binary` 先找到的那份。
- Windows 登入視窗（`cmd start`）與 mac 終端腳本裡跑的 CLI 不經 `spawn.rs`，不過更新擋板〔作者裁決 2026-10-29〕。
- 更新成功後重送仍叫不到模型時，「裝最新版」要再按一次〔作者裁決 2026-10-29〕。
- 官方腳本自己 `setsid`／背景化出去的程序脫離了程序群組，擋不住也殺不到；程序樹的處理已盡力〔作者裁決 2026-10-29〕。
- 不核對下載檔的雜湊或簽章，完全信任官方安裝腳本〔作者裁決 2026-10-29〕。
- unix 上更新程序已 spawn、群組身分還沒寫進標記的瞬間 app 被強制結束：重開後的啟動復原無法確認上一輪那組還活著沒有，直接依版本判定。空窗只有寫一個小檔的時間；Windows 的程序暫停建立、綁上 Job 前當機就停在暫停狀態，不會動作〔模型判斷·未裁決〕。
- grok 還原時把 `~/.grok/bin/agent` 一併指回原版：原本沒有這個連結的舊安裝會多出一個〔模型判斷·未裁決〕。
- 程序停不下來而擋板留著（stuck）時，聊天端的錯誤仍顯示「更新中」，只有設定頁那列寫「請重新啟動 app」〔模型判斷·未裁決〕。
