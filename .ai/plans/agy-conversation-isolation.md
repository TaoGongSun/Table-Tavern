# agy-conversation-isolation — 實作方案

## 拍板

A1：假 HOME＋symlink 到真的 `~/Library/Keychains`，共用登入。原話：「假 HOME＋symlink 到真的 ~/Library/Keychains，共用登入——終端機登入一次 app 就能用；代價是終端機 /logout 會連 app 一起登出」〔作者裁決 2026-10-28〕

## 做法（三方共識 2026-10-28：主線、Sol、獨立 Opus 審查，有條件同意）

### 假 HOME 位置
`<config_root>/agy-home`，0700。不跟 grok 共用 `cli-home`：那裡放了 keychain symlink 會讓 grok 摸得到使用者 keychain，破壞 grok「不共用登入」的拍板。測試通道 `--root` 的 config_root 不同，對話自然落在測試 root 內。

### 環境（`cli::agy_envs(home)`，比照 `grok_envs`）
- `HOME`、`USERPROFILE` ＝ agy-home。
- `AGY_CLI_DISABLE_AUTO_UPDATE=true`（1.4.8 實測只認 `true`：設 `1` 或不設，全新假 HOME 跑 `agy models` 都會在 log 記 `Spawned background update process`，設 `true` 沒有）：HOME 被換時自動更新落點未定，更新交給使用者終端機與安裝腳本。驗收＝假 HOME 單發後沒有更新寫檔；不宣稱所有更新分支都已停用。

### 真 HOME
另寫只讀 `HOME` 的函式（不沿用 `claude_home_dir()`，它先讀 `CLAUDE_CONFIG_DIR`）；空值或相對路徑回錯。

### symlink（`agy_profile`）
- 只在 macOS，只連 `agy-home/Library/Keychains` → `<真HOME>/Library/Keychains`；其他一律不共用。
- 目標須存在且為目錄，否則 `AgyProfileFailed` 附路徑。
- 每次組 agy 環境時就地確保：`symlink_metadata` 分辨不存在與斷鏈；不存在、斷鏈或指向別處 → 同目錄、每次唯一的臨時名建 symlink 再 `rename` 蓋上，失敗清掉臨時檔；已正確 → 不動。
- 原位是實體檔／目錄 → 回錯，附完整路徑與「把它移到別處保留後再試」的指引，不引導刪除。
- 任何失敗都回錯，不退回裸跑（退回就又寫進使用者 ~/.gemini）。

### 接上呼叫點
`cli_envs` 加 `"agy"` 分支，涵蓋設定頁模型清單（`agy models`）、mac 安裝腳本的探針那行（安裝那行刻意不帶前綴）、Windows `InstallSpec` 的 probe。另改兩處繞過它的：lane 續聊的 `LaneProvider::Agy => Vec::new()`；單發 `"agy"` 分支傳 `&[]`（旁白單發、生圖、agy 下的卡重構都走這條；卡重構的 session 路徑只有 claude）。

- 模型清單（`commands/settings.rs` 的 `list_cli_models`）：profile 失敗不可靜默回空，至少 warn 附原因；`cli/detect.rs` 的 `hidden_output`（跑 `agy models`）spawn 前同樣檢查 HOME。
- agy-home 建立或收 0700 失敗都附路徑回 `AgyProfileFailed`，不忽略。
- `run_cli_cancellable` 加最後防線：program 檔名是 `agy`／`agy.exe` 時，envs 必須顯式帶非空、絕對的 `HOME`，否則 spawn 前回錯（只防漏網）。
- `agy --version`（`probe_cli`）不帶：實測它在空 HOME 下不寫任何檔。

### 測試通道
`harness/root.rs` 的 `reject_symlinks`（兩處呼叫共用）照常掃整個 root（含 `config/agy-home`），只放行兩種連結：
- (a) `agy-home/Library/Keychains`：連結值以所在目錄解析後與 `<真HOME>/Library/Keychains` 兩邊 `canonical_lenient` 比對相同。
- (b) agy-home 裡、目標 canonical 後仍在 agy-home 內的連結（agy 1.4.8 自建 `.gemini/antigravity-cli/cli.log` → `log/cli-*.log`）。

agy-home 本身與 `agy-home/Library` 是 symlink 照拒（即使指向 agy-home 內）；agy-home 裡指向外部（root 外，或 root 內 agy-home 外）的目錄／檔案連結照拒。不變式：root 內除了 (a) 那一條，沒有指向 agy-home 外的連結。〔模型判斷·未裁決〕
清理（`--fresh`）用不跟隨連結的 metadata 判斷、連結只刪連結本身：agy-home 內指向 root 外的連結清掉後，外部檔與真 keychain 檔都還在（測試釘住）。

### 舊 lane
舊 conversation id 在新 HOME 會靜默開新對話 → runner 回 `AgyConversationMismatch` → lanes 的 `Err(_) if !opening` 走 `Reopen{ResumeFailed}` 全量重開。不另做遷移；每條舊 lane 一次性多一通（孤兒對話落在 agy-home）。

### 已寫進使用者 ~/.gemini 的 app 對話
app 不處理：讀改使用者對話庫違反隱私紅線，且可能與使用者執行中的 agy 併寫。作者要清就用 agy 自己的介面處理。〔模型判斷·未裁決〕

### 已知行為變更
使用者 HOME 下的 agy 設定（預設模型、settings 等）不再自動套用到 app；app 的模型選擇照 app 設定頁。檔位選「CLI 預設」（不帶 `--model`）時跑的是假 HOME 裡 agy 自己的預設：1.4.8 全新假 HOME 實測是 Gemini 4 Argon (Medium)（agy log 的 selected model override；stream-json 不帶模型欄），可能與使用者終端機的預設不同。

### Windows／Linux
- Windows：設 HOME＋USERPROFILE，不建 symlink。Credential Manager 依使用者不依路徑，預期登入共用；若實際走 `auth.json` 檔案憑證就會變成 app 另行登入。Go 的 `UserConfigDir` 在 Windows 是 `%AppData%`，agy 是否把對話放那裡待證；要的話只對 agy 子程序再設 `APPDATA` 後重驗。真 profile 新檔只回筆數。排 verification-queue。
- Linux：不是發行目標，沿用同一組環境，不另處理。

## 測試

- 單元：`agy_envs` 內容；真 HOME 空值／相對路徑回錯；symlink 確保：新建、冪等、重指錯誤目標、斷鏈重建、目標不存在回錯、原位是實體目錄回錯且不動它；`run_cli` 對 agy 缺 HOME 拒 spawn；harness 白名單放行恰好那條、祖先 Library 是 symlink／同位置指別處／別位置照擋。
- 測試通道（獨立 `--root`）：①單發一回合 → `<root>/config/agy-home/.gemini/antigravity-cli/conversations/` 出現新 `.db`；②同 lane 續聊同 id；③lane id 換成假 id → 一次 mismatch 後重開回話正常；④模型清單抓得到；⑤同 root 重開不被擋；⑥假 HOME 單發後無更新寫檔。
- `~/.gemini` 反證：實測前建標記檔，實測後 `find ~/.gemini -newer <標記> -type f | wc -l` 只回筆數並確認 find 成功；0 只代表範圍內無新檔。不看檔名與內容。〔模型判斷·未裁決〕
- `npm run verify` 綠。

## verification-queue（施工時登記）
- 首次登入（〔作者裁決 2026-10-29〕0.4.0 放行、列已知限制）：乾淨機器從未登入 agy → app 內登入 → 終端機 agy 可直接用。不要求作者機登出。
- Windows 對話落點與登入共用。
- agy 升版：重驗假 HOME 落點與登入，看 `--help` 有無資料目錄旗標。

agy-home 對話清理與上游資料目錄需求另案 agy-home-cleanup，本案不做。
