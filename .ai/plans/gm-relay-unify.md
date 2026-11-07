# gm-relay-unify 方案

案別：大案（核心估 450–600 行：後端共用驅動 250–300、前端改接 150–250，另刪前端 250+）。碰到玩家句收回（資料遺失路徑）、停止與回合鎖，方案與驗收都送審。

## 0. 現況（2026-11-07 主線讀碼）
- **規則一樣、寫了兩份**：旁白 → 點到 NPC 接話 → 點到真人／沒點名／達 `max_round_speakers` 停；點名後被切成舞台卡不接；「GM 請 X 發言」點名列。
  - 單人：前端 [useChatController.ts](../../src/features/play/chat/useChatController.ts) `narrateOnce`／`replyOnce`／`callAndReply`／`advanceFrom`，迴圈與落檔都在前端（`appendEvent` → `append_transcript`）。
  - 房間：後端 [room/run.rs](../../src-tauri/src/room/run.rs) `steps` 狀態機（`Step::Land/Narrate/Reply/Done`）＋`land_narration`／`land_reply`／`append_call`，進度存 room.json 可重送續跑，每段廣播 `gen.part.*`。
- 模型呼叫兩邊已共用後端 `chat_turn::gm_narrate`／`chat_with_character`（房間經 `room::Engine` trait）；只差迴圈與落檔。
- 已看到的行為差異（施工方開工先補齊成對照表，每條定「統一成哪邊」）：
  1. 截斷旗標：單人 `takeResponseTruncated`（輸出上限截斷也標 `truncated`）；房間只在停止（aborted）時標。→ 統一：落檔 `truncated = 停止或輸出上限截斷`。`gen.part.end.interrupted` 仍只代表停止。旗標從模型呼叫帶出（`GmNarration`／`ChatReply` 加不進 IPC 的欄位）。
  2. 正文落檔：單人帶冪等鍵重試 3 次、仍失敗設 `mainLost` 由後端下回合代落；房間失敗直接整段 `gen.failed`。→ 統一：驅動對正文重試 3 次（`AI_EMPTY_RESPONSE` 不重試），仍失敗才整段失敗。房間仍回 `gen.failed`（協定不變），不走 `mainLost`；`PendingPart` 代留。重試只拉長失敗，不改訊框。
  3. `state_error`：單人提示玩家；房間丟掉。→ 統一：驅動回報 host。房間 host 仍丟掉（協定沒有這個欄位）。單人包 2 再提示。
  4. 點名玩家：單人寫 `playerName`（空字串＝顯示時補稱呼）；房間用 `next_player` 那張卡名。→ 統一：host 決定名字。房間仍用那張卡名，單人用 `playerName`（可空）。多人身分必要差異，保留。
  5. 入口：單人四種——一般送出（一次旁白，點名記下給「讓 X 接話」）、送出並接話（旁白＋一句就停）、GM 推進（跑到上限）、請 X 發言／讓 X 接話（直接點名）；房間三種——`narrate`（一次旁白記 called）、`call`（直接點名，可帶點名列）、預設推進。→ 統一：驅動四種模式與三種點名列都做。房間仍只走 narrate／call／advance；`call` 的點名列與重送不重寫留在 Land。
  6. 換桌換幕：單人靠回合世代，旁白落原桌但不再接力、不碰畫面；房間無此概念。→ 統一：驅動有「不再接力」（在途那段照常落完，含變動紀錄，不寫點名列、不再發新呼叫）。房間不送這個訊號。
  7. 截斷的旁白接不接力：單人 GM 推進在回傳值上仍會接（與 `onLanded` 不一致）；房間只在停止時不接。→ 統一：輸出上限或停止都不接力（方案 §1）。停止不寫變動紀錄與點名列；輸出上限仍寫變動紀錄（狀態已提交），不寫點名列、不接話。
  8. 點名缺卡：房間讀不到卡或是舞台卡就停、不寫點名列；單人只擋 `stage === true`，缺 meta 會用 id 當名字去接。→ 統一：讀不到或舞台卡都停（房間現行）。單人缺 meta 的那條等包 2 改成停。〔模型判斷·未裁決〕
  9. 空白旁白：房間不落空正文、不佔段號（一個字都沒吐時）；單人仍會嘗試落。→ 統一：不落空正文（房間現行）。落空會多一則、改段號，破房間鏈。〔模型判斷·未裁決〕
  10. 旁白已回來、落檔期間才按停止：單人不寫點名列、不接話；房間會先寫點名列，下一段開頭才停。→ 統一：不寫點名列、不接話。模型呼叫本身沒被標成停止時，`gen.part.end.interrupted` 仍是 false，整條 `gen.done.aborted` 為 true。〔模型判斷·未裁決〕
  11. 空白正文（沒停止卻一個字都沒有）：房間照 9 不落；單人 host 回 `AI_EMPTY_RESPONSE`，維持失敗彈窗與收回玩家句。〔模型判斷·未裁決〕
  12. 停止與換桌的入口：停止沿用 `chat_abort`，帶接力 id 時整條不再接並中止在途那段；換桌換幕由前端在世代變或卸載時送 `chat_relay_release`。〔模型判斷·未裁決〕

## 1. 方向（Sol 方案審 2 輪定案）
**後端一份驅動，兩邊都用它**。另一條路（只共用停止條件小函式）落檔仍兩份，不解決問題，不採。
- `chat_turn` 新增接力驅動模組：輸入＝模式（一次旁白／旁白＋一句／推進到上限／直接點名〔可帶點名列〕）＋上限；透過 host trait 拿模型呼叫、每段開始／增量／結束的回報、步驟持久化（房間寫 room.json，單人 no-op）與停止查詢。落檔（旁白正文帶回合鍵、`state_update`、點名列、角色回覆、截斷旗標）全在驅動內。
- 房間：`steps` 改成 Land（玩家批次，房間專有）之後交給驅動；廣播、durable、重送留在 room。
- 單人：新 tauri 指令跑驅動，用 Channel 回報 `part.start`／`delta`／`part.end`（帶已落檔事件、點名、出場卡、`state_error`）。前端只負責串流顯示、把後端已落的事件放進逐字稿 store（不再自己寫檔）、記點名鈕、失敗彈窗與乾淨路徑收回玩家句（`append_player_event`／`discard_unanswered_player` 不動，只是 narrated 改由下述 `main.landed`／累積收據判定）。
- **停止與換桌分開**（Sol 第 1 輪必改 1）：驅動一開始就固定原桌原幕，有兩種訊號——
  - 「中斷當段」＝玩家按停止，走既有 `chat_abort`，當段半截落檔、不再接力；
  - 「不再接力」＝換桌換幕，在途那段照常完成並落原桌（含狀態提交），之後不再發新呼叫。
  - 停止旗標整條接力共用、不會每段重設（落檔期間按的停止要生效）；每次模型呼叫仍各有自己的 turn id。
- **正文落檔即回報**（必改 2）：正文一落檔立刻送 `main.landed`（帶落檔事件＝收據），不等 `state_update`、點名列；整條結束或失敗時的結果也帶累積收據，前端據此判定 narrated（決定彈窗帶不帶原文、能不能收回玩家句）。
  - 既有 `PendingPart`、原 turn key 與後端代落流程保留。
  - 代落發生時，回傳補落事件（或旗標讓前端重讀）。涵蓋的入口：下一次玩家追加、角色／GM 呼叫、復原；做到才能刪前端 `mainLost`。
- **入口與點名契約**（必改 3）：
  | 入口 | 驅動模式 | 點名列 |
  |---|---|---|
  | 一般送出／GM 旁白鈕 | 一次旁白，回報點名不接 | — |
  | 送出並接話 | 旁白＋被點角色一句就停 | 寫 |
  | GM 推進／房間預設 | 推進到上限 | 寫 |
  | 請 X 發言（單人 `requestReply`） | 直接點名 | **不寫** |
  | 讓 X 接話（`callNominated`） | 直接點名 | 寫，但該旁白之後已有同名點名列（`calledSince`）不疊 |
  | 房間 `call` | 直接點名 | 照 `host.call` |
  - 點到真人：host 決定名字。單人用 `playerName`（空字串照舊），房間用 `next_player` 那張卡名（多人身分必要差異，保留）。
  - 截斷（輸出上限或停止）的旁白：一律不接力，所有入口都一樣。現行單人 GM 推進截斷仍會接（`narrateOnce` 回傳 `next` 與 `onLanded` 的不一致），統一成不接〔模型判斷·未裁決〕。
- 玩家看得到的行為除上列明定者外不變；差異表其餘各項由施工方逐條定統一方向，預設取單人現行行為，房間跟著改，除非會破房間協定。

## 2. 分包
| 包 | 內容 | 施工 | 驗收 |
|---|---|---|---|
| 1 | 後端驅動模組＋房間改用；房間行為不變（差異表中決定改的除外），驅動用假 host 單元測試 | Grok（做到完，不交接〔作者裁決 2026-11-07〕） | Sol |
| 2 | 單人指令＋前端改接，刪前端迴圈與落檔；三入口＋停止＋換桌實測 | opus-medium | Sol |

〔作者裁決 2026-11-07：分包與分派照此表；兩包驗收都送 Sol、不等額度重置〕

## 3. 驗收
- `npm run verify` 全綠；既有房間測試（`room/tests`）只在差異表決議改動的項目上更新期待，其餘照過；`next-speaker-call`、`chat-send-failure`、`app-relay-lock` 等前端測試期待不變（只改 mock 層）。
- 測試通道實跑單人：一般送出出「讓 X 接話」→點鈕接一句；送出並接話一句就停；GM 推進到點名玩家停；推進中按停止；推進中換桌。房間：雙端跑一輪推進與停止。
- 收尾 diff 確認前端舊落檔碼刪乾淨，檔案行數前後對照。

## 4. 觀察
- `PendingMain` 註解寫「正常回覆的截斷只有前端知道」，`gm_narrate` 其實已把輸出上限寫進代落的 `truncated`。本案落檔用模型回報的旗標，不改代落。
- 單人介面在接力進行中停用「回大廳」與「換幕」，玩家正常操作碰不到「推進中換桌」；`chat_relay_release` 只在世代變或卸載時送，包 2 實測改由測試通道直接送訊號驗證。
- 「回應被中途中斷」同時用於輸出上限截斷與玩家按停止，沒按停止的玩家會疑惑誰中斷的；日後可分成「達輸出上限」（Fable 常識審）。

## 5. 結案（2026-11-08）
- 包 1：`chat_turn/relay.rs` `drive`＋`RelayHost`，開場鎖定原桌原幕、回覆前存說話者名字；房間 `steps` 改走 drive。Grok 施工、Sol 驗收 2 輪（必改 2）。
- 包 2：`chat_turn/solo_relay.rs` 新指令 `chat_relay`（五種 mode、Channel `start／delta／landed／arrived`、回 `{aborted, receipts, stateErrors, error, resync, owed}`）＋`chat_relay_release`；移除 `gm_narrate`／`chat_with_character` 兩個 tauri 指令與 `ai-response-truncated` 事件；前端 `useChatController.ts` 938→774、刪迴圈落檔與 `mainLost`、`calledSince` 搬進測試假後端。opus-medium 施工、Sol 驗收 2 輪（必改 1：測試夾具作者稱呼）、Fable 常識審採一條（請 X 發言點到舞台卡改提示一句）。
- 已知限制〔模型判斷·未裁決；Sol、Fable 判可接受〕：推進途中狀態列刷新晚一段（下一段開始與整條結束時刷）；`owed` 只存記憶體，重開 app 後第一句不重讀（同舊版）；單人空白正文仍跳失敗彈窗（房間不落）；推進中換桌只以測試通道直送訊號驗證（介面在接力中停用換桌入口）；房間雙端實機未跑，靠既有房間測試。
