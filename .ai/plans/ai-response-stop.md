# ai-response-stop 規格

## 拍板
- 半截內容：留著、標「回應中斷」（沿用既有 `truncated: true` 與 `.response-truncated` 樣式）；一個字都沒出來就不落任何事件。不要的玩家用既有「收回」刪。〔作者裁決 2026-10-01〕
- 換幕摘要沒有 turn_id，不給停止鍵。〔作者裁決 2026-10-01〕
- 不另做實機驗收，直接結案；之後介面重新設計時的整體重測順手按停止看一次。〔作者裁決 2026-10-01〕

## 做法
1. **中止入口與識別**：前端每輪產生 `turn_id`，傳進 `chat_with_character`／`gm_narrate`；新 command `chat_abort(world_id, turn_id)` 只中止那一次呼叫——晚到的中止打不中下一輪。`inflight` key 改型別 `(Kind::Chat|Kind::Refactor, world_id)`，對話與重構互不波及；`refactor_abort` 行為不變。
2. **取消訊號進 lane**：CLI lane（`lanes::run_turn`）自己接 `CancelSignal`，不在外層 select 丟掉整個 future。取消時：殺子程序並**等它真正退出**→照常嘗試抹私設（案 C）；抹寫失敗就棄用該 session（刪 session 檔＋清 lane 記錄）。中止回合不更新 `expected_reply`，留 `pending_rewrite` 讓下輪由正典 transcript 重建。非 lane 路徑（API）在呼叫處 `select!` 即可。
3. **勝負以後端 select 結果為準**：取消勝出→回 `aborted: true`，跳過所有寫入（狀態、出場、usage 以外的副作用）；完成勝出→回正常完整結果，即使停止鍵剛好晚一點按下。
   - `chat_with_character` 回傳改成帶 `text`＋`aborted` 的 struct。
   - `gm_narrate` 中止：半截原文照正常剝法剝掉狀態欄／點名行當 `text`，`raw` 為 None、`state_updates` 空、`next` null、`arrived_characters` 空。
   - 後端累積 delta 的緩衝在 emit closure 內同步累加。
4. **前端**（`useChatController`）：
   - `stopResponse()`：記下目前 `turn_id` 並 invoke `chat_abort`；另設 `stopRequested` ref，每個新回合開頭清掉。
   - `replyOnce`／`narrateOnce` 收到 `aborted` → 有字才 `appendEvent(..., truncated: true)`；收到正常結果一律照常，不改標中止。
   - `gmAdvance` 在 `narrateOnce` 後、寫點名事件前、`replyOnce` 後都查 `stopRequested`，有就停。
   - 補同步 `busy` ref 防連點空窗（`generating` 是 state）。
   - 生成中送出鍵原位改「停止」鍵（同位置、可鍵盤操作、aria-label），i18n 十國補齊。
   - `stopResponse` 是唯一中止入口，日後更新閘門直接呼叫。
5. **措辭**：API 丟棄串流＝本端斷線，不保證供應商立即停止計費；不對玩家宣稱「已停止計費」。

## 不做
- lane store 讀改寫的並發鎖（聊天／保溫並行覆寫）是既有問題，本案只確保取消路徑不新增並行寫入。〔模型判斷·未裁決〕

## 驗收
- `npm run verify` 綠（含 tsc、i18n、cargo test）。
- 新增 Rust 測試：同桌 Chat／Refactor key 隔離；以舊 turn_id 中止不影響新呼叫；lane 取消後子程序已退出、私設已抹或 session 已棄用、`expected_reply` 未更新（假 CLI）。
- 實機另驗 CLI 中止後無殘留衍生程序。
- 實機（排進驗證佇列）：API 與 claude CLI 各一輪——GM 旁白中途停、角色對話中途停、GM 接力中途停；半截有標記、下一輪正常、私設沒外洩。
