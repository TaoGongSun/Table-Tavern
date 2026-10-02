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
  - 刪桌取獨占鎖，拿不到回忙碌；刪除時連同轉換前備份、另存、操作日誌一起刪。唯讀桌可以刪，但刪除確認窗要加警告「這張桌有比目前版本新的紀錄，刪掉後更新回新版也找不回來」〔作者裁決 2026-10-02〕。
  - 恢復途中任何 IO 錯都只把那一桌標 `needs_repair`，不刪 I／P／N，清單其他桌照常。操作日誌用暫存檔＋改名替換。
  - 前端所有 `updateConfig` 在模組層依呼叫順序串行送出；設定檔驗證過才落檔，所有平台 tmp→`config.json` 直接改名。

## 包 3 做法（Claude、Grok、Sol 三方共識 2026-10-02）

範圍：偵測、下載進版本庫、更新閘門、兩平台安裝、提醒等級、略過此版與自動檢查開關。**畫面全歸包 5**：包 3 只交後端 command／事件與前端 `useUpdateController`，不畫任何提示。「更新前那一版」由包 4 插進安裝流程。

- **plugin**：`tauri-plugin-updater` 2.13.x 註冊在 Rust 端；不裝 JS 套件、不開 capability（前端只呼叫我們自己的 command）。Windows 安裝模式用預設的 passive。`requireSignedVersion` 不開（包 1 已定）。
- **格式等級資訊**：`finalize.mjs` 從 `src-tauri/src/data/format/marker.rs` 解析 `CURRENT_FORMAT`，寫進 `latest.json` 頂層 `format_version`。客戶端從 `Update.raw_json` 讀；欄位缺或讀不懂就當沒有格式轉換。
- **`update_check(manual)`**：呼叫 `check()`。回 `null` 或 `{version, current_version, notes, pub_date, level, skipped}`：
  - `level`：遠端 `format_version` 大於本版 `CURRENT_FORMAT` → `format`；否則版本號最左邊不同的那一位是第三位 → `patch`，其餘 → `feature`。
  - `skipped`：等於偏好 `update_skipped_version`。被略過的版本照樣回傳，提醒與否由畫面決定。
  - 拿到的 `Update` 物件存進 app state，下載與安裝用它。
  - 自動檢查（`manual=false`）任何失敗都回 `null`、只記 log。手動檢查失敗回錯誤字串。
- **檢查時機**：前端 controller 在啟動時呼叫一次，之後每 24 小時一次。偏好 `update_auto_check` 預設開；關了就只有手動檢查。
- **版本庫**（`app_local_data_dir()/versions/`，包 4 的回退、保留 3 版、逐版刪除都讀這個版面）：
  - 每版一個目錄 `<版本>/`：安裝檔、`<檔名>.sig`（`Update.signature` 原文，即 base64 包著的 minisign 簽章）、`release.json`（`version`、`platform`、`file`、`format_version`、`size`、`downloaded_at`）。
  - 檔名取下載網址最後一段，必須符合 `^[A-Za-z0-9._-]+$` 且副檔名對平台（Windows `-setup.exe`、Mac `.app.tar.gz`），否則拒絕。
  - `update_download()`：該版目錄已在且重驗通過 → 直接沿用、不下載。否則用 `Update::download`（plugin 在這步驗簽），進度用事件 `update-progress`；寫進 `.partial-<版本>/`，檔案 fsync → 舊的同版目錄（驗不過的）先改名成 `.trash-<版本>` → `.partial` 改名成正式 → fsync `versions/` → 刪 trash。每次下載開始前先清掉該版的 `.partial-<版本>`、`.trash-<版本>`；啟動時清全部殘留。
  - **驗簽函式**（安裝前重驗與包 4 回退共用）：`.sig` 與 conf 公鑰各自先 base64 解碼，再用 `minisign-verify` 驗；同時核對 `release.json` 的版本與平台。
- **更新閘門**（`world_lock` 與設定鎖共用一個閘門狀態）：
  - 閘門旗標與每桌鎖表放在同一把 mutex 下。開閘＝在這把 mutex 內設旗標、取出當下所有鎖的快照，並呼叫 `notify_waiters()` 叫醒所有等待中的許可；之後新建的鎖與新許可一律拒絕。
  - `world_write_permit`／`_async` 改回 `Result`：取鎖表時查旗標；同步版自旋的每一圈在 `try_read` 前先查旗標，開閘就回錯；非同步版在持有鎖表 mutex 時就建立開閘通知的 `Notified`，再 `select` 同時等讀鎖與通知，開閘就回錯。兩版拿到讀鎖後都再查一次旗標，已開閘就放掉鎖回錯。
  - 設定寫入的閘在共用的 `update_config_with`：拿設定鎖前查旗標，開閘就回錯。`read_config` 觸發的舊設定遷移遇到閘門就只讀不寫。
  - 開閘後依快照逐一非同步等每桌的獨占，最後拿設定鎖，全部持有到程序結束。在途的 AI 回應握著共用許可，閘門會等它結束；前端在 AI 回應時把安裝鈕標成「等回應結束」並附停止鍵（既有 `stopResponse`）。
  - `update_install()` 本身序列化：同時第二次呼叫直接回「已在安裝」。
  - 桌與設定以外的寫入（模型清單快取、用量紀錄、CLI session 檔等）由實作者逐一列出，分成「已在共用許可或設定鎖下」與「要另外擋」兩類，交驗收。
  - 程序退出不會丟掉作業系統快取，寫入函式回傳即算落地，不另做全碟 sync。
  - 失敗放閘：任何在「啟動安裝」之前的失敗（重驗、Mac 替換的任一步、Windows 安裝程式啟動不了）都放開閘門，App 照常可用。
- **Windows 安裝**：用 plugin 的 `Update::install(bytes)`（NSIS `/UPDATE`＋passive，`ShellExecuteW` 成功後本體 `process::exit`）。安裝程式啟動之後的失敗，舊程序已退出、無法當次挽回，由下次啟動與包 4 回退處理。NSIS 偵測到舊程序還沒退完時會先關它再繼續，這段時序列入實機矩陣。
- **Mac 安裝**（不用 plugin 的 `install`：2.13.1 仍是舊 App 先搬進暫存目錄、換新失敗時舊 App 隨暫存目錄一起被刪）：
  1. 找目前 `.app`（`extract_path_from_executable`）。路徑在 App Translocation（含 `/AppTranslocation/`）或上層目錄不可寫 → 不動任何東西，回「無法自動替換」，前端開 GitHub 下載頁。
  2. 同目錄先跑一次殘留整理（見步驟 6）。整理完 `.TableTavern-update.app` 還在就停，回「無法自動替換」。
  3. 解壓到 `.TableTavern-update.app`：tar 第一層是 `Table Tavern.app/`，剝掉這層，讓 `Contents/` 直接落在目錄下；解完核對 `Contents/Info.plist` 的版本等於要裝的版本。
  4. `renamex_np(新, 舊, RENAME_SWAP)` 原子對調：成功後原路徑是新版，`.TableTavern-update.app` 變成舊版。磁碟不支援對調 → 刪掉解壓物，回「無法自動替換」。
  5. 換下的舊版**不刪**：放進同目錄可見的 `Table Tavern (previous).app`（做法見包 4），新版啟動失敗時玩家還有一個能開的 App。`touch` 新 `.app` 後 `app.restart()`。
  6. 殘留整理（步驟 2 與啟動後）：由包 4「Mac 殘留整理改版」取代。
- **偏好鍵**：`update_auto_check`（bool，預設 true）、`update_skipped_version`（版本字串，略過此版寫入、選了更新的版本就清掉）。
- **測試**：等級判斷、`skipped`、檔名驗證、版本庫沿用／取代／殘留清除、base64 解碼後驗簽（含竄改失敗）、閘門（開閘後新許可與新桌被拒、同步自旋與多個非同步等待者都會因開閘退出、等在途許可放開才完成、失敗放閘、重複安裝被拒）、設定寫入被閘、Mac 流程用假目錄測（剝第一層、版本核對、對調不支援與上層不可寫兩種退路、對調後改名前中斷的舊版被改名成 previous 而非刪除、從 previous 啟動時整理什麼都不做、`post_launch` 只刪較舊版、解壓中斷沒有 plist 的殘留被刪、`0.9.0` 與 `0.10.0` 照 SemVer 比）、`finalize.mjs` 寫 `format_version`。實機另排驗證佇列。

- **實作時補定的細節**（Opus、Sol 驗收通過，〔模型判斷·未裁決〕）：
  - tauri 鎖到 2.12.1（updater 2.13.1 需要），npm `@tauri-apps/api`／`cli` 同為 `~2.12.1`，兩邊要一起升。
  - 待裝版本槽：只有下載中、安裝中算忙碌（檢查不覆寫、不連網）；已下載未安裝照常檢查，遠端換版就改指新版。`update_install` 帶 `update_download` 回的版本核對。
  - 略過鍵在重驗通過、開閘前才清；安裝失敗前端重讀設定。
  - Mac 解壓保留 `mode & 0o777`；只允許解析後仍在 bundle 內的相對連結。
  - 沒握桌鎖的在途 AI（開桌大綱、翻譯開場白、重構建議等）不在安裝等待集合，新呼叫在入口被拒；「一句話開桌」在建桌與補內容之間有空隙，最壞留下一張空桌。

## 包 4 做法（Claude、Grok、Sol 三方共識 2026-10-02）

範圍：更新前那一版、版本清單、回退安裝、自動保留 3 版、逐版刪除、桌備份清單與刪除。**畫面歸包 5**：包 4 只交後端 command 與前端 controller。沿用包 3 的版本庫版面、驗簽函式、更新閘門、安裝序列鎖、Mac 替換與殘留整理；包 3 的 Windows 正向更新（plugin `Update::install`）不動。

- **版本庫操作鎖**：版本庫的寫入、修剪、刪版、回退的「重驗到啟動安裝」共用一把鎖，與包 3 的下載提交同一把。
- **回退點（更新前那一版）**：`ensure_rollback_point()` 確保「目前這一版」在版本庫。
  - `update_download` 不論新版是剛下載還是沿用，最後都呼叫它；它的失敗不往外傳，`update_download` 照常成功並另回 `rollback_ready`（畫面據此寫「沒有回退點」）。下次再按會再補。
  - 來源：GitHub release `v<目前版本>` 的安裝檔與 `.sig`，網址用包 1 的固定檔名與 conf `endpoints` 推出的 repo 網址；照包 3 寫法進版本庫並驗簽，`release.json` 的 `format_version` 填本版 `CURRENT_FORMAT`。已在且重驗通過就沿用。
- **「上一版」記錄**（`versions/previous.json`）：
  - 正向更新重驗通過、開閘之前，寫入待確認 `{pending: {from: 目前版本, to: 目標版本}}`，原本已確認的 `previous` 保留不動。
  - 啟動後的 `update_post_launch()`（兩平台都跑）：執行中版本＝`pending.to` → `previous` 改成 `pending.from`；執行中版本＝`pending.from`（安裝沒成功）或兩者都不是（中途改走回退、手動裝了別版）→ 只清掉 `pending`。已確認的 `previous` 等於執行中版本時清掉。
  - 回退不寫 `previous`（一鍵「回到上一版」只服務正向更新之後；回退後想回較新版走一般更新）。
  - `list_versions` 的 `previous` 與一鍵「回到上一版」只認已確認的 `previous`（且該版在版本庫、可用），不用 SemVer 猜。
- **可回退的條件**：版本庫裡的版本要回退，`release.json` 必須讀得懂、平台相符、帶 `format_version`、版本低於目前。版本庫只由本程式寫入：新版來自 `latest.json`（必定比執行中的新），回退點是執行中這一版（本身帶格式檢查），0.2.0 這類沒有格式檢查的版本沒有寫入路徑。
- **`list_versions()`**：每版 `{version, size, format_version, usable, current, previous}`＋總占用。讀不懂的目錄照列大小、`usable: false`，讓玩家能刪。
- **`rollback_preview(version)`**：逐桌用包 2 的格式判讀（缺標記時解 `state.json`），回兩組：格式大於目標版 `format_version` 的「會變唯讀」，判讀為版本不明的「可能唯讀」；都附 id 與寬鬆名稱。
- **`rollback_install(version)`**：
  1. 拿版本庫操作鎖；核對可回退條件並重驗（包 3 驗簽函式＋`release.json` 版本平台核對）。
  2. 重驗通過、開閘之前：目前版本寫進 `update_skipped_version`（被退掉的那版自動略過）。
  3. 走包 3 同一個閘門與安裝序列鎖。
  4. Mac：包 3 的替換流程（解壓核對的版本＝目標版）。Windows：自寫啟動——安裝檔複製到 `versions/.launch/`（不自動刪，下次啟動才清），以 `/P /UPDATE /R` 啟動（plugin passive 的參數，不帶 `/ARGS`），啟動成功就 `process::exit(0)`。`allowDowngrades` 用 Tauri 預設（true），降版覆蓋列入實機矩陣。
  5. 啟動安裝前的失敗一律放閘，App 照常可用。
- **Mac 殘留整理改版**（取代包 3 的版本比較規則）。解壓殘留與救援副本分開處理，都只在「從正式位置啟動」時動手；從 `previous` 或其他位置啟動什麼都不做；bundle id 不是本 App 的一律不動。
  - **替換紀錄** `.TableTavern-update.json`：`{from, target, stage}`，每次改寫都 fsync 檔案與上層目錄。安裝步驟 2 的整理之後、解壓之前寫 `stage: "extracting"`；主對調成功後改 `"swapped"`；舊 App 放進 previous 的對調成功後改 `"previous_swapped"`；刪完被換出的舊副本才刪紀錄。
  - **`.TableTavern-update.app` 依紀錄處理**（啟動整理與安裝步驟 2 都跑；讀不到 `Info.plist` 的一律當解壓殘留刪掉）。`.TableTavern-update.app` 不在 → 紀錄已沒有要辨認的對象（每次改名都是原子的，沒有半套狀態），直接刪紀錄。依推斷改走別的階段處理之前，先把紀錄的 stage 改成那個階段並 fsync，再動目錄。
    - `extracting`：版本＝`target` 且執行中不是 `target` → 沒換上的解壓物，刪。版本＝`from` 且執行中是 `target` → 主對調已成功、只是階段沒寫上，照 `swapped` 處理。其他 → 無法判定。
    - `swapped`：版本＝`from` → 放進 previous（見下一條）。版本不是 `from`、但 previous 的版本＝`from` → previous 對調已成功、只是階段沒寫上，照 `previous_swapped` 處理。其他 → 無法判定。
    - `previous_swapped`：previous 的版本＝`from` → 點開頭那份是被換出的舊救援副本，只重試刪除，不再對調。其他 → 無法判定。
    - 沒有有效紀錄：版本等於執行中 → 刪；不等於 → 無法判定。
    - 無法判定：`.TableTavern-update.app` 與 previous 都不動，紀錄保留，下一次安裝在步驟 2 停下回「無法自動替換」，提示玩家打開該資料夾手動處理。
  - **放進 previous**（安裝步驟 5 與整理共用）：previous 不在 → 直接改名，成功就刪紀錄；已在 → `RENAME_SWAP` 對調，紀錄改 `previous_swapped`，再刪點開頭那份、刪紀錄。任何一步失敗，看得到的 previous 都還在，下次依紀錄階段接續。
  - **`Table Tavern (previous).app`（救援副本）**：安裝步驟 2 不刪（步驟 5 才由新換下的那份取代）。啟動後只有同時滿足才刪：前端初始載入完成（設定與桌清單都讀成功，`update_post_launch` 改在這之後才呼叫）、且版本庫裡有它那一版可用的回退點。少一個就留著。
  - 已知殘餘：新版初始載入正常、之後才壞到連設定頁的回退都用不了，玩家要到 GitHub 下載頁手動裝舊版（版本庫的 `.app.tar.gz` 不能雙擊）。
- **自動保留 3 版**：每次版本庫新增一版後修剪：目前版本、已確認的 `previous`、`pending` 的兩版、待安裝的新版、正在回退的目標永遠不刪；其餘依 SemVer 由新到舊補到總數 3 版為止，多的刪掉。修剪失敗只記 log。
- **`delete_version(version)`**：拿版本庫操作鎖後永久刪除；目前版本、正在下載或安裝的版本拒絕。刪 previous 不禁止（警告由畫面負責）。
- **桌備份**：
  - `list_world_backups()`：每桌的 `.tt-pre-<id>`（轉換前備份）與 `.tt-newer-<id>`（改用備份時另存的內容）各一列：`{world_id, name, kind, size, format_version, deletable}`＋總占用。主資料夾不在、或目錄組合不乾淨（包 2 的 `combo_clean`）的桌，列出但 `deletable: false`，並標「需要修復」。
  - `delete_world_backup(world_id, kind)`：取該桌獨占（拿不到回忙碌）、`combo_clean` 成立才永久刪；否則拒絕。刪 pre 後該桌不再有「改用備份」；刪 newer＝永久失去改用備份後另存的內容（確認文案由畫面負責）。
- **前端**：`useVersionStoreController`（清單、預覽、回退、刪版、桌備份清單與刪除），回退在 AI 回應中同樣進等待。不畫畫面。
- **測試**：回退點（剛下載與沿用兩條都會補、拿不到不擋、下次再補）、`previous.json` 的待確認與確認（新版啟動成功才轉正、安裝失敗保留原紀錄、回退不寫、等於執行中版本時清掉）與一鍵上一版只認紀錄、可回退條件（缺 `format_version`、平台不符、同版或較新都拒）、預覽兩組（含缺標記解 state、版本不明）、回退寫略過鍵的時機、Windows 啟動參數組成與 `.launch/` 清理、Mac 殘留整理（替換紀錄每個階段中斷後的接續——含主對調後未寫階段、previous 對調後刪除前；解壓殘留刪除、升降版都對、沒有紀錄時的退路與無法判定時停住；previous 只在初始載入完成且有回退點時刪；安裝步驟 2 不刪 previous；從 previous 啟動不動；bundle id 不符不動）、`pending` 與執行中版本對不上時清掉、修剪保留規則（永不刪的各類＋補到 3 版）、刪版與修剪和回退共用鎖、桌備份清單與刪除（忙碌、主資料夾不在、組合不乾淨都拒）。

- **實作時補定的細節**（Opus、Sol 驗收通過，〔模型判斷·未裁決〕）：
  - `update_download` 回 `{version, rollback_ready}`；回退點網址取 conf 第一個含 `/releases/` 的 endpoint 前段，不跟隨重導向。
  - 清單的「可用」走與回退相同的重驗（`.sig`、版本、平台、大小、簽章）。救援副本刪除前也重驗回退點。
  - Mac 替換紀錄進 `swapped` 時另記要放進 previous 那份 App 的目錄識別（dev＋inode），用來分辨同版副本是否已對調；取不到識別就停下重試。替換紀錄與 `previous.json` 都用暫存檔＋改名改寫。
  - `update_post_launch` 在設定與桌清單都讀成功後才呼叫；每步失敗記 log 繼續。

## 包 5 做法（Claude、Grok、Sol 三方共識 2026-10-02）

範圍：把包 3、包 4 的 controller 畫出來，必要時調整這兩支 controller 的前端狀態。後端只做下面「後端補口」列的幾處回傳調整。文案 i18n 十國補齊。畫面位置先照下面的預設做，位置與為過按鈕寬度選的譯詞移交 ui-redesign 一併重定〔作者裁決 2026-10-02〕。

- **後端補口**（審核時發現的缺口）：
  - `update_check` 改回 `{status: "available" | "none" | "failed", offer?, message?}`：`none`＝確定沒有新版（前端清掉更新資訊）；`failed`＝這次沒查成，帶錯誤原因（前端保留原本的更新資訊與小點，後端待裝槽也不動；自動檢查的失敗不顯示，只有手動「檢查更新」的失敗顯示原因）。前端在預覽、等待、下載、確認、安裝任一階段時，任何檢查結果都不改這些狀態。
  - `rollback_preview`：目標版本不合格（不存在、驗不過、不比目前舊、平台不符）回錯誤；掃桌本身失敗改回成功並帶 `scan_failed: true`。
  - `list_versions` 每列加 `eligible`（可用、而且比目前舊，與回退的資格判斷同一支）。
  - `list_world_backups` 每列加 `directory`（該份備份目錄的絕對路徑），給「打開資料夾」用。
- **啟動順序**：初始載入完成後先呼叫 `update_post_launch`，它完成（成功或失敗）之後才第一次讀版本清單，讓剛確認的上一版讀得到。
- **版本號與小點**：側欄底部設定鈕旁加版本號小字按鈕 `v0.x.y`，點了開設定「版本」分頁。只要知道有比目前新、且沒被略過的版本，版本號旁的小點就一直亮，直到更新或略過。遊戲畫面（對話區、狀態列、卡片介面）不出現任何更新提示。
- **橫幅與對話框**（提醒強度裡「啟動時」的那兩種）：
  - 只由**啟動那次自動檢查**的結果觸發；24 小時檢查或手動檢查找到的新版只亮小點，等下次啟動再提醒（不在玩到一半彈出）。
  - 功能版：側欄桌列表上方橫幅「有新版本 X」＋「查看」（開版本分頁）＋關閉。含格式轉換：啟動對話框，說明這版會轉換桌紀錄、轉換前會自動備份，按鈕「查看」「稍後」。小修只亮點。被略過的版本都不出。
  - 同一版只出一次：橫幅或對話框**實際畫出來之後**才寫偏好 `update_reminded_version`；本次執行內另以記憶體去重。檢查失敗或沒結果時什麼都不寫，下次啟動照常再判斷。
- **設定新分頁「版本」**（主要動作置頂）：
  - **更新區**：目前版本；有更新時顯示新版本號、「更新」鈕（一鍵）、展開可見「更新說明」「略過此版」。含格式轉換的版本，更新鈕旁固定寫「這版會轉換桌紀錄，轉換前會自動備份」。
  - 更新流程：按下→下載（進度條）→下載完成時若 `rollback_ready` 為假，停下來問「這次沒有回退點，仍要更新嗎？」（繼續／取消）；為真則直接安裝。`rollback_ready` 每次按更新時重設，不沿用上次。按取消就回到「有更新」的狀態，可以再按更新。下載完成後（不論接著是安裝、取消或失敗）重新整理版本清單。
  - 錯誤與重試：失敗時保留這次的更新資訊與「重試」，並依重讀的設定更新「已略過」標記（安裝前會清略過鍵）；Mac 回「無法自動替換」時改顯示說明＋「打開下載頁」（GitHub releases）。略過後該版仍列在更新區（標「已略過」，可取消略過），不必等下次檢查。「檢查更新」鈕、「自動檢查更新」開關，開關下一行資料流向：「檢查更新會讓 GitHub 看到你的 IP 與目前版本」。
  - **回退區**：「回到上一版 X」一鍵（只在清單有 `previous` 且 `eligible` 的那列時顯示）；展開為版本清單（版本、大小、可用與否；目前版本那列只標「目前」、不給動作），非目前列都給「刪除」，`eligible` 的列另給「回到這版」（含驗不過的非目前列只給刪除）。清單載入失敗顯示錯誤與「重新整理」，不當成沒有版本。
  - 回退確認窗（一鍵與清單都走）：先跑預覽，判斷順序——預覽回錯誤（目標不合格）→ 只顯示原因與「關閉」，關閉即結束這次流程；`scan_failed` → 寫「無法預先判讀，可能有桌會變唯讀」，可選擇繼續；否則列「會變唯讀的桌」「可能唯讀的桌」，兩組都空寫「依目前可判讀的桌紀錄，沒有發現會變唯讀的桌」。可繼續的那扇另寫「退回後，目前的 Y 版會被標為略過，不再提醒」。
  - 刪版確認窗：「只刪程式，不動桌紀錄與備份」；刪的是上一版時加「會失去一鍵回到上一版」。
  - **儲存空間區**：版本庫總占用；桌備份清單（桌名、種類、大小），可刪的列「刪除」＋確認窗——轉換前備份：「刪掉後這張桌不能再改用轉換前的備份」；另存：「這份是改用備份後另存的內容，刪掉就沒了」。不可刪的列標「需要修復」並附「打開資料夾」（用該列的 `directory`）。
- **AI 回應中與互斥**：
  - 更新與回退共用一個前端忙碌狀態：任一個在預覽、等待、下載、確認、安裝中，另一個不能開始，版本分頁其他動作鈕停用；當下流程自己的「取消等待」「停止回應」與確認窗的「繼續／取消」仍可按。
  - AI 回應中按更新或回退：顯示「等目前的回應結束後開始」＋「停止回應」（`stopResponse`，停止後會接著更新／回退）＋「取消等待」。等待狀態在 App 層，關掉設定頁不會取消，只有「取消等待」會。
- **錯誤顯示**：更新與回退的錯誤都顯示在版本分頁對應區塊內；後端「更新進行中，暫停寫入」這類寫入錯誤照現有錯誤條。
- **測試**：vitest 覆蓋提醒判斷（等級 × 略過 × 已提醒 × 是否啟動檢查）、小點條件（`none` 清、`failed` 保留；自動失敗不顯示、手動失敗顯示原因；流程進行中檢查結果不改狀態）、啟動整理完成後才讀清單、只出一次與寫入時機、`rollback_ready` 重設與確認、錯誤保留更新資訊、略過與取消略過、更新與回退互斥、等待與取消等待、回退確認文案分支（目標不合格只能關閉、先判 `scan_failed` 再判兩組空）、一鍵上一版要求 `eligible`、`eligible` 與刪除鈕、下載後刷新清單、取消後可再按更新、清單載入失敗；後端四處補口各有 Rust 測試；i18n 檢查與按鈕寬度檢查要過。GUI 實測等作者在場。

- **實作時補定的細節**（Opus、Sol 驗收通過，〔模型判斷·未裁決〕）：
  - 自動檢查被關或流程中槽是空的，`update_check` 回 `failed` 不回 `none`，免得清掉手上的更新資訊。24 小時檢查遇到流程進行中直接不打。
  - `update_download(version)` 帶玩家看到的版本；槽已被後來的檢查換掉就回「要更新的版本已經換了」不下載，前端進 `rechecking`（算忙碌）重新檢查、顯示最新那版讓玩家再按，過期結果丟棄。
  - `rollback_preview` 多做一次重驗簽，驗不過算目標不合格。
  - 「已略過」一律看目前設定的 `update_skipped_version`，不看 `offer.skipped`。`none` 或換成別版時收掉指向舊版的啟動提醒。回退失敗與更新失敗一樣重讀設定。
  - 確認窗都用 plugin-dialog 原生視窗。設定頁已開著時外部要求切分頁，先經未儲存變更確認。
  - 文案放主字典讓按鈕寬度檢查涵蓋；為過寬度，法文「更新」用 Installer、德文 Updaten、重試用 Repetir（西葡）／Nochmal（德）。

## 實機驗證（技術上未定，失敗有退路）

- **Mac「App 管理」保護**（macOS 13+）：ad-hoc 版沒有 Team ID，替換自身可能被擋。矩陣：`/Applications`、使用者 `~/Applications`、從 Downloads 直接啟動（App Translocation）、替換後重開；記錄新舊 `.app` 的 quarantine 與簽章狀態。被擋→確認舊 App 還在，再開下載頁。第一次從 DMG 安裝的 quarantine 歸 release-1-mac-signing。
- **Windows**：SmartScreen、Win11 Smart App Control 是否擋未簽章安裝檔；NSIS 降版覆蓋安裝。
- 鎖定 plugin 版本後重查 Mac `install` 失敗是否復原。

## 驗收

- 需要兩個真 release 才能端對端測，排進 [verification-queue](../reference/verification-queue.md)。包 5 介面的功能實測併在這一輪；第一個帶更新功能的正式版發出前必須驗過〔作者裁決 2026-10-02〕。
- Windows、Mac 各一輪：偵測→一鍵更新→回到上一版→逐版刪除。
- 跨格式轉換後回退：唯讀、提示、改用備份續玩、另存內容都在；舊版不寫壞新格式。
- 失敗情境：遷移中斷／磁碟滿後重開；Windows 降版安裝失敗；Mac 換 `.app` 失敗（通過條件：App 不消失）；兩個實例同開一桌；手動安裝的舊版沒有 updater 產物。
- 離線或連不上 GitHub：不擋啟動、不報錯。

審核出處：Grok session `01a0f23c-d4a7-76f2-99d3-8f64b870db33`、Sol thread `01a0f23e-c90c-7752-8fc2-ddd592b7fc4b`。
