# test-harness 設計計畫

目標：主線（Claude 透過 Bash）不靠使用者點擊，對真實 app（真後端、真資料流）跑 GUI 實測。方向〔作者裁決 2026-10-02〕：只在測試建置開啟的測試通道，正式版不帶。

## 與 menu-keyboard-webkit 的分工

| | test-harness（本案） | menu-keyboard-webkit |
|---|---|---|
| 跑什麼 | 真 app（Tauri＋Rust 後端＋真資料目錄） | 前端單獨跑在 WebKit（Playwright），IPC mock |
| 擅長 | 端對端流程：開桌、匯入、對話窗、後端資料核對 | 鍵盤／焦點回歸：可信任的按鍵事件、Tab 移焦、方向鍵 |
| 做不到 | 合成事件是 untrusted，瀏覽器預設行為（Tab 移焦、方向鍵捲動）不會發生 | 真後端與原生對話窗 |

本案的 `press` 只把 keydown/keyup 派給 app 自己的處理器（React onKeyDown 等），不宣稱能測瀏覽器原生移焦。

## 真實流程盤點（2026-10-02 grep）

**選檔：全是 HTML `<input type="file">`，前端讀 `File` 後 invoke**，沒有任何 `plugin-dialog open()`：
- `src/features/characters/CastRail.tsx:262`（匯入角色卡／世界書，走 `useImportController.importFile` → `file.arrayBuffer()` → `probe_import` 等）
- `src/features/worldbook/WorldbookSection.tsx:126`（重構卡 JSON）
- `src/features/characters/CardEditor.tsx:450`（角色圖）
- `src/features/settings/SettingsWindow.tsx:379`（贊助包 `.ttpack`）

→ 不必「預先指定下一次選檔路徑」：harness 直接把檔案內容造成 `File`，用 `DataTransfer` 塞進該 input 的 `files` 再派 `change`，原生 NSOpenPanel 根本不開。走的是與真人相同的 onChange → importFile 路徑（前提是包 1 的 DataTransfer spike 成立）。

**原生對話窗：全走 `@tauri-apps/plugin-dialog`，IPC 只有三個指令** `plugin:dialog|message`（confirm／ask／message 都經它，回傳按下的按鈕標籤，JS 端拿標籤跟 okLabel 比）、`plugin:dialog|save`、`plugin:dialog|open`（目前無人用）。後端 Rust 沒有直接呼叫對話窗；`window.confirm/alert` 零使用。
- confirm：`App.tsx` 480/610/660、`SettingsWindow.tsx:130`、`VersionStoreSections.tsx` 37/149、`useUpdateController.ts`、`CardEditor.tsx` 170/200/270/295/305、`CardImageDialogs.tsx:231`、`useCharacterController.ts` 278/299、`useSceneActions.ts:79`、`WorldEditor.tsx:105`、`useWorldbookEditor.ts` 97/201/238、`useRefactorWorkflow.ts:129`
- message：`App.tsx` 167/175/687、`useVersionCenter.ts:43`、`CardEditor.tsx` 257/264/279、`useImportController.ts` 258/300/328、`useRefactorWorkflow.ts:479`
- save：`CardEditor.tsx:238`、`ActReader.tsx:44`、`useSceneActions.ts:137`、`useWorldbookEditor.ts:257`、`useRefactorWorkflow.ts` 106/495
- 註冊點：`src-tauri/src/lib.rs` `.plugin(tauri_plugin_dialog::init())`；權限 `capabilities/default.json` 的 `dialog:default`。

## 架構

**前端零改動**；全部在 Rust 的 `test-harness` cargo feature 內（三個 spike 都成立，前端 shim 備案未啟用）。

```
scripts/harness.mjs ──HTTP(127.0.0.1:隨機埠, Bearer token)──▶ harness::server（tokio，每連線獨立 task）
                                                               │ window.eval(包裝後的 JS)
                                                               ▼
                                              webview 主框架執行 → 事件 harness-reply（plugin:event|emit）{id, nonce, ok, json}
                                                               │
                                              harness::eval 以 id 找 oneshot，回給 HTTP 回應
plugin:dialog|message/save/open ──▶ harness::dialog（假 dialog plugin）掛起，等 HTTP answer
```

1. **eval 回傳路徑**：每次 eval 產生 `id`＋一次性隨機 `nonce`，登記 oneshot；使用者 JS 包成 `async` 函式，結果 JSON 化後以 Tauri 事件 `harness-reply` 送回（走 `plugin:event|emit`，`core:default` 已含權限，不新增指令或 capability），Rust 比對 id＋nonce 後喚醒 HTTP 回應。拋錯回 `{ok:false, error}`；不可 JSON 化（循環、DOM 節點）回明確錯誤而非掛住。原始碼以 JSON 字串傳進頁面、用 AsyncFunction 建構：先當運算式（自動 return），語法不通再當函式本體（要自己寫 `return`）。逾時預設 30 秒，逾時即清 pending，但**不撤銷**已派出的 JS 或它觸發的後端操作，錯誤訊息寫明這點。包 3 的 DOM 輔助函式以 `include_str!` 編進 harness，eval 時前置注入。
2. **對話窗接管**：測試建置不註冊 `tauri_plugin_dialog::init()`，改註冊同名 `"dialog"` 的假 plugin，完整重現 plugin 2.7 的 IPC 契約：
   - message：按鈕 enum（`Ok`／`OkCancel`／`YesNo`／`YesNoCancel`／`OkCustom`／`OkCancelCustom`／`YesNoCancelCustom`）與未給按鈕時的預設值，回傳所選按鈕標籤；取消＝回傳 cancel 那顆的標籤（JS 端 `confirm`/`ask` 拿它與 okLabel 比出 false）。
   - save：成功回路徑字串、取消回 `null`。
   - open：單選回字串或 `null`、`multiple` 回陣列或 `null`、directory 同形（目前無人呼叫，仍照契約做）。
   每次呼叫進「待答佇列」（id、kind、title、內文、按鈕）並掛起，直到主線 `answer`；同一 id 重複回答回錯誤。
3. **選檔**：`file` 指令由 CLI 讀檔→base64→eval 在頁面造 `File`（保留檔名、MIME），用 `DataTransfer` 塞進 input 的 `files` 再派 `change`；明確允許 hidden input。

**包 1 spike 結果（2026-10-02，測試包實跑）**：
- **假 dialog plugin 的 ACL：成立。** 同名 `dialog` plugin 直接通過 `dialog:default`；頁面直呼 `plugin:dialog|message/save/open` 與真 UI 路徑（匯入完成的 `showMessage`「已把 38 條條目匯入世界書。」）都進了待答佇列，回答後 JS 端拿到 `"刪除"`／`"No"`／`null`／`["/tmp/a.png"]`。
- **eval 回報的通道：成立，改走事件。** 不註冊新指令，頁面用 `plugin:event|emit` 發 `harness-reply`，Rust `listen_any` 依 id＋nonce 取等待者。
- **DataTransfer 塞檔：成立。** 對 CastRail 的 hidden input 塞 `WestFantsy.png`：React onChange 觸發（value 被清空、出現「要匯入成世界書嗎」）、頁面讀到的檔名／`image/png`／1516586 bytes／sha256 與原檔一致；同一檔重選第二次 change 仍觸發；匯入後 `read_worldbook` 為 38 條。
- **iframe 隔離：成立。** 與介面卡相同的 `sandbox="allow-scripts"` iframe：父頁 DOM `SecurityError`、`__TAURI_INTERNALS__` 為 undefined、origin `null`，送不出事件也呼叫不了指令。

## 元件落點

| 檔案 | 內容 |
|---|---|
| `src-tauri/Cargo.toml` | `[features] test-harness = ["dep:getrandom", "dep:objc2", "dep:block2"]`（不加預設）；getrandom 產 token／nonce，objc2／block2（macOS）呼叫 WKWebView 截圖 |
| `src-tauri/src/harness/`（多個責任，直接開資料夾） | `mod.rs`（啟動檢查、harness.json、正常退出清理）、`root.rs`（root 驗證、鎖、fresh 清理）、`server.rs`（HTTP＋token 驗證）、`eval.rs`（id/nonce/oneshot）、`dialog.rs`（假 dialog plugin＋待答佇列）、`route.rs`（route 與 AI log）、`helpers.js` |
| `src-tauri/src/lib.rs` | `#[cfg(feature = "test-harness")] mod harness;`；`data_root`／`config_root` 加 cfg 分支；dialog plugin 註冊二選一 |
| `src-tauri/src/commands/update.rs`、`versions.rs` | feature 下安裝／回退入口回錯誤（見更新器） |
| `src-tauri/tauri.harness.conf.json` | 設定覆蓋（見建置組合） |
| `src-tauri/build.rs` | 建置期反向組合檢查（見建置組合） |
| `scripts/harness.mjs` | CLI（Node 內建 `fetch`，零依賴） |
| `package.json` | `harness:build` 指令 |

HTTP 伺服器手寫最小 HTTP/1.1 於 `tokio::net::TcpListener`（tokio 已有 `net`），不新增 web 框架。每連線 spawn 獨立 task，eval 等對話窗時 `answer` 仍進得來。解析規則：header 總長上限 16 KiB、body 上限 32 MiB（容得下 base64 卡圖）、讀取逾時 10 秒；重複的 `Host`／`Authorization`／`Content-Length` 一律 400；帶 `Transfer-Encoding` 一律 400（只收 Content-Length）；不支援 keep-alive。

## 指令集（最小可用）

`node scripts/harness.mjs <指令>`，結果一律 JSON 印 stdout，失敗非零結束碼。target＝CSS selector 或 `text=…`／`role=button[name=…]`。

| 指令 | 作用 |
|---|---|
| `launch [--root DIR] [--fresh] [--config-from FILE]` | 啟動測試包（直接執行 `.app/Contents/MacOS/` 執行檔並帶環境變數，不用 `open`），等 harness.json 出現、核對 session id 與 root、ping 通 |
| `status` / `quit` | 存活、root、session id、埠、pending 數；`quit` 有 pending 時先全部回答取消再正常結束 |
| `eval '<js>'` / `eval -f file.js`（別名 `js`） | 跑任意 JS，回傳值。別名是因為部分 shell 包裝會擋字面上的 eval |
| `text [target]` | 可見文字 |
| `query <target>` | 列出符合元素：tag、role、aria-label、文字、disabled、可見、是否被遮擋 |
| `click <target>` | `element.click()`；⋯ 選單照點 |
| `fill <target> <value>` | 原生 value setter＋`input`/`change`，React 受控欄位吃得到 |
| `select <target> <value>` | 設原生 `<select>` 值＋派 `change`，不開 NSMenu；值不在選項內回錯 |
| `press <target> <key>` | 聚焦後派 keydown/keyup（只測 app 自己的鍵盤處理器） |
| `wait <target> [--gone] [--timeout ms]` | 輪詢等出現／消失 |
| `file <target> <path>` | 塞檔進 file input（允許 hidden）並派 change |
| `dialogs` / `dialog-wait [--timeout]` | 列／等待掛起中的原生對話窗（內文、按鈕） |
| `answer <id\|next> <按鈕標籤\|ok\|cancel\|路徑>` | 回答對話窗；save 給路徑或 `cancel` |
| `invoke <command> [json]` | 在頁面內呼叫後端指令讀資料（如 `read_worldbook`），與 app 同一條 IPC |
| `route` | 送出前看各用途會打哪個模型（見 AI 額度） |
| `shot <out.png>` | webview 截圖：app 內以 WKWebView `takeSnapshot` 拍、PNG base64 回傳，不需螢幕錄製授權（`screencapture -l` 實測在無授權時失敗：could not create image from window） |

`click/fill/select/press` 防呆：target 匹配 0 個或多於 1 個、元素 disabled、不可見，或中心點 `elementFromPoint` 既不是目標也不是目標的後代（例如按鈕內的圖示／span 算命中）時一律回錯，不默默點別的。`file` 豁免可見性與 hit-test（hidden input 沒有可命中的點），仍檢查唯一匹配、必須是 `input[type=file]`、不得 disabled。

## 建置組合與編譯隔離

- **正式包**：`npm run tauri build`（不帶 feature、不帶 harness 設定）→ `harness` 模組、假 dialog plugin、輔助 JS、環境變數讀取全不編譯。
- **測試包**：`npm run harness:build` ＝ `tauri build --features test-harness --config src-tauri/tauri.harness.conf.json --bundles app`，產物 `src-tauri/target/release/bundle/macos/Table Tavern Harness.app`。
- `tauri.harness.conf.json`（Tauri 設定合併是 JSON Merge Patch，陣列整組替換）：`identifier: com.tabletavern.app.harness`、`productName: Table Tavern Harness`、`bundle.createUpdaterArtifacts: false`。若需要 harness 專用 capability，在 harness 設定的 `app.security.capabilities` **完整列出** `default` 加 harness 那份（陣列會整組替換，漏列 default 會砍掉現有權限）；harness capability 不放進 `src-tauri/capabilities/`，因為未指定清單時該目錄的檔案會被正式包預設全部納入。
- **組合一致性（雙向）**：feature 開但 identifier 不是 harness 的 → 啟動即中止。反向（用了 harness 設定或 shim mode，卻沒開 feature，會產出名稱像測試包、卻讀寫正式 data/config 路徑的 app）在**建置時**擋：`src-tauri/build.rs` 讀 Tauri CLI 傳入的合併後設定（identifier、`frontendDist`），若是 harness identifier 或指向 `dist-harness/` 而 `CARGO_FEATURE_TEST_HARNESS` 未設定就讓建置失敗；`vite.config.ts` 的 harness mode 也要求同一組合的環境旗標，否則失敗。檢查只在建置期，正式 runtime 不加任何 harness 程式。施工時先確認 build.rs 讀得到合併後設定的實際管道，讀不到就改由 `harness:build` 包裝腳本與 build.rs 共同擋並回報主線。正式包即使設了 `TT_HARNESS_ROOT` 也照常啟動、不開 listener、不改路徑（程式碼根本不在）。
- 前端 shim 備案未啟用，沒有 `dist-harness/`／vite harness mode；build.rs 仍擋 `dist-harness` 字樣，日後若啟用備案要補 vite 端檢查。
- **正式包驗收**（先建 harness 包、再建正式包，確認沒有殘留）：正式 app 的有效 ACL 不含 harness 權限、IPC 呼叫 harness 指令回「找不到指令」；帶 `TT_HARNESS_ROOT` 啟動不產 harness.json、無 listener（`lsof -p <pid> -iTCP`）；前端 assets 與 `dist/` 一致。掃標記字串只當補充。
- release 工作流程（`scripts/release/` 與 CI）不得出現 `test-harness`，verify 加一條文字檢查。

## 安全

- 只綁 `127.0.0.1`、埠由系統隨機分配；每次啟動產生 256-bit 隨機 token 與 session id，寫入 `<root>/harness.json`（0600，含埠、token、session id、pid、root）。CLI 讀檔取得，不經命令列參數或環境變數外洩。
- 每個請求驗 `Authorization: Bearer`（常數時間比對）；`Host` 必須是 `127.0.0.1:<埠>`（擋 DNS rebinding）；帶 `Origin` 標頭一律拒（擋瀏覽器網頁）。此規則沒有例外端點。
- **webview 側**：沒有任何從 webview 發起 eval 或回答對話窗的入口；不做把 `postMessage` 轉成控制指令的橋接。webview 唯一的 harness IPC 是 eval 回報，憑該次 id＋一次性 nonce，用過即失效。nonce 只防偽造回報，**不證明主頁腳本可信**：主頁若被注入腳本，仍可能攔截或竄改該次回報結果。主頁就是被測 app 本身，這在測試包可接受，但驗收結論不得建立在「回報不可能被竄改」上。
- **介面卡 iframe**（`CardInterfaceOverlay.tsx`，`sandbox="allow-scripts"` 無 same-origin）：包 1 實跑確認 iframe 內腳本碰不到父頁 DOM、取不到 `__TAURI_INTERNALS__` 或 invoke 失敗、無法送 eval 回報或回答對話窗；輔助 JS 只注入主框架。
- 不防本機同使用者的其他程式（同使用者本來就能讀 token 檔）。

## 資料隔離與啟停

- **root 驗證（任何寫入與背景工作之前）**：測試包必須有 `TT_HARNESS_ROOT`，缺了就中止。root 須為絕對路徑。比對一律用 canonical 路徑（系統路徑別名如 `/var`→`/private/var` 先解析掉，預設 root 落在 `$TMPDIR`＝`/var/folders/...` 因此合法）；root 尚不存在時，解析最近存在的祖先再接上剩餘段落組成 canonical 路徑。canonical 後不得等於或位於正式 data_root（`~/Documents/TableTavern`）、正式 config_root（`~/Library/Application Support/TableTavern`）之內，也不得是它們或 `$HOME` 的祖先（含 `/`、`$HOME`、`~/Documents`、`~/Library`），正式路徑本身也先 canonical 再比。root 建為 0700。data_root＝`<root>/data`、config_root＝`<root>/config`。CLI 預設 root `$TMPDIR/tt-harness`，CLI 端做同一套檢查（雙保險）。
- **identifier 層級互斥鎖**：鎖檔放在固定控制目錄 `~/Library/Application Support/com.tabletavern.app.harness.control/harness.lock`，不屬於任何清理範圍（`--fresh` 只清 root 與下述兩個共用目錄，碰不到它；root 驗證也拒絕等於、位於或包含此控制目錄的 root）。測試包啟動時取 `flock` 排他鎖並持有到退出（不論 root），鎖檔內寫 pid、root、session id；退出只釋放鎖、不刪檔，process 死掉鎖自動釋放。鎖檔永不刪除，避免 unlink 後另一實例重建同名檔再取鎖。
- **`--fresh`**：清理在測試包自己的 process 內做（CLI 帶 `TT_HARNESS_FRESH=1` 啟動）：先取 identifier 鎖，取不到＝任何 root 上都有活的 harness → 以結束碼 78 拒絕並印出鎖檔記載的 root／pid；持鎖期間刪 root 內容與共用目錄，同一把鎖接著持有到退出，清理與啟動之間沒有空窗。`--config-from` 的複製也在清理之後、同一把鎖內做。共用目錄實測為 `~/Library/WebKit/com.tabletavern.app.harness`、`~/Library/Caches/com.tabletavern.app.harness`，以及按需建立的 `~/Library/Application Support/com.tabletavern.app.harness`（版本庫）。
- **同 identifier 不同 root 會共用 WebKit 與 app_local_data_dir**：換 root 不等於全新，要乾淨就 `--fresh`。
- **discovery**：harness.json 帶 session id。`launch` 前遇到舊 harness.json：pid 已死 → 刪掉重來；pid 活著 → 拒絕並提示 `status`／`quit`。第二個測試實例（任何 root）會因 single-instance 把第一個叫到前面後自行退出；`launch` 發現讀到的 session 不是本次啟動的，就報錯說明。正常退出刪 harness.json；異常退出靠 pid 檢查收尾。
- **重用 root**：持鎖後、任何寫入與背景工作前，整棵 root 不得有 symlink；data／config 與 config.json 的實際目的地 canonical 後須在 root 內、不碰受保護路徑，否則以 78 拒絕。
- **`--config-from`**：只複製該檔到 `<root>/config/config.json`，0600。
- **與正式版並存**：另一個 identifier 讓 single-instance 的 socket 不同（macOS 版以 `/tmp/<identifier>_si.sock` 判斷，已查 `tauri-plugin-single-instance-2.4.5/src/platform_impl/macos.rs`），可與使用者正式版同時存在；測試包彼此之間仍單一實例。目前沒有 deep-link 註冊，不會搶 URL scheme。
- **新 identifier 的首次啟動**：macOS TCC 授權以 bundle 計，測試包首次啟動（例如子程序跑 AI CLI、截圖）可能跳系統授權；包 1 實測能否全自動，不能就列出需要使用者一次性點的項目。
- **AI 登入態**：Claude／Codex 等 CLI 走使用者家目錄的登入，測試包共用。**Grok 例外**：它的 profile 在 config_root 下（`transport/dispatch.rs` `grok_profile`：`<config_root>/cli-home`、`<config_root>/grok-home`），測試根目錄是空的，`--config-from` 只複製 config.json 也不帶登入態，測試包裡 Grok 會是未登入。

## 更新器

測試包停用安裝／回退類更新指令，檢查更新照常〔作者裁決 2026-10-02〕。在後端（不只 UI）於 feature 下處理：

- 回錯誤：`update_install`（`commands/update.rs`）、`rollback_install`（`commands/versions.rs`）。
- `update_post_launch`：會依 `current_exe` 找出所在 `.app`，條件成立時以 `swap_directories` 換掉它（`update.rs` 的 `mac_bundle`／`swap_for_launch`）；測試包下直接回 `Ok(())`，不做任何整理。
- 照常：`update_check`、`update_download`、`list_versions`。`update_download` 保留，因為下載涉及更新檔在網路上的存放位置與簽章驗證，要能在測試包裡測〔作者裁決 2026-10-02〕。

## AI 額度

不攔 AI 呼叫〔作者裁決 2026-10-02〕；花額度的測項由主線先問使用者。

- **`route`**：重用實際 resolver，不另寫一套：檔位挑選（`transport/client.rs` 的 `gm_tier` 等）＋`resolve_model`；智慧免費啟用時（`smart_free::is_active`）列出 `smart_free::prepare_call` 會組的候選清單與 fallback 順序，標明「實際由回應者決定」；檔位沒設模型、走 CLI 預設時明示「由 provider 決定」，不猜。涵蓋 GM、角色 lane、重構、翻譯、一句話開桌（`commands/genesis.rs`）、角色生圖（`commands/image.rs`）。
- **AI log**（`<root>/harness-ai.log`）：在實際派送點記錄，不記預估值。每次送出（含重試、smart-free 換模型）一行：時間、入口、provider、實際模型、第幾次嘗試。入口同上全涵蓋。

## 分包

| 包 | 內容 | 獨立驗收 |
|---|---|---|
| 1 骨架（完成 2026-10-02） | feature、harness 設定、root 驗證、鎖與 fresh、discovery、HTTP＋token、eval 往返、`launch/status/eval/quit/shot`；三個 spike 定案；更新器入口停用；假 dialog plugin 與 `dialogs/dialog-wait/answer` 為 spike 需要提前做進來 | 結果見交接檔 |
| 2 選檔＋對話窗實跑 | `file` 指令；假 dialog plugin 已在包 1 完成（契約單元測試已有） | 實跑：刪桌 confirm 取消／確定各一次、匯入完成 message 被接住、save 取消回 null 與給路徑後檔案真的寫出、同一 dialog 重複回答回錯 |
| 3 DOM 指令＋route＋端對端 | `text/query/click/fill/select/press/wait/invoke/route`、防呆、AI log | 多重匹配／disabled／遮擋各回錯；`route` 對空白設定與 `--config-from` 設定各印一次；下方端對端驗收 |

## 驗收（主線獨力，零 AI 額度）

fixture：CLI 驗收腳本收 `--fixture <path>`（`TestCards/` 在主 checkout，worktree 沒有），預設 `<主 checkout>/TestCards/WestFantsy.png`，sha256 `89fcec4c1588d487b590adf097390699b3135fbf551e6346d6d7b03071734421`，不符即中止。基準：匯入後世界書 38 條（主線交辦的實機基準）。

1. 開始前對正式資料兩個目錄做遞迴「路徑＋大小＋sha256」快照。正式 app 若開著，它自己的背景寫入也會造成差異：驗收期間讓正式 app 停在大廳不動，結束後差異逐檔列出由主線判讀，不以「沒差異」為唯一判準。
2. `harness:build` → `launch --fresh`。
3. 大廳點開新桌 → 進桌 → `shot`。
4. `file` 把 fixture 塞進 CastRail 的匯入 input → `dialog-wait` 接住匯入完成訊息、`answer next ok`。
5. `invoke read_worldbook {worldId}` 數到 38；畫面世界書列表也 `query` 得到 38 → `shot`。
6. `click` 一條世界書條目 → `wait` 條目編輯器出現 → `shot` → 取消離開（若跳未存 confirm，`answer` 取消路徑）。
7. 設定切換語言（`select`）→ 回大廳看範例桌詢問出現 → `shot` → 取消。
8. 再做一次正式資料快照並比對，差異逐檔列出。
9. 主線目視截圖：無裁切、遮蓋、樣式錯誤；截圖路徑附在回報。

## 拍板

- 設定檔預設空白起步；`launch --config-from <正式 config.json>` 可複製進測試根目錄的 config，會帶金鑰，只落在本機測試根目錄〔作者裁決 2026-10-02〕。
