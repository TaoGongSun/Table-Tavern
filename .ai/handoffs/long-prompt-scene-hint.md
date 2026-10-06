# 桌子太長撞到指令長度上限：先修 Windows、上限前提醒換幕

公開前必做〔作者裁決 2026-10-04〕。規格、拍板與驗收細節在 [plans/long-prompt-scene-hint.md](../plans/long-prompt-scene-hint.md)（§1 實測、§2 範圍 2、§3 範圍 3、§4 範圍 4）。分支 `long-prompt-scene-hint`（未合併）。

## 範圍（立案原文）

1. 實測三個作業系統下 claude／grok／agy 撞上限的門檻與錯誤表現。
2. 撞得到就改傳遞方式（stdin 或暫存檔），不能只靠提示補救。
3. **上限前就提醒換幕**：換幕本身也要送 AI，所以必須在還送得出去時就提醒；門檻要把換幕那次呼叫的長度算進去〔作者裁決 2026-10-04〕。
4. 真的撞上時的錯誤走既有人話錯誤路線（`explainAiError`／`ErrorNote`／`TurnFailedDialog`）。
5. 文案十語系。

## 現況

- 範圍 1、2：完成，Sol 驗收通過。
- 範圍 3、4（2026-10-06 拍板見 plans §3.1）：包 A–D 與各輪修正全部 Sol 驗收通過、主線乾淨 checkout 重跑 verify 10 步綠。
  - A 範圍 4 太長錯誤 `7bfb6c5`
  - B+C 容量量測／提醒／鎖／後端關卡 `4163199`，修正 `d420579`、`9c7934e`、`6e82476`
  - D 分段摘要＋停止 `1aa403c`，修正 `a2e1922`、`b40edbd`，建議 `c064ffa`
  - E 校準係數（haiku 五類夾具，存成 `scene_budget/estimate.rs` 測試）`d389456`
- 主要程式：`src-tauri/src/scene_budget/`（mod／capacity／estimate／gate／limits／measure／summarize）、`src-tauri/src/chat_assembly.rs`（實送與量測共用組裝）、`src-tauri/src/transport/context_overflow.rs`、前端 `src/features/play/{scene-budget.ts,useSceneBudget.ts}`、`PlayView` 提醒列、`TurnFailedDialog` 換幕鈕。

## 包 E 測試通道實測（進行中）


測試通道已 quit、無殘留 app 程序。`npm run harness:build` 已在本 worktree（`agent-a6dcd3607d32fbf75`，含 `2ff1dc3`）打過包，換程式要重打。輔助腳本在 scratchpad（臨時，可重寫）：`/private/tmp/claude-501/-Users-pachelo-GitHub-Table-Tavern/2f79d440-443a-471c-ab83-e159be2fd267/scratchpad/`——`hx.mjs`（包 `scripts/harness.mjs`，cwd 寫死 worktree 路徑，換 worktree 要改）、`fill.mjs`（交錯玩家句＋旁白）、`fill2.mjs`（每段旁白各帶不同 `action_id`、無玩家句，模擬連續旁白）、`waitn.mjs`（等逐字稿到 N 則並印 kind／action_id）、`gate.mjs`、`agy-config.json`／`claude-config.json`。root：`h-agy2`、`h-claude`、`h-claude2`（可刪）。範例桌按鈕：`click 'role=button[name*="進入 迷霧酒館"]'` 進桌，旁白鈕用 `js` 找文字為「GM 旁白」的按鈕 `.click()`。

已過（agy gemini-3.8-flash-low 5 次；claude haiku 9 次小呼叫＋1 次被擋；另直接 CLI haiku 校準 13 次）：
- 範例桌本幕 155KB → 換幕提醒＋「現在換幕」；181KB → 鎖：旁白／推進／送出停用、換幕可按（`e-agy-full.png`）。
- 後端關卡：滿容量時 `append_player_event` 回 `scene_capacity_full`、逐字稿不變。
- 換幕單次（181KB）成功；230KB 走分段：2 段＋1 合併成功。換幕中按停止：幕號不變、無錯誤彈窗、子程序收掉。
- claude 700KB 桌送出：TurnFailedDialog「這一幕太長…」＋換幕鈕。
- claude haiku 正常旁白一次後 `model-capacity.json` 記下 `claude-haiku-4-5-20251001`、context 200000、max_output 32000；換幕一次寫入 summary 校正（估 765／實 1279），之後 `scene_budget` 為 `reliable`＋`lockable`（ratio 1.67）。
- 按動作切段（`2ff1dc3`）在真 app：一次 GM 推進（3 輪旁白＋點名＋接話）9 則全帶同一個 `action_id`，之後 GM 旁白另一個 id；推進後 gReply 7,083（＝該推進總量 ×1.2）。再灌 20 段各自獨立 id 的 10KB 旁白（無玩家句）：used 169,225／cap 187,166、gReply 9,707 → 只提醒不鎖，送出／旁白／推進都可按（`e2-noearlylock.png`；舊算法會把整幕併成一段、預測約 20 萬直接鎖）。再加 2 段到 185,394 → `append_player_event` 回 `scene_capacity_full`，真滿時照樣鎖。

固定證據 `scratchpad/evidence-E/`（同上 scratchpad 目錄）：
- `gate-claude.json`：滿容量時 `append_player_event` 回 `scene_capacity_full`，關卡前後逐字稿 sha256／bytes／則數一致（`gate-evidence.mjs` 產生，免 AI，可重跑）。
- `model-capacity-claude.json`、`model-capacity-agy.json`：容量與校正原檔。
- `scenes-agy.json`、`scenes-claude.json`：各桌目前幕號、每幕逐字稿 sha256／則數、開頭提要（`scene-evidence.mjs` 離線產生）。agy 桌現在第 2 幕（230KB 分段換幕產生）。
- `harness-ai-*.log`：只有 dispatch／spawned，測試通道 log 不記子程序退出。
- 截圖 `e-agy-hint.png`、`e-agy-full.png`、`e-claude-toolong.png`、`e2-claude-scene.png`、`e2-noearlylock.png`。
- **沒有的**：取消換幕那次沒存「取消前」的幕號／逐字稿快照，也沒存子程序退出紀錄（當時只用 ps 確認收掉）。現在只能從 `scenes-agy.json` 看到「取消後」的狀態。要補得重跑一次 agy 換幕＋停止（付費），沒重跑。

還沒跑：
- grok：2026-10-06 22:00 額度恢復後才可送（之前一律不碰）。正文自身 >100KB 驗 `--verbatim` 不搬檔、模型讀得到尾巴；爆 context 的錯誤長相與上限，回寫 plans §1.3、§3.4、§4，必要時補 `context_overflow` 字樣。

## 2026-10-06 拍板

- CJK 估計係數維持 1.45，不調〔作者裁決 2026-10-06〕。
- G_reply 改按每次動作切段〔作者裁決 2026-10-06〕：逐字稿事件新增 `action_id`（前端 `appendEvent`／玩家句蓋上，GM 登場紀錄後端蓋），`scene_budget::predict_reply` 依 id 切段、舊事件退回玩家句切段（plans §3.6）。`2ff1dc3` Sol 驗收通過；狀態更新、獨立點名、卡片登場蓋 id 的斷言已補上。

## 未決

- GM 指示四態對拍只補了 Narration／InterfaceTakeover，CardFormat／CardFormatAbsent 兩態是建議、未做。

## 下一步

1. 剩 grok：22:00 後實送（最低階模型、次數最少），拿到結構化錯誤與上限，回填 plans §3.4、§4 並補樣本測試，然後結案。
2. 三家真 CLI 在 Windows 讀暫存檔已排進[實測佇列](../reference/verification-queue.md)梯 3。
3. 結案：刪暫時探針 `.github/workflows/argmax-probe.yml`、`.github/argmax-probe/`；照 CLAUDE.md 整理分支歷史（分包大案一階段一筆、修正併進所修階段；`2ff1dc3` 併進 B+C）。main 已進 image-model-picker，合併時 `transport/client.rs` 兩邊改動都要保留。測試通道全機一把鎖：啟動前 `ps` 確認沒有 Table Tavern 程序，用完立刻 quit。
