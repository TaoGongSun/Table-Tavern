# card-mvu-shim — 卡片介面沙盒墊 MVU 讀寫變數函式

分支 `card-mvu-shim`（未合併 main，2c 做完才結案；結案時一包一筆壓縮合併）。計畫：[plans/card-mvu-shim.md](../plans/card-mvu-shim.md)，規格以計畫為準，這裡只寫狀態與接手要點。

## 現況
- **包 1（只讀墊片）完成**：Sol 驗收通過；GUI 驗收排[實測佇列](../reference/verification-queue.md)梯 1 第 4 項。
- **包 2 設計定案**：計畫第 8 節（Sol 第 2–15 輪同意）。
- **2a（寫入核心）**：第一版 da93e51；Sol 第 1 輪修正 216a68b、第 2 輪修正 724a23f、第 3 輪 7a44b51；第 4 輪 2 項必改已修完（`append_opening` 鎖內最前面先交接、截斷標記改由該次呼叫回傳），等主線檢查與 Sol 第 5 輪驗收。做法與取捨都在計畫「包 2a 施工結果」。GUI 驗收排實測佇列梯 1 第 4a 項（自製測試卡 `scripts/harness-fixtures/mvu-write-probe.json`；harness 28 不代替這個端到端）。
- **2b（非 message 層，B2 六層）**：Sol 驗收通過（第 23 輪）。做法、取捨與核對都在計畫「包 2b 施工結果」。GUI 驗收排實測佇列梯 1 第 4b 項（同一張自製測試卡，已加六層欄位與寫入按鈕）。
- 2c（parseMessage）未動。

## 下一步
1. 開 2c（計畫 8.9），另派子代理。

## 接手要點（計畫沒寫到的）
- **協作**：施工中有問題先 SendMessage 問主線；子代理不直接找 Sol。回覆一律繁中。commit 訊息 `card-mvu-shim: …（verify 結果）`，結尾加 Co-Authored-By。
- **禁止**：不啟動正式版 app（com.tabletavern.app），不寫 `~/Documents/TableTavern`、`~/Library/Application Support/TableTavern`；GUI 驗收由使用者做，只排實測佇列。
- **verify**：`npm run verify` 約 3–5 分鐘，跑到「✓ verify 全部通過」；Rust 改完先 `cd src-tauri && cargo fmt`。效能閘：`cargo test --release --lib perf_gate -- --ignored --nocapture`。
- **鎖**：`with_commit` 可重入（同執行緒已持有就直接跑）；鎖內函式收 `&CommitTx`。`read_state` 是投影入口（鎖內讀）；直接讀檔是 `read_state_cache`；改 state.json 欄位用 `data::update_state`。GM 回合：`begin_turn`（先 `settle_previous_turn` 代落上一輪）（回 `TurnTicket`）→ `apply_gm_block`（核 turn_id／幕／世代，提交時才啟用變數模式，正文與 `TurnSide` 附屬部分一起進待落清單）→ 前端 `append_event` 帶 `TurnKey`。新事件一律經 `append_transcript_tx`（鎖內先交接），只有 `append_line_tx`（transcript.rs 私有）不交接；同回合附屬追加用 `append_within_turn`＋憑證。
- **TranscriptEvent 加欄位**：Rust 約 50 處結構字面值；照 `cargo test --no-run` 回報的 E0063 位置，在該行 `TranscriptEvent {` 之後插新欄位（可寫小腳本照錯誤位置插），不要用「找 `TranscriptEvent {` 到配對 `}`」硬套。
- **故障注入**：控制檔與 state.json 都走原子替換（暫存檔＋改名），`RenameFailGuard` 打得到；寫到一半用 `WriteFailGuard::partial_ending("state.json.tmp", n)`（只打指定檔）；逐字稿追加用 `AppendFailGuard`。唯讀權限擋不住改名，別再用。
- **換幕結算會自動隱藏沒登場的卡**，隱藏的 MVU 卡不算資格；跨幕測試要把卡解除隱藏（`message_vars/tests.rs` 有例子）。
- **沙盒墊片**：原始碼在 `mvu/card-mvu-shim-source.ts`（純 ES5＋async）；內部互叫用閉包函式，不讀 `window.*`（卡片會換掉）。單元測試用 `new Function("window", 原始碼)(假 window)`，假 window 放 `_: lodash` 與 `parent.postMessage`。
- **非 message 層**：後端 `data/card_vars.rs`（`write_layer` 桌內層在短提交鎖內核對世代、根目錄層用 `with_root_file_lock`）；沙盒寫入目標 key 以 `parseLayerKey` 辨識；controller 的 `layers` 只在面板開著時讀、關掉清空。（計畫第 6 節）：兩張受惠卡都走 `CardFormat`，導演指示可能壓掉 `<UpdateVariable>`，等 GUI 驗收實抓提示才定；確認衝突就另立案（不由本案子代理立案）。
- 上下文到約 600k 時，在手上這輪做完寫交接換手。

## 受惠卡
bcd368、DongeonMaster（TestCards 普查，2026-10-02）；寫入類沒有真卡，實測用自製測試卡。
