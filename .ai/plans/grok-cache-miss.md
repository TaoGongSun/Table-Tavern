# grok-cache-miss — grok 通道改走續聊

## 根因（2026-08-22 實證定案）

**不是我方把前綴打散。** 逐段比對 08:18 與 08:20 兩次請求：system 5773 字元、CLI 注入的 `<user_info>` 2050、skills 提醒 9037 三段 md5 全同，正文共同前綴 9468／9815 字元，合計共享 16158／16371 tokens（98.7%），xAI 仍只回 128。

**「九成命中」是加總假象。** grok CLI 的 `unified.jsonl` 顯示 8/21 每輪跑 3 個 loop（同 session 內的 agent 迴圈），app 的 prompt-cache 一列把三次加總：22:26 那列 in 62266／cached 58624 ＝ 20563+20776+20927／17408+20480+20736。loop2、loop3 天生共用 loop1 前綴，必中。

**真正壞的是跨 session 那一格。** 只算每輪第一次呼叫，全部樣本 28 次中 7 次；app 自帶 profile 那段 15 次中 1 次。xAI Responses API 用 `prompt_cache_key` 把同一段對話導向同一台 cache server，grok CLI 沒有這個旗標，每次新 session 就是隨機路由。

**帳號不是分水嶺**：同一個帳號 `34787ec2` 走 `~/.grok` 命中 6／12、走 app 的 grok-home 全是 128。

## 實驗（2026-08-22 13:08–13:14，headless 實跑，attempts=1 才採計）

| 形態 | 第1輪 | 第2輪 | 第3輪 | 第4輪 |
|---|---|---|---|---|
| 續聊 A（`-s` 開線、`-r` 續） | 7909／128 | 9524／128 | 10235／9472＝92.5% | — |
| 續聊 B | 7908／128 | （attempts=2 不採計） | 10036／9472＝94.4% | 10514／9984＝95.0% |
| 對照（每輪新 session 全量重送） | 7909／128 | 8451／128 | 完全相同 prompt 重送：7909／128 | — |

暖起來後穩定 92–95%，且 prompt 只長增量。

## 拍板

grok 聊天與 GM 走 lane 續聊，沿用 claude 那套 `lanes.rs`（水位＋指紋＋回覆對點，對不上就重開全量）。與 claude 的三處差異：

1. **一角一線**：線名 `chars:<模型>:<角色 id>`（GM 仍是 `gm:<模型>`）。grok 的 session 檔沒有可靠的回合後抹寫路徑，私設改提進該角色自己的凍結 system（`hoist_private`），一角一線才不會漏給別的角色。`run_turn` 對 grok 帶機密段一律回 Err 擋下。
2. **system 只在開線送，漂移就整線重開**：grok 把 system 凍在 session 建立那一刻（`-s` 配 `--system-prompt-override`），續聊 `-r` 不重帶。補丁是 user 層訊息、壓不過權重更高的舊 system，所以 `frozen_system` 一變就 `Reopen{SystemChanged}`（Sol 驗收指出）。凍結快照只吃 constant／public／未停用的素材，每輪不變，重開只在玩家改卡、改世界書、新角色上場時觸發。
3. **不保溫**：ping 要截 session 檔尾，只有 claude 的格式做得到；`LaneState` 存 `provider` 供 keepalive 過濾，換 CLI 開的線一律 `Reopen{ProviderChanged}`（舊檔沒這欄位當 claude，不白重建快取）。

`-s` 對已存在的 id 會報「Session ID is already in use」，開／續兩條旗標不能互換（實測）。

## 驗收

- cargo 530 綠（新增 `grok_session_args` 旗標、`lane_key` 細分、漂移分流、換 CLI 重開、grok 機密段防呆五項測試）
- headless 實跑：續聊第 3、4 輪 cached 92–95%，對照組固定 128
- 待使用者實機：grok 通道連玩三輪，`prompt-cache.jsonl` 的 cached_tokens 隨對話增長

## 查證中順手發現，另案處理

1. **角色卡回歸事件會漏私設**：`transport::card_arrival_text` 把 `private_md` 寫進事件正文，`record_card_arrivals` 標 `gm_only: false`，所有角色線都讀得到。原始碼註解說「chars 快照本來就含全卡，不算新洩漏」，但 `chars_lane_system` 實查只吐 `public_md`（`grep -c private_md` = 0），前提不成立。早於本案，claude 共線與 api／codex／agy 單發同樣會漏，本案未動。
2. **`-p` 走 argv 有 ARG_MAX 風險**：8/21 實測有一列 241,336 tokens（約 700KB），接近 macOS 的 1MB 上限。grok 1.0.5 有 `--prompt-file <PATH>`。續聊讓大包只剩開線那輪，要真正解決得連 `grok_args` 單發一起改（會動到生圖、翻譯、卡重構），本案未動。
