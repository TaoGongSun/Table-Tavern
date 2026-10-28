# codex-lane-resume — 實作方案

## 拍板
- B：隔離 `CODEX_HOME`，再用 `auth.json` 連結指回 `~/.codex/auth.json` 共用登入〔作者裁決 2026-10-28〕。作者看過「終端機登出會連 app 一起登出、同時 refresh 撞車那一輪會失敗」後選定，讀作者 auth.json 已授權。
- Windows 用硬連結〔作者裁決 2026-10-28〕。

查證與命中實測數據見 [handoffs/archive/codex-lane-resume.md](../handoffs/archive/codex-lane-resume.md)。審查第 1、2 輪（Sol、opus-review）的必改已併入本檔。

## 1. codex-home（新檔 `cli/codex_home.rs`，比照 `cli/agy_home.rs`）
- **位置**：`<config_root>/codex-home`，權限 0700；建立或收權限失敗時附路徑回 `CodexProfileFailed`。不與 agy-home、cli-home 共用。測試通道 `--root` 自然落在測試 root 內。
- **環境**：`codex_envs(home)` 只設 `CODEX_HOME`，HOME 不動。依 Sol 讀原始碼：
  - 全域 AGENTS.md 從 `CODEX_HOME` 讀，隔離後不再帶入使用者的 `~/.codex/AGENTS.md`（現行單發其實會帶）。
  - `HOME/.agents/skills` 仍會被掃描，但清單不進 prompt。只有正文出現 `$技能名` 才會明確載入，罕見，列入已知限制。
  - shell snapshot 會起 login shell 讀 `.zshrc`，由精簡旗標關掉（見 §2）。
- **真家目錄**：`real_home()` 先讀 `HOME`，沒有再讀 `USERPROFILE`，與 `dispatch::claude_home_dir`、codex 自己的解析一致。兩者都空或是相對路徑就回錯。agy 共用同一個函式，agy 只在 macOS 建連結，不受影響。
- **連結目標**：固定為 `<真家目錄>/.codex/auth.json`。使用者自己在 shell 設的 `CODEX_HOME` 不追〔模型判斷·未裁決〕，列入已知限制。
- **確保連結**：每次組 codex 環境時就地確保。macOS 與 Windows 規則分開如下。

### macOS／Linux：symlink（`ensure_auth_symlink`）
用 `symlink_metadata` 判斷 `codex-home/auth.json`：

| 現況 | 處置 |
|---|---|
| 不存在 | 用同目錄的唯一臨時名建 symlink，再 `rename` 蓋上；失敗就清掉臨時檔 |
| 已是指向目標的 symlink（含斷鏈） | 不動。斷鏈也留著：使用者之後在終端機登入，codex 會穿過去建出目標（實測） |
| 是指向別處的 symlink | 用臨時名重建後 rename 蓋上 |
| 是實體檔或目錄（例如有人用這個 CODEX_HOME 跑過 `codex login`） | 回 `CodexAuthSlotOccupied`，附完整路徑與「移到別處保留後再試」，不刪不搬 |

### Windows：硬連結（`ensure_auth_hardlink`）
硬連結無法標記「這是我們建的」，所以在 `codex-home\.auth-hardlink` 放一個標記檔，硬連結建成後寫入該檔的身分（Windows 的 volume serial＋file index）。身分在 Windows 用 `winapi-util`（`same-file` 也是靠它，已在依賴樹內，改為直接依賴）讀磁碟序號＋file index，unix 用 dev＋inode（測試在 mac 上跑同一套判斷與真硬連結），比對同一檔就是比身分。下表的「有標記」都指「標記存在且記的身分等於槽位現在那個檔」；標記在、身分卻不符，一律當不相干的實體檔回 `CodexAuthSlotOccupied`。

| 現況 | 處置 |
|---|---|
| 目標 `%USERPROFILE%\.codex\auth.json` 不存在 | 有標記就刪掉槽位上的舊檔（那是我們建、已分離的舊連結）和標記；然後回 `CodexAuthMissing`。沒標記但槽位有實體檔，照下一列「不相干的實體檔」處理 |
| 槽位不存在 | 用臨時名 `hard_link` 建好，再 rename 蓋上，接著寫標記。跨磁碟建不了就附兩個路徑回 `CodexProfileFailed` |
| 槽位與目標是同一檔 | 不動；缺標記或標記身分過時就補寫 |
| 槽位與目標不是同一檔，且有標記 | 舊硬連結已分離：使用者重新登入時，codex 的 ChatGPT 登入流程會先 logout、刪檔再建新檔。用臨時名重建後 rename 蓋上 |
| 槽位與目標不是同一檔，且沒有標記 | 不相干的實體檔：回 `CodexAuthSlotOccupied`，不覆蓋 |

與 macOS 的差別：symlink 本身就看得出是我們建的，斷鏈也能留著等使用者登入；硬連結要靠標記判斷來歷，而且目標消失後沒有東西可以連，只能回錯。行為上兩邊一致：終端機登出，app 跟著登出；重新登入後，下一次呼叫就接回去。檢查完到 spawn 之間如果剛好有人登入或登出，那一輪會失敗，屬已知限制。

- **讀不到登入**：`cli_envs` 只建目錄、確保連結，沒登入不算錯（模型清單、換幕容量都會走到）；真正要跑 codex 的兩處（單發 codex 分支、`prepare_lane_call`）才檢查目標 `auth.json`，不存在就回 `CodexAuthMissing`（十語系）。文案大意：「app 讀不到 codex 登入。請到設定頁登入 codex；若你把 codex 登入設成存在系統鑰匙圈，app 無法共用，需改回檔案儲存。」不讀使用者的 config.toml 去猜是哪種情況〔模型判斷·未裁決〕。
- **最後防線**：`run_cli_cancellable` 要 spawn 的程式是 `codex`／`codex.exe` 時，envs 必須明確帶非空、絕對路徑的 `CODEX_HOME`，否則 spawn 前就回錯。這和 agy 的 `check_spawn_envs` 合併成一個依程式名查表的共用函式。
- **呼叫點**：
  - `transport::dispatch::cli_envs` 加 `"codex"` 分支（`codex_profile`）。
  - 單發的 `"codex"` 分支改用它（現在傳 `&[]`），lane 加上 `LaneProvider::Codex => cli_envs(app, "codex")?`。
  - 生圖也走同一個 home，登入狀態才一致。
- **設定頁**：
  - 登入／登出腳本與 Windows 的安裝 spec 都不帶 CODEX_HOME（`cli_setup::setup_envs` 對 codex 一律給空環境）。共用登入下，`codex login` 照樣寫 `~/.codex/auth.json`，正好就是連結目標；帶了會把登入寫進 codex-home、佔掉連結位置。
  - `codex login status` 探針也不改。唯一落差是 keyring 使用者會看到「已登入」，實跑卻報 `CodexAuthMissing`；情況罕見，靠錯誤文案處理。
- **模型清單**（`cli/catalog.rs`）：從 envs 取 `CODEX_HOME`，先讀 `codex-home/models_cache.json`，讀不到再退回 `~/.codex/models_cache.json`。
- **換幕容量**（`scene_budget/limits.rs` 的 `read_codex_files`）：改吃 codex-home 路徑。新增純路徑函式 `codex_home_dir(config_root)`（比照 `grok_home_dir`），`cli_envs` 與 `scene_budget/mod.rs:265-268、471-474` 都用它；換幕容量這條路徑不建目錄、不確保連結，也不因為還沒登入就失敗。
  - `models_cache.json` 先讀 codex-home，再退回 `~/.codex`。
  - `config.toml` 只讀 codex-home 那份，正常情況不存在，所以預設模型照現行「解析不到」分支處理。理由：app 一直帶 `--ignore-user-config`，使用者 `~/.codex/config.toml` 裡的 `model` 本來就不會生效，現在拿它解析預設模型是既有的不一致。沒覆寫模型時本來就用不到 models_cache，容量走 200k 猜測值，只提醒、不鎖。
  - `:282` 那句「沿用使用者自己的 ~/.codex，沒有另設 CODEX_HOME」的過時註解一併改掉。

## 2. 精簡旗標（`cli/request.rs`）
- **內容**：`CODEX_LEAN_FLAGS` 是以下 14 組 `-c`（不含 `features.skip_host_skill_discovery`，它是開發中旗標，會在 JSON 吐 error item）：
  `skills.include_instructions=false skills.bundled.enabled=false cloud.skills.enabled=false features.plugins=false features.remote_plugin=false features.recommended_plugins=false features.memories=false memories.use_memories=false memories.generate_memories=false features.multi_agent=false features.multi_agent_v2=false include_collaboration_mode_instructions=false agents.enabled=false features.shell_snapshot=false`
  - 真登入實測帶前 13 組不報錯。`shell_snapshot=false` 是用假 key 驗的：`shell_snapshots/` 從 3 個檔變 0 個，也沒有新錯誤。施工時再用真登入確認一次。
- **套用範圍**：單發（旁白單發、卡重構、換幕摘要等）、lane 開線、lane 續聊都帶。生圖（`allow_tools`）不帶，它要用 `$imagegen` 與 bundled skills。
- **參數順序**：`exec --json --skip-git-repo-check --ignore-user-config -s read-only [-m M] -c model_reasoning_effort=E <LEAN> [resume <thread_id>] -`。
  - `-s` 一定放在 `resume` 前。`-m`、`-c` 是全域參數，統一放前面。
  - 開線不帶 `--ephemeral`；單發照帶。
- **函式拆分**：`codex_args`（單發）與 `codex_session_args(model, effort, resume: Option<&str>)`。

## 3. 成功判定（runner，只對 codex 生效）
**現況**：runner 收到 `Done` 不會 break，後到的 `Done` 會蓋掉前面的，所以「頂層 error → turn.completed」現在本來就判成功。真正的缺口只有一個：EOF 時沒收到 `turn.completed`、行程卻以 0 結束，會被當成功（`runner.rs:587-607` 只看正文空不空）。codex 確實有這種分支：EOF 直接結束迴圈，沒有檢查是否收到完成事件。

**改法**：
- 把 `parse: fn(&str) -> CliLine` 換成 `&CliProtocol { parse, require_done: bool }`，每家一個常數。只有 `CODEX_PROTOCOL.require_done = true`，claude、agy、grok 維持 false，行為不變。呼叫端是 `dispatch.rs`、`lanes/mod.rs`、`refactor_ai/session.rs`、`inflight.rs`、`harness/ai_log.rs` 與兩份 runner 測試，只換參數。
- 「已報失敗的收尾不被後到事件覆蓋」只對 codex（`require_done`）生效；其他家照舊以後到的收尾為準（grok 頂層 error 之後 end_turn＝成功）。
- `CliLine` 新增 `SoftError(String)`。codex 解析器把頂層 `{"type":"error"}` 改吐 `SoftError`，不再吐 `Done{is_error:true}`；runner 記 log，並存下最後一則訊息。
- 判定規則：

| 情況 | 結果 |
|---|---|
| 收到 `turn.completed`，exit 0 | 成功（中途有 SoftError 也一樣） |
| 收到 `turn.completed`，exit 非 0 | 本輪正文照常交付、不重送，記 log；lane 收到「非正常退出」旗標，下一輪重開、不續用這條線〔模型判斷·未裁決〕：completed 事件不能證明非正常退出前已經持久化 |
| 收到 `turn.completed`，之後不退出、被 app 寬限期（DONE_GRACE）殺掉 | 成功、lane 照常續用〔模型判斷·未裁決〕：codex 0.164.0 回合結束先 `flush_rollout` 才送 TurnComplete（`core/src/tasks/mod.rs:395-412`），flush 等寫入任務確認並投影 SQLite（`thread-store/src/local/live_writer.rs:346-389`、`rollout/src/recorder.rs:1099-1118`）。期限到時先 `try_wait`，已自己退出的照上一列；這輪出現「Failed to save the conversation transcript」存檔失敗警告的照上一列丟線；每次寬限殺都記 log |
| `turn.failed` | 失敗，錯誤文字取 `turn.failed` 的訊息 |
| 沒有 `turn.completed`、exit 0（含有正文） | 失敗。錯誤文字優先取最後一則 SoftError，沒有才用通用的未完成訊息。`context_overflow::cli_failure` 照現行吃這段文字，太長類錯誤仍認得出來 |
| exit 非 0、沒有 `turn.completed` | 照現行 `CliCrashed`；有 SoftError 時把它附進 tail |

- **影響範圍**：也作用到 codex 的單發路徑（旁白單發、卡重構、換幕摘要、生圖）。實際變化只有兩項：「EOF 沒收到完成事件卻判成功」改判失敗；頂層 error 改走 SoftError。

## 4. lane（`LaneProvider::Codex`）
- **形狀**：同 Agy。`chars_lane_shape` 的 Codex 分支是一角一線、私設提進該角色自己的凍結 system、不抹、不加前綴。`run_turn` 開頭對 confidential／prefix 的防呆、`keeps_tail`、`StateBlockGone` 都比照 Agy。不改寫 rollout。
- **線名**：維持「線種:純模型名[:角色 id]」，不放 effort，`last_prompt_tokens`（`lanes/mod.rs:290-294`、`scene_budget/mod.rs:409`）才查得到。effort 的處理：
  - `LaneCall` 補 `effort: Option<String>`（只有 codex 填）。
  - `LaneState` 存 `effort`。
  - `plan_turn` 遇到 effort 不同就以新的 `ReopenReason::EffortChanged` 重開；比較放在所有 resume 出口之前，緊接在 `ProviderChanged` 之後。重開時 usage base 一併清掉。
  - 一條 codex 角色線屬於固定角色、固定檔位，換 effort 只發生在玩家改檔位覆寫時，很少見。
  - 主線原本給的方向是「effort 放進 scope 或另一個 key 欄位」；改成重開，是因為放進 key 同樣會讓 `last_prompt_tokens` 要多帶 effort 才查得到，重開比較單純。請複審確認。
- **正文**：開線時 stdin 送「system＋本輪」，續聊只送本輪（與 `agy_session_body` 同形，抽成共用函式）。
- **thread id**：codex 不能預先指定 id，開線時從 `thread.started` 擷取。
  - 擷取改成 run_cli 的獨立選項（`SessionWatch { id_out, expected }`），不綁在 `usage_log` 有值的條件上。agy 現行寫法不動。
  - 續聊時驗證 `thread.started` 的 id 等於預期，不符就回新的通用碼 `CliSessionMismatch`（帶 CLI 名；`AgyConversationMismatch` 是已落檔的持久代碼不改名，agy 照用），交給 `Err(_) if !opening` 走 `Reopen{ResumeFailed}`。
  - resume 一個不存在的 id，codex 會回「no rollout found」，同樣走重開（假 key 實測）。
- **usage**：`turn.completed.usage` 是整條線的累計值，逐輪減掉上一輪。
  - `LaneState` 新增 `codex_usage: Option<CodexUsageCounters>`（input、cached、cache_write、output），不泛化 `agy_usage`，也不加 serde alias（發佈前不管舊桌）。
  - runner 拿 base 相減後寫進帳本。
  - 綁 thread：重開時 base 歸零。中止或失敗時，呼叫前已落 `pending_rewrite`，下一輪一定重開，等於重新同步（沿用現行機制）。
  - 任一計數倒退，或續聊時 base 缺：這輪用量記為未知，`prompt_tokens_out` 給 0，原始累計值只留作診斷證據，不當本輪用量入帳。成功收尾後照樣存下目前的累計值，當成下一輪的基準。
- **偶發不中不觸發重開**：`plan_turn` 本來就沒有「因為不中而重開」的出口，這裡也不加。
- **快取壽命**：codex 線比照 agy，成功後給 `LEGACY_CACHE_TTL_SECS`，不走 `next_cache_ttl` 的 claude 推算。
- **換幕容量校正**：`calibration_estimate` 不納入 codex（開線那輪的實報包含 codex 自帶約 11k 的底線，估計值和實報配不上）。
- **lane 範圍**：只有角色聊天與 GM 旁白走 lane（`lane_provider` 加 `"codex"`），其他呼叫仍是單發。
- **拆檔**：`lanes/mod.rs` 現在 968 行，先把每家參數與正文組裝（`match call.provider` 那段），連同 UsageLog 依 provider 分流的那段（`mod.rs:671-703`），一起純搬到 `lanes/provider_args.rs`，再加 Codex 分支。

## 5. 測試通道
`harness/root.rs` 的 `scan_for_symlinks` 走到 `root/config/codex-home/tmp`、確認它是真目錄（上層各層都已驗過不是連結）時，就地用 `remove_dir_all` 刪掉（不跟隨連結）。那裡是 codex 每次自建的 `apply_patch`／`applypatch` 連結，指向 codex 執行檔。config 本身是指向外部的連結時，什麼都不刪、直接回錯。

`auth.json` 的 symlink 比照 agy keychain 那條規則放行：連結值解析後與 `<真家目錄>/.codex/auth.json` 兩邊 `canonical_lenient` 相同才放行，其他照擋。清理（`--fresh`）只刪連結本身，不碰目標（加測試釘住）。

## 6. 已知限制〔模型判斷·未裁決〕
- codex-home 裡的 rollout、SQLite、generated_images 會累積，不做清理，比照 agy-home-cleanup 另案。
- 使用者用 keyring 存登入，或自己在 shell 設了 `CODEX_HOME`：app 共用不到登入，只靠 `CodexAuthMissing` 文案。
- 同時 refresh 撞車，或確保連結到 spawn 之間剛好有人登入／登出：那一輪失敗，下一輪恢復。
- 正文剛好出現 `$技能名` 時，會明確載入使用者 `~/.agents/skills` 裡的技能。
- 續聊約 1/10 的輪次偶發不中（掉回底線），兩次實測（9.8k 卡腳本、範例桌測試通道）一致；屬 codex／後端行為，app 只記錄、不因此重開。

## 7. 測試與驗收
- **單元測試**：
  - `real_home`：只有 HOME、只有 USERPROFILE、兩者皆無、相對路徑。
  - symlink 的四種情況、斷鏈照建、原位是實體檔時回錯且不動它。
  - 硬連結的五種情況：判斷函式寫成跨平台、可在 mac 上測，實際建連結的那段用 cfg 包起來。
  - 目標不存在時 spawn 前回 `CodexAuthMissing`；codex 缺 `CODEX_HOME` 就拒跑。
  - 參數：單發、開線、續聊三種的順序與旗標；生圖不帶精簡旗標。
  - 成功判定：
    - 頂層 error → turn.completed → exit 0：成功。
    - 只有頂層 error、exit 0：失敗，錯誤文字是那則 error。
    - 有正文但 EOF 沒完成、exit 0：失敗。
    - turn.failed：失敗。
    - turn.completed、exit 非 0：成功並記 log。
    - claude／agy／grok 的既有判定不變。
  - 累計相減：正常、倒退時記未知並重設基準、重開歸零。
  - lane：Codex 形狀、thread id 擷取（usage_log 為 None 也擷取得到）、id 不符時重開、effort 相同續用／不同重開、confidential 防呆、completed 但 exit 非 0 時正文只交付一次且下一輪不續用。
  - 硬連結標記：身分相符才替換、不符回錯、同一檔缺標記補寫。
  - harness：config 是外部連結時不刪任何東西、直接回錯。
  - harness：`codex-home/tmp` 被清掉、auth.json 連結放行、其他連結照擋。
  - `read_codex_files` 與模型清單的讀取順序。
- **測試通道**（獨立 `--root`，真登入）：
  - ① 開線一輪後，`<root>/config/codex-home/sessions/` 出現 rollout。
  - ② 續聊沿用同一個 thread。
  - ③ 把 lane 的 thread id 換成假 id：一次重開後回話正常。
  - ④ 模型清單抓得到。
  - ⑤ 單發與生圖都能跑。
  - ⑥ `~/.codex/sessions` 反證：實測前先建標記檔，實測後用 `find -newer` 只回筆數，不看內容。
  - ⑦ 用真登入確認 `shell_snapshot=false` 不報錯。
- **命中率**：用測試通道真桌，2 分鐘間隔跑 10 輪以上、10 分鐘間隔跑 5 輪，從帳本取逐輪 input／cached。
  - 指標（衡量上一輪的前綴有沒有被沿用）：每輪 `min(1, max(0, 本輪 cached − 底線) ÷ (上一輪 input − 底線))`，逐輪平均；掉回底線的那輪算 0、照樣算進平均。底線＝同旗標單發的 cached（本次 11,136，會隨 CLI 版本變，要當場重量）。不扣底線會把 codex 自帶的固定段也算成「沿用」，數字虛高（本次約 95%），不採用。這不是 OpenAI／Anthropic 官方的命中率（官方是 cached ÷ 總輸入），只用來看續聊前綴有沒有接上。
  - 輔助數據：我方內容命中＝`max(0, cached − 底線) ÷ (本輪 input − 底線)`。每輪新增內容佔比越大它天生越低（小桌約八成上限），只作參考。
  - 判準：2 分鐘組上述指標平均 85% 以上，且沒有任何一輪因為不中而重開。
  - 實測（範例桌，帳本 scratchpad `cxroot/data/prompt-cache.jsonl`）：GM 線 2 分鐘組 11 輪 88.47%、10 分鐘組 5 輪 95.15%；輔助數據 74.7%／89.1%。
- **verification-queue**：Windows 實機登入共用（硬連結建立、終端機重新登入後接回）。
- `npm run verify` 12 步綠。

## 預計動到的檔案（目前行數）
| 檔案 | 現行行數 | 內容 |
|---|---|---|
| cli/codex_home.rs（新） | 0 | home、symlink／硬連結確保、auth 檢查、envs |
| cli/agy_home.rs | 270 | `real_home` 加 USERPROFILE 後備；`check_spawn_envs` 抽成共用 |
| cli/mod.rs | 36 | 匯出 |
| cli/request.rs | 412 | 精簡旗標、`codex_session_args`、共用正文函式 |
| cli/request/tests.rs | 504 | 參數測試 |
| cli/types.rs | 88 | `CliProtocol`、`CliLine::SoftError`、`CodexUsageCounters`、`SessionWatch` |
| cli/stream.rs | 941 | codex 解析改吐 SoftError、thread id 與累計值解析 |
| cli/runner.rs | 617 | require_done 判定、SoftError 收集、thread id 驗證、累計相減、spawn 防線 |
| cli/runner/tests.rs | 903 | 成功判定測試（若破千就另開子檔） |
| cli/runner/runaway_tests.rs | 286 | 換參數型別 |
| cli/catalog.rs | 327 | models_cache 讀取順序 |
| scene_budget/limits.rs | 488 | `read_codex_files` 改吃 codex-home、註解 |
| scene_budget/mod.rs | 565 | 傳入 codex-home |
| lanes/mod.rs | 968 | 先抽出 provider_args 再加 Codex 分支 |
| lanes/provider_args.rs（新） | 0 | 各家參數、正文、UsageLog 分流 |
| lanes/plan.rs | 310 | `EffortChanged`、形狀判斷擴到 Codex |
| lanes/tests.rs | 1815 | 呼叫點換型別；Codex 測試另開 `lanes/tests/codex.rs` |
| cli/prompt_file.rs | 222 | 呼叫點換型別 |
| transport/dispatch.rs | 734 | `codex_profile`、`cli_envs`、`lane_provider`、單發 envs 與參數 |
| commands/chat.rs | 1655 | provider 分支只加 match 臂 |
| refactor_ai/session.rs | 274 | 換參數型別 |
| inflight.rs | 410 | 換參數型別 |
| harness/ai_log.rs | 280 | 換參數型別 |
| harness/root.rs | 685 | 清 codex-home/tmp、auth.json 放行 |
| ui_msg.rs＋十語系字典 | — | `CodexProfileFailed`、`CodexAuthMissing`、`CodexAuthSlotOccupied`、`CliSessionMismatch` |
| Cargo.toml | — | `winapi-util` 直接依賴（Windows） |
