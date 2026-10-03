# card-mvu-shim — 卡片介面沙盒墊 MVU 讀寫變數函式

已合併 main（包 1、2a、2b、2c 各一筆，Sol 驗收皆通過）。計畫：[plans/card-mvu-shim.md](../plans/card-mvu-shim.md)，規格以計畫為準，這裡只寫狀態與接手要點。

## 現況
- 四包程式都在 main：包 1 只讀墊片、2a message 層寫入核心、2b 非 message 層六層、2c `Mvu.parseMessage`（值解析在宿主 Web Worker）。做法、取捨與已知限制見計畫各「施工結果」段。
- 只剩實機驗收：[實測佇列](../reference/verification-queue.md)梯 1 第 4／4a／4b／4c 項（4a–4c 用自製測試卡 `scripts/harness-fixtures/mvu-write-probe.json`）。

## 下一步（待確認）
1. 實機驗收上面四項（使用者做）。
2. 待確認：兩張受惠卡的 GM 格式指示可能壓掉 `<UpdateVariable>`（導演指示寫「不要輸出規定格式以外的任何說明或狀態欄」）；實機抓 GM 提示時判定，確認衝突就另立案，不在本案處理。

## 接手要點（計畫沒寫到的）
- **禁止**：不啟動正式版 app（com.tabletavern.app），不寫 `~/Documents/TableTavern`、`~/Library/Application Support/TableTavern`；GUI 驗收由使用者做，只排實測佇列。
- **verify**：`npm run verify` 約 3–5 分鐘，跑到「✓ verify 全部通過」；Rust 改完先 `cd src-tauri && cargo fmt`。效能閘：`cargo test --release --lib perf_gate -- --ignored --nocapture`。
- **鎖**：`with_commit` 可重入（同執行緒已持有就直接跑）；鎖內函式收 `&CommitTx`。`read_state` 是投影入口（鎖內讀）；直接讀檔是 `read_state_cache`；改 state.json 欄位用 `data::update_state`。GM 回合：`begin_turn`（先 `settle_previous_turn` 代落上一輪）（回 `TurnTicket`）→ `apply_gm_block`（核 turn_id／幕／世代，提交時才啟用變數模式，正文與 `TurnSide` 附屬部分一起進待落清單）→ 前端 `append_event` 帶 `TurnKey`。新事件一律經 `append_transcript_tx`（鎖內先交接），只有 `append_line_tx`（transcript.rs 私有）不交接；同回合附屬追加用 `append_within_turn`＋憑證。
- **TranscriptEvent 加欄位**：Rust 約 50 處結構字面值；照 `cargo test --no-run` 回報的 E0063 位置，在該行 `TranscriptEvent {` 之後插新欄位（可寫小腳本照錯誤位置插），不要用「找 `TranscriptEvent {` 到配對 `}`」硬套。
- **故障注入**：控制檔與 state.json 都走原子替換（暫存檔＋改名），`RenameFailGuard` 打得到；寫到一半用 `WriteFailGuard::partial_ending("state.json.tmp", n)`（只打指定檔）；逐字稿追加用 `AppendFailGuard`。唯讀權限擋不住改名，別再用。
- **換幕結算會自動隱藏沒登場的卡**，隱藏的 MVU 卡不算資格；跨幕測試要把卡解除隱藏（`message_vars/tests.rs` 有例子）。
- **沙盒墊片**：原始碼在 `mvu/card-mvu-shim-source.ts`（純 ES5＋async；parseMessage 那段在 `mvu/card-mvu-parse-source.ts`，String.raw 片段不能含反引號、`${`，比對反引號寫 `\x60`）；內部互叫用閉包函式，不讀 `window.*`（卡片會換掉）。單元測試用 `new Function("window", 原始碼)(假 window)`，假 window 放 `_: lodash` 與 `parent.postMessage`。
- **非 message 層**：後端 `data/card_vars.rs`（`write_layer` 桌內層在短提交鎖內核對世代、根目錄層用 `with_root_file_lock`）；沙盒寫入目標 key 以 `parseLayerKey` 辨識；controller 的 `layers` 只在面板開著時讀、關掉清空。（計畫第 6 節）：兩張受惠卡都走 `CardFormat`，導演指示可能壓掉 `<UpdateVariable>`，等 GUI 驗收實抓提示才定；確認衝突就另立案（不由本案子代理立案）。

## 受惠卡
bcd368、DongeonMaster（TestCards 普查，2026-10-02）；寫入類沒有真卡，實測用自製測試卡。
