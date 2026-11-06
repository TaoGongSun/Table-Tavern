# room-scene-capacity：幕容量提醒（方案）

來源：查換幕鈕露出（room-scene-advance-bar）時發現房間沒有單機版那種換幕前的容量提示：執行端不回報幕容量，房主要等 GM 因「這一幕滿了」失敗才知道該換幕。一般玩家會遇到，作者同意與換幕鈕案並行〔作者 2026-11-06 問「可以同時進行嗎」〕。全篇取捨未標來源者為〔模型判斷·未裁決〕。

案別：小案偏中，核心估約 90 行（執行端 Rust 約 40、伺服器約 20、客戶端約 30）。碰執行端上送的訊框（信任邊界）所以送 Sol 方案審；Sol 方案審 1 輪（必改 1：相容性，已採納改掛 `status` 欄位；建議 3 條採納）。做完驗收一次。

## 0 現況已有的（不重做）

- 單機版：`scene_budget::compute(config_root, root, world, &config, with_chat)` 算本幕容量，`summary.path.hint`（換幕摘要那條到提醒門檻）與 `chat_hint`（任一聊天路徑到門檻）；前端打字區上方出三級提示，容量滿的鎖由後端 `check_capacity` 把關。
- 房間執行端（`src-tauri/src/room/run.rs`）：GM 鏈跑完送 `gen.done`；換幕工作 `job.result`；`status_frame(room)` 從 `room.status()` 組狀態訊框，`report(room)` 送出；連上時（`link.rs`）送 `capabilities`＋`status`。`data::read_config` 已套房間覆寫的路線／模型（`play_override`），所以用它算出來的容量就是房間目前模型的。
- 房間幕號與桌面版桌內 `current_scene` 不同步（協定 §1），執行端不拿桌面幕號對照。
- 伺服器對未知執行端訊框回 `E_BAD_FRAME`、以 4400 關線，執行端 `link.rs::closed` 視為伺服器過舊、停止重連——所以**不能開新訊框**，新資料只能掛在既有訊框的可省欄位（舊伺服器忽略多出來的欄位）。
- `status` 訊框：驗證表 `EXECUTOR_FRAMES.status`、處理器存進 `s.exec`、`announceExec` 廣播 `vol.executor`；快照與即時廣播共用 `round.ts::execView` 投影。

## 1 執行端（`src-tauri/src/room.rs`、`room/run.rs`）

- `Engine` 多一個方法 `fn capacity_hint(&self, world: &str) -> Option<bool>`：`AppEngine` 用 `data::read_config(&config_root)` 與 `scene_budget::compute(.., world, &config, true)`，回 `Some(summary.map_or(false, |s| s.path.hint) || chat_hint)`；`compute` 失敗回 `None`＝量不到不報。測試引擎（`room/tests.rs`）回一個可由測試設定的值。
- 房間狀態（`room.status()`）多一欄 `capacity_hint: Option<bool>`；`status_frame` 在它是 `Some` 時多帶 `capacityHint`（`None` 不帶欄位，舊伺服器與舊狀態都當沒有）。
- 重算時機（都在派工／工作收尾、`work.dispatch`／`work.job` 清掉**之後**，用 `spawn_blocking` 跑 `capacity_hint`，算完寫進狀態再 `report(room)`；不擋 `gen.done`／`job.result` 送出，也不延後下一派工）：
  1. `gen.done` 之後（`gen.failed` 不算：失敗那句自己會講）。
  2. 換幕工作 `job.result` 且 `ok` 之後（新幕剛開，通常是 false，讓舊提醒消掉）。
  3. 換幕工作**開始**時先把 `capacity_hint` 清成 `None`（舊量測不帶進新幕；背景量測晚到也只會在 job.result 之後重算一次覆蓋）。
  4. 連上時不另算：`link.rs` 送的 `status` 帶目前存的值（重連時沿用上次量的；還沒量過就不帶）。
- 測試（`room/tests.rs`）：派工跑完 `expect("gen.done")` 之後的 `status` 帶 `capacityHint` 等於測試引擎設定的值；換幕工作後的 `status` 重新量；還沒量過的 `status` 沒有這個欄位。

## 2 伺服器（`party/server/core`）

- `validate.ts` `EXECUTOR_FRAMES.status` 加可省欄位 `capacityHint?:b`。
- `executor.ts` `status` 處理器：`p.capacityHint !== undefined` 時 `tx.w.exec.capacity = { scene: tx.s.scene, hint: p.capacityHint }`（蓋上房間當下的幕號，不信執行端報的幕）；沒帶欄位不動既有值。既有的 `announceExec(tx)` 照跑。
- `types.ts`：`RoomState.exec.capacity?: { scene: number; hint: boolean } | null`（舊狀態沒有欄位＝沒有）；`ExecutorView.capacityHint: boolean`。
- `round.ts::execView`（快照與 `announceExec` 共用）：`capacityHint = !!exec.capacity && exec.capacity.scene === s.scene && exec.capacity.hint`。幕號對不上（已換幕、執行端還沒重報）就是 false；舊版執行端從不報也是 false。換幕成功的 `sceneResult` 已經會 `announceExec`，提醒跟著幕號自然清掉，不另刪欄位。
- 測試（執行端測試檔）：`status` 帶 `capacityHint:true` 後成員的 `vol.executor.capacityHint` 為 true 且快照同值；不帶欄位的 `status` 不改它；`scene.started` 之後變 false；再收一則又為 true；`capacityHint` 不是布林 → `E_BAD_FRAME`。

## 3 客戶端（`party/src/features/room/RoundBar.tsx`）

- 只有房主看：`view.room.status === "playing"`、`r.phase === "acting"`、`view.executor.capacityHint` → `.round-actions` 下方一行 `data-testid="capacity-note"`，文案 `roundCapacityHint`「這一幕快滿了，再玩幾回合 GM 就送不出去，建議換幕」。換幕鈕本身是 room-scene-advance-bar 案放進同一個動作列的「換幕 ▾」，提醒就貼在它旁邊，不另加鈕。
- 手機 `compact` 模式這行不收（要處理的東西不收，與回合鈕同理）。GM 生成中、卡住、換幕中不出；非房主不出（他們動不了）。
- `ExecutorView` 型別來自伺服器，客戶端 `room-state.ts` 不用另加欄位（`executor` 整個換）。
- i18n：`roundCapacityHint` zh-TW／zh-CN／en 翻譯、其餘七語填英文；`node scripts/check-i18n.mjs` 綠。
- 測試（`room.test.tsx`）：假執行端送 `status { ..., capacityHint: true }` → 房主看到 `capacity-note`、玩家看不到；換幕完成後消失。

## 4 文件

- `.ai/plans/multiplayer-room-protocol.md`：§6 執行端訊框表 `status` 那列加 `capacityHint?`（時機＝§1；伺服器蓋幕號存 `exec.capacity`、投影成 `ExecutorView.capacityHint`）；`ExecutorView` 定義加 `capacityHint`；註明新資料只掛既有訊框可省欄位的理由（舊伺服器 4400 關線）。
- 說明書 `web/src/features/funnel/guide/room.{zh-TW,zh-CN,en}.md` 換幕那條加半句：這一幕快滿時回合列會先提醒。

## 5 驗證

- `cd src-tauri && cargo test room`（執行端）；`cd party && npm run typecheck && npm test && npm run format`；根目錄 `node scripts/check-i18n.mjs`、`node scripts/check-structure.mjs`；結案前主線 `npm run verify`。
- 與 room-scene-advance-bar 同時改 `RoundBar.tsx`、i18n、`room.test.tsx`：先合併換幕鈕案，再合本案解衝突。

## 6 觀察

- 不送比例數字：單機版提示也沒有數字，門檻判斷在執行端一處就好。要顯示「約幾成」再加欄位。
- 容量只在 `gen.done` 後重算：玩家行動落到桌上（派工時）不重算，提醒最多慢一回合。
- 背景量測與換幕的競態（量測讀檔時換幕正在寫）：換幕開始先清值、結束後重算一次，沒量到延遲前不另加鎖（Sol 列為建議）。

## 6a 施工結果

- 執行端：`Engine::capacity_hint`（`AppEngine` 讀 `read_config` 後 `scene_budget::compute(..., true)`，失敗回 `None`）；`Status.capacity_hint`；`status_frame` 只在 `Some` 時帶 `capacityHint`。`gen.done` 成功後、換幕 `ok` 後用 `spawn_blocking` 量完再 `report`，不擋這兩則訊框與下一派工；換幕開始先清成 `None`。測試兩條（還沒量過不帶欄位＋`gen.done` 後帶設定值；換幕中先清、成功後重算）。
- 伺服器：`status` 加可省 `capacityHint?:b`；有欄位才寫 `exec.capacity` 並蓋房間幕號；`execView.capacityHint` 在沒報過或幕號不符時為 false。測試五條。
- 客戶端：房主、playing、acting、且 `capacityHint` 時，動作列下方 `capacity-note`（手機 compact 不收）。`roundCapacityHint`：zh-TW／zh-CN／en 翻譯，其餘七語沿用英文。
- 文件：協定 `status` 列與 `ExecutorView.capacityHint`（註明只掛既有訊框可省欄位）；三語說明書換幕那條加半句。
- 核心約 Rust 45、伺服器 8、客戶端 8。測試 Rust 75、伺服器 74、客戶端 18。
- 已知限制：同 §6。背景量測若在換幕清值後、成功重算前寫回，舊幕的 true 可能被下一派工開頭的 `status` 帶上、被伺服器蓋成新幕，直到那回合 `gen.done` 重算才修正（只影響提醒，Sol 列為已知限制）。量測 task 本身 panic 時少送一次收尾 `status`（`busy` 伺服器只存不讀，無流程後果）。
- Sol 驗收 1 輪 PASS；修正輪：提醒改 `round-capacity` 警示樣式（accent、桌面版自成一行）、換幕清值測試先建立舊值。Sonnet 截圖三張主線親看。
