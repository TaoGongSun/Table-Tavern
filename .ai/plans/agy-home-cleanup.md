# agy-home-cleanup — app 專用 CLI 家目錄（agy／grok／codex）的清理

狀態：方案第 5 版三方共識（Sol 串 01a18196-444c-76d3-a9ce-b9d5fb752043、opus-review）。包 1 已三方驗收通過（在分支上，整案結案才進 main）；包 2 等 codex-lane-resume 合入 main。

## 拍板〔作者裁決 2026-10-28〕
1. 保留天數 7 天。
2. grok 用自帶的 `storage.cleanup_ttl_days`，由 app 寫進自己 grok-home 的 `config.toml`。
3. `prompt_history.jsonl` 每次 grok 呼叫結束就刪。
4. 加做即時刪：agy 重開線時刪舊對話，單發呼叫一結束就刪。

「單發一結束就刪」本方案套用到 agy 與 grok 兩家的文字單發（grok 單發的 session 沒有抹寫，留著就是私設原文）；生圖呼叫例外，見下〔模型判斷·未裁決〕。

## 範圍
- agy-home（`<config_root>/agy-home`）、grok-home（`<config_root>/grok-home`）、codex-home（包 2）。
- 不碰使用者自己的 `~/.gemini`、`~/.grok`、`~/.codex`、`~/.claude`。claude 的 session 檔在使用者 `~/.claude`，照既有 retire 規則，不在本案；`refactor_ai/session.rs` 只開 claude session，不在本案。
- grok 的 `session_search.sqlite` 納入（主線判斷〔模型判斷·未裁決〕）：在途 grok 程序歸零時整組刪掉，理由與 prompt_history 同類。
- 與 grok-image-original-leftover 不衝突：生圖呼叫（`allow_cli_tools=true`）本案不即時刪，由那案在複製成功後刪；生圖失敗留下的交給 7 天回收。兩邊都是冪等刪除，目標不存在當已刪。

## 查證依據（2026-10-28）

### agy 1.4.8
- 沒有刪除對話的子命令、旗標、保留期設定；沒有資料目錄變數（agy 自答，`agy --help` 核對）。
- 一個對話的檔：`.gemini/antigravity-cli/` 下 `conversations/<id>.db`、`.db-wal`、`.db-shm`、`brain/<id>/`、`annotations/<id>.pbtxt`、`presence/<id>.lock`。`implicit/<uuid>.pb` 的 uuid 與對話 id 不同（實測），不屬於對話。
- 大小：乾淨結束約 0.4–1 MB；留下 WAL 的可到 9 MB。測試通道 45 分鐘 15 個對話＝81 MB。
- 刪檔後 `--conversation <已刪 id>`：rc=0、警告 not found、另開新 id 照常回覆（實測）→ 既有 `AgyConversationMismatch` 擋下重開，白燒一次。所以 lanes.json 還指著的對話不刪。
- 共用索引（`conversation_summaries.db`、`jetbox_summaries_proto.pb`、`cache/last_conversations.json`）留孤兒列；刪後開新對話正常（1 個樣本）。
- `log/cli-<時間>.log` 每次呼叫一份（約 25 KB），`cli.log` 是指向最新那份的 symlink；agy 不自己輪替。

### grok 1.0.70
- `storage.cleanup_ttl_days` 寫進 `$GROK_HOME/config.toml` 才生效：headless `-p` 啟動時觸發、跨所有群組，閒置超過天數的 session 目錄整個刪掉（實測兩次）。`GROK_CONFIG` 不收 `storage.*`（實測沒刪）；`$GROK_HOME/managed_config.toml` 會被 grok 自己刪掉（實測）。
- config.toml 先寫：只含 `[storage]` 的 config.toml 跑一次 `-p` 後，grok 照常補上自己的 `[marketplace]` 段、保留我們的 `[storage]`（實測）。全新 grok-home 未登入時 `inspect`、`sessions list` 都不寫 config.toml，無法在空家做到 agent 初始化，以「只含 storage」這個實測代替。
- 正在用的線與 TTL（審查第 7 項，實測）：① 一個 57 天前的 session 正在 `-r` 續聊，1.5 秒後另一個呼叫觸發跨群組回收 → 續聊成功、目錄還在；② 單獨 `-r` 續聊一個 57 天前的 session（它自己的啟動也會觸發回收）→ 成功、目錄還在；③ 同條件下沒人續聊的對照 session 被刪。推論：續聊一啟動就更新閒置時間，回收不會刪到正在續聊的線。沒驗到的時序：回收先跑完、續聊才開始——那時 session 已刪，續聊回 rc=1 remote 404，開頭就失敗不燒模型，走既有 ResumeFailed 重開。
- `--resume <已刪 id>`：rc=1、remote 404，開頭就失敗不燒模型。
- 原生清理不碰 `sessions/<群組>/prompt_history.jsonl`：每次呼叫追加 prompt 全文（含私設），lane 抹寫掃不到，官方沒有上限或關閉設定。app 不用 grok 的 prompt 歷史。
- `session_search.sqlite`（實測）：grok 每次啟動（含 `-p`）從磁碟重建索引（`last_bootstrap_at` 跟著更新）。① 直接刪 session 目錄後下一次呼叫，該 id 的列不見；② TTL 刪掉的 session，列也不見；③ 改寫 session 檔拿掉標記、再續聊一次，索引是改寫後的版本。但 ④ 一輪結束、抹掉 session 檔裡的標記後**不再啟動 grok**，索引列仍含標記（原始檔 `strings` 也找得到）——兩次重建之間索引留著抹寫前的內容，最後一輪之後會一直留著。⑤ 刪掉 `session_search.sqlite` 後跑一次 `-p`：rc=0，索引自動重建（20 列）。⑥ 把索引檔寫壞（`integrity_check` 報 malformed）再跑 `-p`：rc=0 照常回覆，grok 把壞檔改名成 `session_search.sqlite.corrupt.<時間戳>`（連同 `-wal`、`-shm` 的 `.corrupt.<時間戳>`）、重建新索引——改名的壞檔留著舊內容，要一併刪。另有 `session_search.sqlite.lock`，是 grok 的鎖檔，不刪。
- `.config-init.lock`（實測）：grok 執行檔字串有「`.config-init.lock` timed out waiting for … after 1s」「skipping official marketplace auto-register: failed to acquire init lock」。只含 `[storage]` 的 config.toml 跑 `-p`：不持鎖時 grok 補上 `[marketplace]` 段；外部以 `flock` 或 `lockf` 持有這把鎖時，grok 跳過、不寫 config.toml。結論：grok 用它當 config.toml 的寫入鎖（等 1 秒、拿不到就放棄這次寫入），與 flock 互斥。

### codex 0.164.0
- 沒有 rollout 保留期設定（Sol）；app 的單發已帶 `--ephemeral`，不寫 rollout。
- `codex delete <uuid> --force` 可非互動刪，不收 `--ignore-user-config`（Sol）。沒登入的測試家複本實測：rollout 檔刪了、`state_5.sqlite` 的 threads 列還在、rc=1。有登入的環境要重驗。
- resume 不存在的 id 回 `no rollout found` → 重開（codex-lane-resume 已實測）。

## 包 1：agy＋grok＋prompt_history

### 家目錄定位
- 一律由 config_root 推出：`agy_home_dir(config_root)`、既有 `grok_home_dir(config_root)`。不從 `call.envs` 的 HOME／GROK_HOME 推。
- `LaneCall` 加 `config_root`；lane 與單發的刪除都拿 config_root 推出的路徑。
- 比對 env 只在「本輪 provider」與「被清理的 provider」同一家時做：本輪 envs 的 HOME（agy）或 GROK_HOME（grok）必須與推出的路徑完全相等才刪，不相等或缺少就跳過記 warn。撤銷別家的舊線（例如本輪 grok、清的是舊 agy 線）不看本輪 envs，直接用推出的路徑。
- 測試：agy→grok、grok→agy 切換時舊線照刪；同一家但 env 指向別處時不刪。

### 共用刪除規則（新檔 `cli/home_cleanup.rs`）
- 名稱：只認標準 36 字元 UUID（小寫十六進位＋連字號）。
- 路徑：從家目錄本身一路 `symlink_metadata` 驗到要動的子目錄（agy：agy-home、`.gemini`、`antigravity-cli`、子目錄；grok：grok-home、`sessions`、群組），任一層是 symlink 或不是目錄就跳過、記 warn，不解析。
- 項目：本身是 symlink 只刪連結；檔案 `remove_file`；目錄 std `remove_dir_all`（文件明載不跟隨 symlink，unix 用 openat／unlinkat 防 TOCTOU）。NotFound 當已刪。
- 失敗只記 warn，不擋玩家這一輪。
- grok 的 `remove_session` 改走這套（home、sessions、群組三層都驗）：連結與一般檔不是群組、可略過；群組或候選讀不到就不刪（判定不了 id 是否唯一）。

### agy
- `remove_conversation(config_root, id)`：刪上面列的五種檔（後綴白名單 `.db`、`.db-wal`、`.db-shm`、`.pbtxt`、`.lock`，`brain/<id>` 目錄）；其他一律不碰：`implicit/`、`Library/`（keychain 連結）、`.gemini/config/`、`installation_id`、`bin/`、`builtin/`、`cache/`、`crashes/`、summaries 檔。
- 重開線（含 ResumeFailed、mismatch 降級重開）：舊線 provider=agy 時，先從 store 拿掉並 `write_store`，**落檔成功才刪**；落檔失敗回錯、檔留著（磁碟上的 lanes.json 仍指著它）。不照抄 `revoke_grok_lane`（那條落檔失敗也照刪，是 grok 為了不留私設的既定取捨，本案不改）。
- 開線失敗、中止、mismatch 時已經拿到的新 conversation id：當場刪（它從沒寫進 store）。mismatch 的實際 id 由 runner 在檢查前另存一格回傳。
- 文字單發（`transport/dispatch.rs` 的 agy 分支，`allow_cli_tools=false`）：傳 `conversation_id_out` 取 id；drop guard 宣告在 run_cli 的 future 之前（區域變數逆序 drop，future 先 drop、`kill_on_drop` 先送出，guard 才刪），正常返回、出錯、future 被丟棄（中止）都在 guard 的 Drop 裡刪。子程序由既有 `kill_on_drop` 收掉；若刪時子程序還沒結束又寫回檔，留給 7 天回收。沒拿到 id 也留給回收。
- 生圖單發（`allow_cli_tools=true`）不即時刪：回覆裡的圖檔路徑可能在 agy-home 內，交給 7 天回收。

### grok
- TTL 設定（`grok_profile()`，每個 app 行程成功一次就不再做）：
  - 照 grok 的鎖協議：以 std `File::try_lock`（unix 為 flock、Windows 為 LockFileEx）取 grok-home 的 `.config-init.lock`（不存在就建），最多重試約 2 秒；拿不到就略過、記 warn。略過、成功、不需寫都算這個行程做過了，同一行程不再重試（免得每次呼叫等 2 秒），下次 app 啟動才再試。鎖在寫完 rename 後才放。
  - 在 `grok_profile()` 的一次性初始化裡做，同一行程的 grok spawn 都要等它結束（成功或略過）才往下走。
  - `config.toml` 不存在：`create_new` 建一個只有 `[storage] cleanup_ttl_days = 7` 的檔（實測不干擾 grok 之後補自己的段落）；`create_new` 撞到已存在就改走下一條。
  - 存在且是一般檔：用 `toml_edit` 解析，只設 `storage.cleanup_ttl_days = 7`；`storage` 已存在但不是 table、解析失敗、檔案是 symlink → 不寫、記 warn。內容沒變不寫；要寫走同目錄臨時檔＋rename，保留原權限。
  - Windows 上 grok 是否用同一把鎖未驗：拿得到鎖就寫，與 mac 同一套。
  - 測試：另一執行緒持鎖時略過不寫；兩個寫入者並行時只有一個寫入、另一個看到內容已是 7 而不寫；grok 自己的段落保留。
  - `toml_edit` 加為直接依賴（Cargo.lock 已有 0.25）。
- 文字單發（`allow_cli_tools=false`）：改帶 `-s <app 產生的 uuid>`，結束（含中止）時交給 grok 閘：沒有 grok 在跑就立刻刪，還有在途程序（被中止的單發可能還沒收屍）就等在途歸零才刪。lane 照既有 `revoke_grok_lane`（家目錄定位改照上面）。
- 生圖單發（`allow_cli_tools=true`）不即時刪 session：`image.rs` 可能讀 `sessions/…/images/*.jpg`，由 grok-image-original-leftover 在複製成功後刪。
- prompt_history：每次 grok 呼叫（單發、lane、生圖都算）結束，在同一個 drop guard 裡刪 `sessions/` 下各群組的 `prompt_history.jsonl`（只認這個檔名）。並行的另一個 grok 若正在追加，寫進的是已刪的 inode，下次呼叫結束再刪。
- session_search.sqlite：不能在有 grok 開著它時刪（SQLite 官方 How To Corrupt：刪掉使用中的 -journal／-wal 會壞庫）。做法：
  - 新檔 `cli/grok_gate.rs`：以 grok-home 路徑為鍵，app 行程內維護一個閘（Mutex 內含「在途程序數」與「待清理」旗標）。
  - 單一入口：新檔 `cli/spawn.rs` 提供唯一的子程序 spawn helper（吃 `tokio::process::Command` 與 envs，回傳持有 Child 的 guard）。規則：envs 帶 `GROK_HOME` 的一律先進閘拿票；spawn 失敗立刻交回票；guard 負責收屍與交回。現有所有 CLI spawn 改走它：`runner.rs` 的 `run_cli`、`detect.rs` 的 `hidden_output`（含逾時被丟棄的路徑）與 `cli_version`、`install.rs` 的 `run_hidden`、`run_probe`、`run_terminal`。`command.output()` 改成 guard 內 `wait_with_output`。
  - 結構強制：`cli/spawn.rs` 的單元測試掃整個 `src-tauri/src` 的正式碼（略過測試檔與檔尾測試模組）：cli/ 底下出現 `.spawn()`、`.output()`、`.status()`、`std::process::Command` 就紅；cli/ 以外出現 `Command::new(`、`process::Command` 就紅，只放行不帶 GROK_HOME 的固定例外 `inflight.rs`（kill）、`updater/`（codesign）、`harness/` 整檔，`commands/cli_setup.rs` 只放行 `Command::new("open")`（登入開終端機）與它的 import。測試不限 unix，路徑以 components 比對，Windows CI 也擋。verify 不跑 clippy，所以不用 `disallowed_methods`。
  - 退出：票只在子程序真的被 wait 收屍完成後才交回。正常路徑在 `child.wait()` 回來後交回；`run_cli` 的 future 被丟棄、出錯提早返回時，持有 Child 的 guard 在 Drop 裡把 Child 移進一個 tokio 任務：`start_kill` → `wait().await` → 交回票。`kill_on_drop` 送出終止不算退出。確認不了已退出（沒有 runtime、`wait` 回錯、收屍任務被 runtime 中止）時票不交回（`Ticket::leak`）：在途數不歸零，這個行程不再清索引與延後的 session；索引等下次 app 啟動回收，延後的 session 靠 grok 的 7 天 TTL。
  - 交回時：在途數減一並設「待清理」；若在途數變成 0 且待清理，就在持有 Mutex 的狀態下依序刪 `-journal`、`-wal`、`-shm`、主檔（只認這四個檔名，套共用刪除規則），另刪 `session_search.sqlite*.corrupt.*`（grok 改名留下的壞檔，名稱以 `session_search.sqlite` 開頭、含 `.corrupt.`）；`session_search.sqlite.lock` 不刪。刪完清掉旗標再放開 Mutex。新的 `enter` 要等 Mutex，所以刪除期間沒有新 grok 能啟動。
  - 還有程序在跑時只留著旗標，由最後一個退出的負責刪；旗標在刪除成功前不清，不會遺失。刪除失敗記 warn、旗標保留，下次歸零再試。
  - grok 下次啟動自動重建索引（實測）。

### 啟動回收（`lib.rs` `.setup()` 背景執行一次，比照 `smart_free::spawn_background_refresh`）
- 順序：先列出 agy 候選 → 再讀 mark → 每組刪前重新 stat 一次。
- mark（只讀、不吞錯）：
  - 用純讀的 `discover_ids` 列舉所有桌 id；每個 id 的本體與 `.tt-pre-`、`.tt-staging-`、`.tt-newer-`、`.tt-trash-` 副本，加上 `.tt-reset-<id>/worlds/<id>/` 裡的 `lanes.json` 都讀。
  - 待修復判定用唯讀檢查：把 `data/format/commit.rs` 的 `assess`（只讀 presence 與交易紀錄，不取鎖、不復原）開成 crate 內可用。讀 lanes.json 前後各跑一次 assess：任一次 `needs_repair`、任一次看到交易紀錄檔、或前後兩次結果不同，就停。不呼叫 `recover_for_list`（它會取獨占鎖並實際復原），背景清理不改任何桌子的資料。
  - 只有 NotFound 算空（目錄存在與否用 `try_exists`，權限或 I/O 錯誤不當成不存在）。列舉失敗、讀取或解析失敗、判定不了、有桌待修復，這一輪 agy sweep 一律停。
  - 不用 `read_store`（它讀失敗回空）。
  - 收全部 session_id，不篩 provider。
- agy sweep：`conversations/`、`brain/`、`annotations/`、`presence/` 下名稱合格（UUID＋白名單後綴，不認得的後綴跳過）的項目依 id 歸組。不在 mark、且組內所有項目（目錄遞迴、不跟隨連結）的 mtime 都早於 7 天才刪。組內任何列舉、metadata、mtime 讀取失敗，整組保留。
- agy log：刪 `log/cli-*.log` 早於 7 天的一般檔；不刪 `cli.log` 與它指向的那份。
- grok：刪各群組的 `prompt_history.jsonl`（補崩潰漏掉的）；透過 grok 閘（在途數為 0 時拿著 Mutex）刪 `session_search.sqlite` 整組，處理崩潰或刪除失敗的殘留；session 交給原生 TTL。

### 與進行中呼叫的協調（為什麼不另加鎖）
- 回收只刪「不在 mark、而且 7 天沒動」的組。正在進行的呼叫只會碰到三種對話：
  - lane 續聊的舊對話：在 lanes.json 裡，被 mark 保住。續聊期間 lanes.json 不會拿掉它：同桌 lane 鎖串行，只有重開時才換 id，而重開時由 lane 自己刪。
  - lane 剛開的新對話：還沒寫進 lanes.json，但 mtime 是現在，7 天門檻擋住。
  - 單發的對話：mtime 是現在，同樣擋住；結束後由 drop guard 刪。
- 唯一會誤刪的是「7 天沒動、又沒有任何 lanes.json 指著、卻有人要用」的對話：只發生在從備份還原的舊桌（見已知限制），代價是一次 mismatch 重開。
- 每組刪前重新 stat，候選列出後才被動到的組不刪。
- 測試：舊對話（mtime 早於 7 天、在 mark 裡）正在被假 CLI 續聊時觸發回收 → 不刪；同一組不在 mark 時 → 刪前重新 stat 看到新 mtime → 不刪。

### 測試
- 單元（tempdir）：
  - UUID 與後綴白名單；agy 五種檔刪除、`implicit/` 不動。
  - agy：家目錄、`.gemini`、`antigravity-cli`、子目錄任一層是 symlink 時整個不刪，外部哨兵檔留著；項目是 symlink 時只刪連結、目標留著；`Library/Keychains` 連結與目標都留著。
  - grok `remove_session`：home、sessions、群組任一層是 symlink 時不刪，外部哨兵留著。
  - 家目錄比對：envs 的 HOME／GROK_HOME 與推出的路徑不同時不刪。
  - mark：本體與四種副本都讀；lanes.json 壞掉、讀不到、列舉失敗、有桌待修復時整輪不刪；NotFound 當空。
  - sweep：mark 內的舊對話不刪、7 天內的新對話不刪、組內 metadata 失敗整組保留、刪前重新 stat。
  - log 規則；config.toml 新建、合併保留 grok 段落、內容不變不寫、symlink 不寫、storage 不是 table 不寫、解析失敗不寫、權限保留；prompt_history 只刪該檔名。
- lane（既有假 CLI 測試）：agy 重開後舊對話檔不見、store 落檔在先；落檔失敗時舊檔留著；ResumeFailed 與 mismatch 同樣處理，mismatch 拿到的新 id 也刪。
- 單發：drop guard 在正常、出錯、future 被丟棄三種情況都刪；生圖呼叫不刪 session、但刪 prompt_history。
- grok 閘：A 結束、B 還開著 → 不刪、旗標留著；B 結束 → 刪；C 在刪除期間 `enter` → 等到刪完才拿到票；future 被丟棄 → 收屍完成前不刪、收屍後才刪；最後一次待清理不遺失；刪除順序 -journal、-wal、-shm、主檔；刪除失敗時旗標保留、下次歸零再刪。`.corrupt.*` 壞檔一併刪、`.lock` 留著。
- spawn helper：spawn 失敗立刻交回票；逾時丟棄（`hidden_output` 路徑）收屍後才交回；不帶 GROK_HOME 的不進閘；原始碼掃描測試在新增未走 helper 的 `Command::new(` 時失敗。
- config.toml：略過後同一行程不再嘗試。
- mark：不呼叫 `recover_for_list`、背景清理前後桌子的檔案逐位元相同；`.tt-reset-` 裡的 lanes.json 也算進 mark。
- 實測（測試 HOME、Gemini 3.8 Flash Medium；grok 用 grok-auth-root）：agy 單發後 conversations 不留該 id；agy lane 重開舊對話被刪；grok 單發後 session 目錄與 prompt_history 不在；grok-home `config.toml` 有 TTL、grok 自己的段落還在。

### 已知限制
- agy 共用索引留孤兒列（1 個樣本無害，長期影響未知）。
- 閒置超過 7 天的 grok lane 被原生 TTL 刪掉，下次走 ResumeFailed 重開一次（不燒模型）。
- 失效的線名（角色刪了、換了模型）留在 lanes.json，它指著的對話會一直被 mark 保住、不回收。
- 從備份還原的舊桌若指著已被回收的 agy 對話，第一輪 mismatch 重開、白燒一次。
- agy 單發若在吐出 conversation id 前失敗，或中止時子程序晚於刪除才退場、又寫回檔，留給 7 天回收。
- grok 程序確認不了已退出時（極少見）：該行程的搜尋索引留到下次 app 啟動回收；延後刪除的單發 session 沒有記下 id，啟動回收刪不到，靠 grok 自帶的 7 天 TTL 清掉。
- 有 grok 在跑時 `session_search.sqlite` 仍可能含抹寫前的內容，在途數歸零才刪；玩家連續呼叫不斷時會延後。
- 行程外的 grok 不在閘的計數內：上次 app 崩潰留下、還在跑的孤兒，以及 mac 登入走 Terminal.app 開的 grok。刪索引時若它正開著，可能壞庫。保底：實測 grok 讀到壞索引會改名成 `.corrupt.*`、重建、照常回覆（rc=0）；改名的壞檔下次在途數歸零時由本案刪掉。最多影響搜尋索引（罕見，輕防）。
- Windows 對話落點未驗（agy-conversation-isolation 已列）：目錄不在就不做事。
- agy 沒有資料目錄變數，繼續靠換 HOME＋keychain symlink；向 Google 提需求不做〔模型判斷·未裁決〕。

## 包 2：codex（codex-lane-resume 合入 main 後）
- 單發已是 `--ephemeral`，不另做。
- lane 重開（含 ResumeFailed）時撤銷舊 thread，落檔成功才刪；啟動回收只動 `sessions/YYYY/MM/DD/rollout-*-<uuid>.jsonl`，mark 與失敗規則同 agy。
- 刪法待開工時在有登入的測試環境重驗：`codex delete <id> --force` 若乾淨（rc=0、threads 列也清）就用它，否則直接刪 rollout 檔＋列已知限制。
- `auth.json`（mac symlink、Windows 硬連結）、各 SQLite、`installation_id` 不在可刪範圍。

## 包 1 實際行數（前 → 後）
| 檔案 | 前 | 後 |
|---|---|---|
| cli/home_cleanup/mod.rs | 新檔 | 154 |
| cli/home_cleanup/agy.rs | 新檔 | 143 |
| cli/home_cleanup/grok.rs | 新檔 | 225 |
| cli/home_cleanup/tests.rs | 新檔 | 647 |
| cli/grok_gate.rs | 新檔 | 261 |
| cli/spawn.rs | 新檔 | 333 |
| data/format/lane_scan.rs | 新檔 | 79 |
| lanes/tests/agy_cleanup.rs | 新檔 | 329 |
| cli/runner.rs | 617 | 618 |
| cli/detect.rs | 236 | 255 |
| cli/install.rs | 938 | 940 |
| cli/mod.rs | 36 | 39 |
| lanes/mod.rs | 968 | 986 |
| lanes/cleanup.rs | 164 | 234 |
| lanes/grok_session.rs | 591 | 577 |
| lanes/store.rs | 71 | 103 |
| transport/dispatch.rs | 734 | 775 |
| lib.rs | 445 | 449 |
| data/format/commit.rs | 991 | 991 |
| Cargo.toml | 78 | 80 |
