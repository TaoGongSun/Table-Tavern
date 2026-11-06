# room-scene-advance-bar：換幕鈕搬到回合列（方案）

來源：玩家回饋三案之後，同一位桌面版房主說「需要加入換幕功能」。查證：換幕已有（房主頁第五段、`scene.advance`），手機與桌面嵌入同一套元件，沒有版本落差；問題是露出——房主整場待在故事頁，回合列沒有換幕，GM 因「這一幕滿了」卡住時只有句子沒有鈕。作者選「搬到回合列」〔作者裁決 2026-11-06〕。全篇取捨未標來源者為〔模型判斷·未裁決〕。

案別：小案，核心估約 50 行（純客戶端）。不做方案審，做完 Sol 驗收一次、Sonnet 截圖主線親看。

## 0 現況已有的（不重做）

- 伺服器 `scene.advance { round, next? }`：房主限定；`acting` 或 `held` 且 `scene_full`；沒帶 `next` 就用出額度者在 GM 額度頁「下一幕設定」存的 `nextSettings`，沒存就沿用本幕。`next` 只有房主本人是出額度者才收。
- 換幕中回合列寫「換幕中，GM 正在整理前情…」；卡住（15 分鐘沒進度）房主有「取消換幕」鈕（回合列，不動）。
- GM 額度頁「下一幕設定」（`set_next`）：出額度者任何時候都能存，換幕才生效。

## 1 回合列（`RoundBar.tsx`，`.round-actions` 內、「⏱ 倒數」之後）

只有房主看得到；`view.room.status === "playing"` 且有回合。

- `r.phase === "acting"`：`PartyMenu`，觸發鈕「換幕 ▾」（`className="ghost"`，與倒數選單同款），只有一項：「結束本幕」，`note`「整理前情開新幕；已送出的行動退回草稿」。選了送 `scene.advance { round: r.round }`。一項選單就是確認步驟：換幕會取消本回合、沒有退回路，直接一顆鈕放在結算旁太容易誤點。
- `r.phase === "held" && r.hold?.kind === "scene_full"`：直接一顆 `primary` 鈕「換幕」送同一指令，不經選單——狀態句已寫「這一幕滿了，請房主換幕」，這時只剩這條路。
- 其他階段（generating、transition、lobby）不出鈕。手機 `compact` 模式 `.round-actions` 本來不收，照舊。
- 不帶 `next`：房主兼出額度者要換模型，用 GM 額度頁「下一幕設定」（既有）。原房主頁的「換幕順便選模型」拿掉，伺服器行為相同（§0）。

## 2 房主頁（`HostPanel.tsx`）

移除「換幕」段落、`canAdvance`、`RoutePicker` import；檔頭註解把換幕改寫成「換幕在回合列」。其餘不動。

## 3 文案

- `party/src/i18n`：新增 `hostAdvanceMenu`「換幕 ▾」、`hostAdvanceItem`「結束本幕」、`hostAdvanceNote`「整理前情開新幕；已送出的行動退回草稿」；`hostAdvance`「換幕」留給卡住時的主鈕。zh-TW 正典、zh-CN／en 翻譯、其餘七語填英文；`node scripts/check-i18n.mjs` 綠。
- 說明書 `web/src/features/funnel/guide/room.{zh-TW,zh-CN,en}.md`「換幕」那條改成：在回合列點「換幕 ▾」→「結束本幕」；GM 卡在「這一幕滿了」時回合列直接有「換幕」鈕；想換模型先到 GM 額度頁存「下一幕設定」。

## 4 驗證（`room.test.tsx`）

- 既有「換幕：已送出的行動退回草稿…」測試：房主不再切到房主頁，改在故事頁點「換幕 ▾」→「結束本幕」，其餘斷言不變。
- 新增：非房主的回合列沒有「換幕 ▾」；GM 生成中房主也沒有；`held` 且 `scene_full` 時房主（不是出額度者也一樣）看到「換幕」主鈕，點了伺服器收到 `scene.advance`。
- `cd party && npm run typecheck && npm test && npm run format`、根目錄 `node scripts/check-i18n.mjs`、`node scripts/check-structure.mjs` 綠；結案前主線跑 `npm run verify`。

## 5 觀察（範圍外，不立案）

- 房間沒有單機版那種換幕前的容量提示（紀錄很長／快到換幕容量／接近模型讀得完）：執行端不回報幕容量，房主要等 GM 失敗才知道該換幕。要做得碰協定（執行端隨額度回報帶容量比例）＋桌面執行端＋伺服器＋回合列，屬中案；一般玩家會遇到，等本案結案後另立。

## 5a 施工結果

- 核心約 +28／−22（RoundBar 加選單與主鈕、HostPanel 刪換幕段與 RoutePicker import）、測試 +63、i18n 三鍵十語、說明書三語。Grok 施工、Sol 驗收 1 輪 PASS、Sonnet 截圖四張主線親看。
- 已知限制：手機 375 寬房主進行中動作列三顆鈕（結算、倒數、換幕 ▾）擠不下，「換幕 ▾」換到第二行；動作列本來就允許換行，不另縮鈕。執行端離線時鈕仍可點，伺服器回「GM 主機離線」，與原房主頁相同。
