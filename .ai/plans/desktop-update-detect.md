# desktop-update-detect — 桌面版 App 內更新與回退

「更新」「回退」「桌紀錄」「介面」四節〔作者裁決 2026-09-30〕。「技術做法」是 Claude、Grok、Sol 三方審核共識（2026-09-30），不是作者裁決。

## 前置與範圍

- **本案實作排在 [ai-response-stop](../handoffs/archive/ai-response-stop.md) 完成之後**：更新閘門要用它的中止功能，先做本案會缺這塊〔作者裁決 2026-09-30〕。
- **熱更新（只換前端、不換 App 本體）本輪不做**：約三分之二的程式 commit 會動到 Rust 本體，熱更新很少派得上用場；等 Mac「App 管理」實測結果再議〔作者裁決 2026-09-30〕。

## 更新

- **方案 B：偵測＋一鍵更新**，用 `tauri-plugin-updater`。取代 release-2「v1 不做自動更新」。
- **updater 金鑰**：minisign，與 OS 程式碼簽章無關、免費。私鑰除 GitHub secret 外另存離線備份——弄丟＝已安裝的玩家永遠收不到更新。
- **什麼算有更新**：版本號比目前高的正式 release（預發布不算）。App 不判斷改了多少，發不發版由作者決定。
- **提醒強度**（0.x 版以第二位為功能版）：
  - 第三位變（小修）→ 版本號旁亮小點
  - 第二位變（功能版）→ 啟動時橫幅一次
  - 含資料格式轉換的版本 → 啟動時對話框，說明「會轉換桌紀錄、已自動備份」
  - 不做強制更新。
- **檢查時機**：啟動一次，長開每 24 小時一次；離線／rate limit／任何失敗都靜默、不擋啟動。
- **安裝**：一律等玩家按，不在遊戲進行中彈出；有 AI 回應在途時等它結束；要提前結束就用中止功能——那是本來就該有的獨立功能，另案 [ai-response-stop](../handoffs/archive/ai-response-stop.md)，更新流程共用同一套中止。玩家面雙軌：一鍵更新＋展開看更新說明／略過此版。
- **自動檢查預設開**，設定可關；關了只在手動按「檢查更新」時連線。發布帖的資料流向聲明要寫：檢查更新會讓 GitHub 看到 IP 與目前版本。
- **不做測試版頻道**。要先給人試就放 GitHub 預發布版手動下載。
- **版本號紀律**：`tauri.conf.json` 的 version 每次發版要升，CI 擋 git tag 與它不一致。
- **Mac 只出 Apple Silicon（aarch64）**，不做 Intel／universal。

## 回退

- **版本庫**：App 內更新下載的安裝包存進版本庫；另存「更新前那一版」的安裝包，讓玩家能回上一版。
- 早於「第一個帶更新功能＋格式版本檢查」的版本不列入回退清單。
- 回退前重新驗簽；被退掉的那一版自動標為略過。
- 回退確認視窗列出「回到這版後，哪幾張桌會變唯讀」。
- **位置**：設定頁「版本」區一鍵「回到上一版」，展開可從清單選其他版本。
- **空間**：自動保留最近 3 版；清單逐版可永久刪除——顯示每版大小與總占用、確認視窗；刪「更新前那一版」時警告會失去一鍵回退（不禁止）；確認視窗寫明只刪程式、不動桌紀錄與備份。

## 桌紀錄

- 每張桌加 `format_version`，**開到哪張才轉哪張**，逐版轉換鏈常駐（只要可能還有舊資料就不算死程式碼）。
- 轉換前備份整張桌。轉換失敗的桌標「需要修復」，原檔不動，其他桌照常。任何情況都不刪原檔。
- **舊版遇到比自己新的格式**：該桌唯讀（可看對話紀錄、不能續玩），提示「要繼續請更新到 X 版」；另提供「改用轉換前的備份繼續玩」，轉換後玩的內容另存不刪。新版才新建的桌在舊版同樣唯讀／提示更新。
- **備份保留**：每張桌只留最近一次（轉換前備份、上一條的另存各一份）；跨兩次以上格式變更再回退時，那些桌只能看不能玩。設定頁與版本庫同一區列出大小，可逐份永久刪除。

## 介面

- 首頁（桌列表）版本號旁放更新標記。
- 設定頁「版本」區：檢查更新、更新說明、略過此版、回到上一版、版本清單、儲存空間（版本庫＋桌備份）。
- 遊戲畫面內不出現任何更新提示。

## 技術做法

- **路徑**：桌在 `document_dir()/TableTavern`、設定在 `config_dir()/TableTavern`（`src-tauri/src/lib.rs:33`），都不在安裝目錄，換程式本體碰不到。版本庫與桌備份放 `app_local_data_dir()`——不進 Windows Roaming、不進 iCloud／OneDrive 同步。
- **正向更新**：Rust 端 `check()` → `download()`（plugin 在這步驗簽）→ 存進版本庫 → 安裝。JS 的 `downloadAndInstall()` 拿不到安裝包，不用。
  - Windows：跑 NSIS（`/UPDATE` 加 passive `/P`），本體隨即退出讓 NSIS 換檔。
  - Mac：自寫替換——舊 `.app` 先挪到可復原位置，換上新版，失敗就搬回。plugin v2 現行 `install` 失敗時舊 App 會跟暫存目錄一起被刪，除非鎖定的 plugin 版本已修，否則不用它換 Mac。
- **回退**：plugin 無法安裝本機檔，自寫「讀快取 → 核對版本／平台／檔型 → 用同一把公鑰驗 `.sig` → 同上安裝步驟」。版本庫存 `*-setup.exe` 與 `.app.tar.gz`，各帶 `.sig`。
- **更新前那一版**：在開始安裝**之前**下載並驗簽完成（Windows 安裝一啟動本體就退出，背景下載會被切斷）；拿不到（沒有 updater 產物、離線、已刪）就照實顯示沒有回退點。0.2.0 等沒有格式檢查的版本一律不列，優先於「能回上一版」——所以第一個帶更新功能的版本沒有回退點。
- **更新閘門**：兩平台都在呼叫安裝之前完成——擋新請求、等所有在途寫入落地並持久化。不能只靠現有 `inflight`（一般聊天沒登記），也不能靠 `RunEvent::Exit` 收尾（Windows 走 `process::exit`；`on_before_exit` 只有 Windows 會跑）。
- **格式轉換＝可恢復的整桌提交**：鎖桌 → 整桌複製到同磁碟暫存目錄 → 轉換 → 驗證 → 在暫存桌寫格式標記 → 舊桌挪到備援名 → 暫存桌改成正式名（Windows 不能 rename 覆蓋非空目錄）。每一步中斷後重開都要能判斷並接續或復原。重開、從備份還原之後一律再走格式檢查。
- **唯讀要擋全部寫入**：格式檢查放在任何反序列化或副作用之前，擋整張桌所有寫入入口（state、transcript append、角色、世界書、圖、開桌自動回存）。修掉 `append_transcript` 在 `read_state` 失敗後仍寫入的路徑。回退確認列唯讀桌時直接讀格式標記，不靠完整反序列化。兩個 App 實例同開一桌：單一實例或每桌鎖檔。
- **設定檔維持可寫**：後端寫入以磁碟上最新的整份 JSON 為底，只覆寫認得的欄位，其餘（含巢狀）原樣保留，也擋掉舊畫面過期副本的覆蓋；寫法改成保留 `0600` 權限的暫存寫入＋改名（現為直接 truncate）。0.2.0 手動裝回仍會丟新設定，但桌紀錄不在設定檔裡。
- **發版**：新做正式 release workflow（現有 test-build 只吃 `test-v*`、只傳 artifact）。Windows 只出 NSIS 並固定 NSIS。Windows、Mac 兩個打包 job 之後，由第三個彙整 job 合併 `latest.json`、核對 `.sig` 與檔案，再公開 release（兩個 job 各自判斷「我是最後一個」會重複寫）。

## 分包

1. **發版管線**：updater 金鑰、正式 release workflow、彙整 job、tag／版本號一致檢查。可先做。
2. **格式版本與遷移**：`format_version`、可恢復的整桌提交、備份、唯讀擋全部寫入、單一實例／鎖檔、設定檔保留未知欄位。
3. **偵測與一鍵更新**：Rust 端下載存庫、安裝步驟、更新閘門、檢查時機、提醒強度、略過此版、設定開關。
4. **版本庫與回退**：更新前那一版、驗簽、回退安裝、自動保留 3 版、逐版刪除、桌備份管理。
5. **介面**：首頁標記、設定頁版本區與儲存空間區。

包 2–5 可分開開發，但**首次公開啟用更新的那一版必須全部一起發**：少了包 2 回退會寫壞資料，少了包 4 第一批玩家沒有回退點，少了包 5 提示沒地方放。

## 包 1 做法（Claude、Grok、Sol 三方共識 2026-10-01）

- **金鑰（作者手動）**：`npx tauri signer generate -w ~/.tauri/table-tavern.key`（設密碼）→ 私鑰與密碼離線備份 → `gh secret set TAURI_SIGNING_PRIVATE_KEY`／`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。公鑰進 `tauri.conf.json` 的 `plugins.updater.pubkey`，`endpoints` 指 `https://github.com/TaoGongSun/Table-Tavern/releases/latest/download/latest.json`（GitHub 的 latest 不含預發布與草稿）。plugin 本身包 3 才註冊；包 3 不開 `requireSignedVersion`（CLI 2.11.4 的簽章不含版本）。
- **`createUpdaterArtifacts: true` 只在 workflow 用 `--config` 打開**，不寫進 `tauri.conf.json`：本機 `npm run tauri build`（GUI 實測用）不需要私鑰。不用 `v1Compatible`。
- **版本一致**：新 `scripts/check-version.mjs [tag]`——`package.json`、`Cargo.toml` 的 `[package].version`、`tauri.conf.json` 三處相同；給了 tag 就還要等於 `v<版本>`。無 tag 模式加進 `npm run verify`。
- **`release.yml` 取代 `test-build.yml`**：
  - 推 `v*` tag → 正式發版；推 `test-v*` tag 或手動觸發 → 演練（打包＋組 `latest.json`＋驗簽，只傳 artifact，不建 release）。`test-v*` 讓分支上就能演練——手動觸發要 workflow 先在 main 才出現。
  - `check` job：版本一致（正式發版含 tag）；正式發版另檢查 tag 的 commit 在 `origin/main` 上（checkout 抓完整歷史）。
  - `build-windows`：`tauri build --bundles nsis`；`build-macos`：`--target aarch64-apple-darwin --bundles app,dmg`。都帶簽章 secret，上傳安裝檔＋`.sig`（＋DMG）為 artifact。任一邊失敗就不公開。
  - `finalize` job（ubuntu，唯一有 `contents: write`）：收兩包 → 改無空格檔名 `TableTavern_<版本>_x64-setup.exe`、`TableTavern_<版本>_aarch64.app.tar.gz`、`TableTavern_<版本>_aarch64.dmg`（`.sig` 同步改名）→ `.sig` 與 conf 公鑰各自 base64 解碼後以 minisign 驗每個安裝檔（CLI 金鑰不符只警告，這裡才是真的閘）→ 組 `latest.json`：`version`、`notes`、`pub_date`（當下 RFC3339）、`platforms.windows-x86_64`／`darwin-aarch64` 的 `url`（`/releases/download/<tag>/<檔名>`）＋`signature`（`.sig` 原文）→ 正式模式：同名 release 已公開就失敗、是草稿就先刪 → 建草稿 release、傳齊全部檔案（含 `latest.json`）、最後轉公開；依 SemVer 解析，有 prerelease 段就明設預發布且不當 latest，否則明設為 latest。演練模式只傳 artifact。
  - `notes`＝`CHANGELOG.md` 的 `## [版本]` 到下一個二級標題；正式發版找不到就失敗，演練容許缺。
- 提醒強度要用的「含格式轉換」欄位留給包 2／3 加進 `latest.json`，包 1 不預留。

## 包 2 做法（Claude、Grok、Sol 三方共識 2026-10-02）

本包的程式有兩種角色：新版用來轉換舊桌；**舊版（從第一個帶更新功能的版本起）遇到更新格式時要靠它不寫壞**。後者一旦發出就改不了，所以前向相容要先做對。

- **備份改成「原桌改名」**：轉換前備份就是原桌目錄本身改名成 `worlds/.tt-pre-<id>/`，同磁碟改名、位元組不動，免跨磁碟複製與驗證，原檔字面上從不刪除。這推翻「技術做法」裡桌備份放 `app_local_data_dir()` 的審定（版本庫仍放那裡）；代價是文件夾被 OneDrive／iCloud 同步時備份也跟著同步。
- **格式標記**：每桌 `worlds/<id>/format.json` = `{"format_version": N, "app_version": "x.y.z"}`。`CURRENT_FORMAT = 1`，轉換鏈先是空的（測試用假轉換驗機制）。`create_world` 與轉換寫標記，平常存檔不重寫。判讀：
  - 標記是正整數 → 照值。
  - 缺檔且 `state.json` 能用現行 `WorldState` 解開 → 1。
  - 標記解不開、不是正整數，或缺檔且 `state.json` 也解不開 → 當作「比本版新、版本不明」，不轉換、不寫入。
- **開桌**：新 command `open_world(world_id)`，前端 `enterTable` 第一步呼叫，回 `ready`／`migrated {from,to}`／`read_only {format_version?, app_version?, backup_available}`／`needs_repair {message}`。轉換失敗原桌不動；目錄組合乾淨時每次開都重試，不另存失敗旗標。
- **轉換提交**（每桌一個操作日誌 `worlds/.tt-op-<id>.json`，記 `op`、`stage`、起始時有沒有舊備份；每次改日誌都 fsync，fsync 完才做下一步）：
  1. 目錄組合乾淨（沒有日誌、staging、trash）才開始；取得該桌獨占鎖，有寫入在途就回 busy。日誌 `{op:"migrate", from, to, stage:"build", had_pre}`。
  2. 複製 `<id>` 到 `.tt-staging-<id>` → 逐版轉換 → 用現行程式完整讀過 → 寫標記 → fsync 檔案與目錄。
  3. 日誌 `stage:"swap"` → r1 `.tt-pre-<id>`→`.tt-trash-<id>`（有舊備份才做）→ r2 `<id>`→`.tt-pre-<id>` → r3 staging→`<id>`。
  4. 日誌 `stage:"cleanup"` → 刪 trash（作者裁決備份只留最近一次）→ 刪日誌。
- **恢復＝查目錄組合表**（I＝`<id>`、S＝staging、P＝pre、T＝trash、N＝`.tt-newer-<id>`）。只有表內組合自動處理；表外一律 `needs_repair`：不改名、不刪任何目錄、不重試。自動處理只會刪 S 與 T，絕不刪 I、P、N。往前接續的 S 必須標記等於目標版且完整讀得過；驗不過就照退回欄處理。
  每列是完整條件：「有」＝必須存在，「無」＝必須不存在，「h」／「n」＝存在與否必須等於日誌的 `had_pre`／`had_newer`，「≤h」＝h 為真時可有可無、h 為假時必須無，「—」＝不看也不碰。任一欄不符就是表外。
  | op／stage | I | S | P | T | N | 處理 |
  |---|---|---|---|---|---|---|
  | migrate／build | 有 | 可有可無 | h | 無 | — | 刪 S、刪日誌（原桌未動） |
  | migrate／swap（r1 前） | 有 | 有 | h | 無 | — | 刪 S、刪日誌 |
  | migrate／swap（退回刪 S 後中斷） | 有 | 無 | h | 無 | — | 刪日誌 |
  | migrate／swap（r1 後，限 h 真） | 有 | 有 | 無 | 有 | — | 退回：T→P、刪 S、刪日誌 |
  | migrate／swap（r2 後） | 無 | 有 | 有 | h | — | S 驗過→S→I、進 cleanup；驗不過→P→I、（h 真）T→P、刪 S、刪日誌 |
  | migrate／swap（r3 後） | 有 | 無 | 有 | h | — | 進 cleanup |
  | migrate／cleanup | 有 | 無 | 有 | ≤h | — | 刪 T（若在）、刪日誌 |
  | restore／swap（r1 前） | 有 | 無 | 有 | 無 | n | 刪日誌（未動） |
  | restore／swap（r1 後，限 n 真） | 有 | 無 | 有 | 有 | 無 | 退回：T→N、刪日誌 |
  | restore／swap（r2 後） | 無 | 無 | 有 | n | 有 | P 的格式 ≤ 本版才 P→I、進 cleanup；否則表外 |
  | restore／swap（r3 後） | 有 | 無 | 無 | n | 有 | 進 cleanup |
  | restore／cleanup | 有 | 無 | 無 | ≤n | 有 | 刪 T（若在）、刪日誌 |
  - 沒有日誌：有殘留 S、且 I 或 P 至少一個在，才刪 S；有 T 沒有日誌屬表外。
  - 表內處理途中改名失敗（佔用、權限）就停：這次回 `needs_repair`、不刪 I／P／N；下次開桌重新比對表格。
  - 退回照欄內寫的順序執行（先 P→I，再 T→P）。「進 cleanup」＝先把日誌 stage 改成 cleanup 並 fsync，才刪 T。日誌解不開或 op 不認得也屬表外。
  - 恢復完成（含 restore）一律再走格式判讀，才開放寫入。
  - 每次開桌都會重試轉換，但只在組合乾淨時；落入表外就停在 `needs_repair`，不再自動動作。
  - `list_worlds` 先跑恢復，再列出：
    - `worlds/` 下名稱不是合法 id 的目錄不當桌列出。
    - I 不在、但有日誌或 P 的桌，以 `needs_repair` 列出，名稱從 P／S／N 寬鬆讀。
    - `needs_repair` 的畫面說明狀況，並提供「打開資料夾」（既有 opener plugin）。
- **寫入閘門（三層）**：
  - 每桌一把讀寫鎖：會寫桌的 command 進入時取共用許可，持有到最後一次落檔（跨 await）；轉換與改用備份取獨占。
  - 資料層所有寫進桌目錄的動作改走共用寫入函式，寫入當下再查格式標記。
  - 加掃描測試：資料層與 command 層在共用寫入函式之外直接寫檔就失敗；豁免只限非桌目錄（設定、用量記錄等），逐條寫理由。實作者另列全部 command 的「寫／唯讀」分類供驗收。`append_transcript` 讀不到 `state.json` 時改成報錯，不再照寫。
- **舊版看新格式的桌**：
  - `list_worlds` 的 id 取目錄名，name 寬鬆讀（`state.json` 當 JSON 取 `name`，讀不到就顯示 id），帶 `read_only` 狀態，不因解不開而消失。
  - `enterTable` 先依 `open_world` 結果分流：唯讀與需要修復的桌不呼叫任何嚴格讀取（state、transcript、角色、世界書、分支綁定）。唯讀桌只走 `read_world_readonly`：逐行當 JSON 只取 `speaker_name`／`text`／`kind`，回傳略過的行數讓前端註明「有 N 則無法顯示」；幕號讀不到就取編號最大的紀錄檔。
  - 前端：輸入與編輯全關。橫幅寫「這張桌由 X 版建立或轉換，要繼續請更新到 X 版或更新版本」，版本不明時省略 X。有可用備份時加「改用轉換前的備份繼續玩」。
- **改用備份**：`restore_world_backup(world_id)`，只在唯讀、且 `.tt-pre-<id>` 的格式 ≤ 本版、且目錄組合乾淨時可用。日誌 `{op:"restore", stage:"swap", had_newer}` → r1 既有 `.tt-newer-<id>`→trash → r2 `<id>`→`.tt-newer-<id>`（轉換後玩的內容另存）→ r3 `.tt-pre-<id>`→`<id>` → `stage:"cleanup"` 刪 trash、刪日誌。全程只改名，中斷照上表。
- **日後升格式的規則**：新格式要讓 0.2.0 的 `WorldState` 解不開，例如改掉必填欄位，免得手動裝回 0.2.0 時無聲覆寫新欄位。
- **單一實例**：`tauri-plugin-single-instance`，第二次啟動只把既有視窗帶到前面。不做每桌鎖檔。
- **設定檔改成補丁語意**：新 command `update_config(patch)`，補丁以原始 JSON（`serde_json::Value`）接收，`null` 才留得住。`api_keys`／`tier_models`／`preferences` 三張表各自逐鍵處理：沒送的鍵不動、送 `null` 刪鍵、空字串是合法值。後端用一把鎖序列化所有設定寫入（含 `apply_free_bootstrap`——清 `openrouter/free` 要送 `null`——以及 OAuth、smart_free 遷移），以磁碟原始 JSON 為底套補丁，其他頂層與巢狀欄位原樣保留。寫法 `config.json.tmp`（unix 0600）→ 改名。前端各寫入點改成只送本次改動的鍵；`write_config` 移除。
- **實作時補定的細節**（Opus、Sol 驗收通過，〔模型判斷·未裁決〕）：
  - `open_world` 多第五態 `busy`：該桌有寫入在途時不排隊，前端顯示忙碌、不換桌。
  - 刪桌取獨占鎖，拿不到回忙碌；刪除時連同轉換前備份、另存、操作日誌一起刪。唯讀桌目前**可以刪**（待作者裁決）。
  - 恢復途中任何 IO 錯都只把那一桌標 `needs_repair`，不刪 I／P／N，清單其他桌照常。操作日誌用暫存檔＋改名替換。
  - 前端所有 `updateConfig` 在模組層依呼叫順序串行送出；設定檔驗證過才落檔，所有平台 tmp→`config.json` 直接改名。

## 實機驗證（技術上未定，失敗有退路）

- **Mac「App 管理」保護**（macOS 13+）：ad-hoc 版沒有 Team ID，替換自身可能被擋。矩陣：`/Applications`、使用者 `~/Applications`、從 Downloads 直接啟動（App Translocation）、替換後重開；記錄新舊 `.app` 的 quarantine 與簽章狀態。被擋→確認舊 App 還在，再開下載頁。第一次從 DMG 安裝的 quarantine 歸 release-1-mac-signing。
- **Windows**：SmartScreen、Win11 Smart App Control 是否擋未簽章安裝檔；NSIS 降版覆蓋安裝。
- 鎖定 plugin 版本後重查 Mac `install` 失敗是否復原。

## 驗收

- 需要兩個真 release 才能端對端測，排進 [verification-queue](../reference/verification-queue.md)。
- Windows、Mac 各一輪：偵測→一鍵更新→回到上一版→逐版刪除。
- 跨格式轉換後回退：唯讀、提示、改用備份續玩、另存內容都在；舊版不寫壞新格式。
- 失敗情境：遷移中斷／磁碟滿後重開；Windows 降版安裝失敗；Mac 換 `.app` 失敗（通過條件：App 不消失）；兩個實例同開一桌；手動安裝的舊版沒有 updater 產物。
- 離線或連不上 GitHub：不擋啟動、不報錯。

審核出處：Grok session `01a0f23c-d4a7-76f2-99d3-8f64b870db33`、Sol thread `01a0f23e-c90c-7752-8fc2-ddd592b7fc4b`。
