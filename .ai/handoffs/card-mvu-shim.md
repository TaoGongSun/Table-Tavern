# card-mvu-shim — 卡片介面沙盒墊 MVU 讀寫變數函式

已合併 main（包 1、2a、2b、2c、實機驗收修正各一筆，Sol 驗收皆通過）。計畫：[plans/card-mvu-shim.md](../plans/card-mvu-shim.md)，規格以計畫為準，這裡只寫狀態與接手要點。

## 現況
- 四包程式都在 main：包 1 只讀墊片、2a message 層寫入核心、2b 非 message 層六層、2c `Mvu.parseMessage`（值解析在宿主 Web Worker）。做法、取捨與已知限制見計畫各「施工結果」。
- 實機驗收修正已合併 main（一筆，Sol 驗收通過）：
  - 面板↔狀態欄雙向同步：手改（`useTableStateController.save` 後 `onEdited`）→ App 重讀逐字稿事件（`chat.reload`）；卡片寫入確認（`committed`）→ `onCardWrite` → `tableState.refresh`。根因：變數模式面板讀前端 events 的 `message_vars`，手改只改後端事件表。
  - 重讀競態：`chat.reload` 同世代只跑一趟＋補讀；本地改動（追加、收回、復原、開場白、丟棄玩家句走 `beginWrite`，卡片寫入 `replaceEvent`）在讀的期間完成或在途，那份讀結果作廢、等改動落定重讀。追蹤（`WriteTrack`）按換桌換幕世代各一份，換世代叫醒舊等待者、舊收尾只動舊那份。`beginWrite` 的收尾一定要在呼叫 `reload` 之前，否則重讀等自己。同世代落檔永不回應時重讀跟著等、不加逾時；持續改動延後重讀套用——兩者 Sol 判定可接受。`replaceEvent` 只換同一物件或同 id 那一則；目標是沒 id 的舊事件時另外重讀拿權威結果。`tableState.refresh` 同桌同世代只跑一趟＋補讀（`hydrate` 換世代）。
  - 捲動：`PlayView` 同 `storyKey`（桌＋幕）且只換變數表（`sameStory`）不捲動，換桌換幕一定捲到底。
  - 測試卡 `mvu-write-probe` 的 first_mes 補 `<StatusPlaceHolderImpl/>`（開場事件不補占位）。
- 測試通道代測：4、4a、4b、4c 全過。通道點不到沙盒 iframe 內的按鈕，卡片寫入是在主框架派一筆 `source` 為 iframe 的 `mvu-write` 訊息代替（token 取自 srcdoc、base 取自畫面上的事件）；卡片按鈕實際送出排在實測佇列。
- 第 4 項⑤結論：`<UpdateVariable>` 規定沒被壓掉；格式缺失與數字欄 replace 拒收已另立 `gm-format-directive-missing-target`、`mvu-replace-numeric`。
- 未追觀察：匯入後「選開場白」面板有時沒出現（實測時用 `post_opening` 繞過），原因未追。

## 下一步
- 只剩實測佇列梯 1 的 card-mvu-shim 項目（含卡片介面 iframe 內按鈕實際互動）。

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
