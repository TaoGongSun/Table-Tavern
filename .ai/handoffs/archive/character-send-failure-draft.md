# character-send-failure-draft — 對角色送出：對白已上桌後才出錯仍彈帶原文的失敗視窗

## 狀態
已結案（2026-10-28）：Sol、opus-review 驗收 PASS，已合進 main。

## 查證結論
- 打字送角色：`submitTurn`（`src/features/play/chat/useChatController.ts`）→ `replyOnce` 落對白後跑 `markCliConnected`，回到 `submitTurn` 再跑 `refreshWorlds`。後兩步任一拋錯，`narrated` 只有 GM 路徑會設，於是走收回路徑：`discard_unanswered_player` 因玩家句已非檔尾回 false（不會刪、不遺失），最後 `failTurn(reason, started, raw)` 彈帶原文的視窗——誘導重送。
- 對白與玩家句本來就不會遺失或重複；缺陷只在彈窗帶原文（外加一次白跑的收回呼叫）。
- 已正確不需動：`requestReply`、`callNominated`、`gmAdvance`（從不帶原文）、卡片介面送出（`fromComposer=false`）、送出並接話（GM 旗標已設）。

## 做法（已施工）
1. `replyOnce` 加選用參數 `onLanded`，兩處 `appendEvent` 成功後（停止留半截與正常完成）、`markCliConnected` 前呼叫；不看 `appendEvent` 回傳值（換桌時回 undefined 但已落檔）。
2. `submitTurn` 的 `narrated` 改名 `replied`，GM 與角色共用；角色分支傳 `() => { replied = true; }`；更新 `submitTurn` 上方註解。catch 不變。
3. 測試（`chat-send-failure.test.tsx`）：角色對白落檔後 `refreshWorlds` 失敗、`markCliConnected` 失敗、停止留半截後失敗、回覆途中換桌落回原桌後失敗四例都不收回、彈窗不帶原文、桌上兩則都在；既有「角色回覆前失敗→收回放回」照過。`markCliConnected` 改成可注入的 mock。

## 已知限制
- 角色對白的 `append_transcript` 本身失敗時沒有 GM 那種原鍵重試與後端代落，對白遺失、玩家句照舊收回放回輸入框。既有行為，不在本案〔模型判斷·未裁決〕。
