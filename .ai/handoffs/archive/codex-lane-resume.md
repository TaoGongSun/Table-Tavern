# codex-lane-resume — codex 改走 lane 續聊，讓我方內容吃到快取

## 現況
- 已結案（2026-10-28 合併 main；Sol、opus-review 驗收 PASS；Windows CI 只有 cargo check 綠，結案條件改為「Windows cargo check 綠＋本機 verify 12 步綠」〔作者裁決 2026-10-28〕）。已知限制：Windows 測試沒實際執行過（CI 的 cargo test 卡在別案的 `fake_claude` cfg 與測試程式缺 manifest 啟動不了，主線另案）。session 存放採 B（隔離 CODEX_HOME＋auth.json symlink 共用登入）、Windows 用硬連結〔作者裁決 2026-10-28〕；方案見 [plans/codex-lane-resume.md](../../plans/codex-lane-resume.md)。剩實機驗收（macOS 設定頁登入、Windows、codex 升版）排在[實測佇列](../../reference/verification-queue.md)。
- 測試通道實測（迷霧酒館範例桌、真登入、CLI 預設模型 gpt-6.1-sol、GM 檔位 medium；帳本 scratchpad `cxroot/data/prompt-cache.jsonl`；底線＝同旗標單發 11,136）：
  - 判準照方案 §7（扣底線、對上一輪可中量）：GM 線 2 分鐘組 11 輪 88.47%（含 1 輪掉回底線，age 167 秒），10 分鐘組 5 輪 95.15%；輔助的我方內容命中 74.7%／89.1%。
  - 10 分鐘組每輪啟動、送出、退出讓出通道，實際 age 648–827 秒，0 次掉底線。
  - 角色線（一角一線，間隔隨點名而定 58–3,685 秒）12 輪：2 次掉底線（age 794、1,366 秒），61 分鐘那輪仍中 13,824。
  - 偶發不中都沒有觸發重開；lane 的 thread id 換成假 id → 一次 resume-failed 後重開回話正常；模型清單抓得到；換幕摘要（單發，帶精簡旗標）與 codex 生圖（不帶精簡旗標）都通，生圖存進圖庫為 PNG。
  - codex-home 只有 app 的 5 個 rollout（GM、3 角色、重開後的 GM）；`shell_snapshots/` 只在生圖那次建出、是空的。`~/.codex/sessions` 實測期間多了 11 個檔，同時段有其他代理用 codex 問 Sol，依規定不讀內容，無法逐一歸屬。
- 併入 codex-skills-in-prompt〔主線 2026-10-28〕：app 的 codex 呼叫把使用者自己的技能清單（skills_instructions，單發第 1 輪約 4.9KB、續聊第 4 輪又重注入 19.5KB）帶進跑團請求；下方固定旗標同時解這件，單發路徑也要帶。
- 起因：non-claude-real-cache 實測（[plans/non-claude-real-cache.md](../../plans/non-claude-real-cache.md)），單發 `exec --ephemeral` 我方內容零命中，`exec resume` 同線 4 輪中 2 輪中到上一輪整段。

## 查證結果（codex-cli 0.164.0）
- 快取鍵：`prompt_cache_key＝session_id＝root thread id`，ChatGPT 路徑另以 session-id header 決定 cache affinity；單發每次新 session，所以中不到。續聊不保證命中。第 3 輪掉回底線的原因未明。
- 只隔 sessions 的官方設定不存在：rollout 固定寫 `CODEX_HOME/sessions`，`sqlite_home` 只搬 SQLite。
- auth.json（檔案式，`--ignore-user-config` 下預設 file）：login／refresh 以 `OpenOptions` 原地覆寫，**跟隨 symlink**，斷鏈時穿過去建 0600 目標檔；`logout` 只刪連結本身。以上實測過。keyring 模式的帳號鍵綁 CODEX_HOME 路徑。refresh 有行程內鎖、送出前重讀；跨行程沒有鎖，撞上 `refresh_token_reused` 時當次判永久失敗（不刪檔），下次重讀到新憑證就恢復。
- 注入：下列 `-c` 每輪固定帶，實測 `<skills_instructions>`、`<multi_agent_role/mode>` 消失、plugins 不再 clone，開線＋resume 三輪沒有新 developer 訊息；工具定義 36→30.6KB、base instructions 22KB 照留：
  `skills.include_instructions=false skills.bundled.enabled=false cloud.skills.enabled=false features.plugins=false features.remote_plugin=false features.recommended_plugins=false features.memories=false memories.use_memories=false memories.generate_memories=false features.multi_agent=false features.multi_agent_v2=false include_collaboration_mode_instructions=false agents.enabled=false`
  `features.skip_host_skill_discovery` 不帶〔模型判斷·未裁決〕（開發中旗標，會在 JSON 吐 error item；include_instructions=false 已擋住清單進 prompt）。
- usage：`turn.completed.usage` 是整線累計（`ThreadTokenUsage.total`），逐輪相減；被 kill 的回合已落檔的用量會算進下一次差值。
- 參數：`-s` 要放 `resume` 前；`-m`、`-c` 是 global，前後皆可。每輪重帶同一組 `-s -m effort` 與注入旗標。換模型不報錯但不沿用快取。
- `exec` 不觸發 CLI 自我更新。

## 真登入命中實測（2026-10-28，~/.codex，CLI 預設模型 gpt-6.1-sol、effort low、帶上列旗標）
- 我方內容約 9.8k token（WestFantsy 卡描述＋世界書），每輪加一句玩家行動；用量取 `turn.completed` 累計值相減（實測確認是累計：20,948 → 42,034 → 63,244）。旗標真回應下不報錯。
- 底線（單發 `--ephemeral`，獨立前綴 10 通）：9 通 11,136、1 通 0；我方內容 0/10 命中。另一組單發曾中 20,736，是因前綴與同時跑的續聊線相同、蹭到別條線的快取，不算單發自己命中。
- 續聊（不含開線輪）：

| 輪間間隔 | 輪數 | 整段命中 | 少中 1–2 塊 | 掉回底線或 0 | 我方內容平均命中 |
|---|---|---|---|---|---|
| 約 2 秒（兩條線） | 20 | 14 | 4 | 2 | 88% |
| 2 分鐘 | 10 | 10 | 0 | 0 | 98% |
| 5 分鐘（中途停跑） | 3 | 0 | 2 | 1 | 64% |
| 10 分鐘 | 4 | 4 | 0 | 0 | 98% |

- 每輪請求都是上一輪的嚴格延伸，固定段（developer 訊息＋工具定義 33,148 bytes）各線雜湊相同，沒有中途注入；所以掉快取不是前綴變動，是後端路由／寫入時差。上一則回覆剛結束就送（約 2 秒）時較常不中；5 分鐘那次不中而 10 分鐘全中，不是 TTL。
- 原始紀錄：scratchpad `cachetest/`（各組 `turns.jsonl`、`prefix.txt`）。

