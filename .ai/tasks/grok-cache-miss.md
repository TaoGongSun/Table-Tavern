# Grok 快取命中率從九成掉到 2%

## Summary
grok 通道每輪開新 session 的無狀態單發，跨呼叫拿不到 prompt cache（實測 28 次中 7 次，app profile 那段 15 次中 1 次）。原本看到的九成命中是加總假象：grok CLI 每輪跑 3 個 loop，app 的 prompt-cache 把三次加總，loop2／loop3 天生共用 loop1 前綴。根因是 xAI Responses API 靠 `prompt_cache_key` 把同一段對話導向同一台 cache server，grok CLI 沒這旗標，新 session 就是隨機路由；帳號不是分水嶺。已改成走 lane 續聊（`-s` 開線、`-r` 續、只送增量）。規格與實驗數據見 .ai/plans/grok-cache-miss.md。

## Progress
- GM 線實機驗收綠（2026-08-22）：連四輪 128／128／92%／93%，prompt_tokens 8915→9874→10763→11560 每輪只長約 900，確認在送增量。
- 角色線驗收暫停，等 card-arrival-private-leak 拍板——那案會決定角色線是維持 grok 的一角一線、還是改回共線，現在驗了到時候要重驗。
- 根因與實驗定案，Sol 兩輪驗收標 DONE。
- cli.rs：抽出 `grok_common_args`，新增 `grok_session_args`（Open 帶 `-s`＋system override、Resume 只帶 `-r`）；`ClaudeSession` 改名 `CliSession`
- lanes.rs：`ClaudeCall`→`LaneCall`＋`LaneProvider`；`lane_key` 加 scope（grok 一角一線）；grok 素材漂移一律 `Reopen{SystemChanged}`；換 CLI 開的線 `Reopen{ProviderChanged}`；grok 帶機密段直接回 Err；keepalive 只 ping claude
- lib.rs：`prepare_claude_call`→`prepare_lane_call(provider)`；新增 `lane_provider(config)`；角色與 GM 兩條路都改走它，grok 的私設提進凍結 system
- cargo 530 綠、rustfmt 差異數與 HEAD 同基準、clippy 無新增警告

## Next action
等 card-arrival-private-leak 拍板角色線怎麼組裝，再驗收角色線：讓角色接三輪以上話，看 `chars:grok-4.6:<角色 id>` 的 cached_tokens 隨對話增長，並確認換角色、改卡、換幕之後不會每輪重開。GM 線已驗完，不必重驗。

## Constraints
- grok 沒有 session 檔抹寫路徑，私設一律提進該角色自己的凍結 system＋一角一線；`run_turn` 對 grok 帶機密段直接擋下。這個設計是否保留由 card-arrival-private-leak 決定。
- grok 的 system 凍在 session 建立那一刻，續聊不重帶；素材一漂移就整線重開，不走補丁。
