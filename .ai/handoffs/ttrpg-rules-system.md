# Task
Task-ID: ttrpg-rules-system
Title: 跑團規則系統：規則書引入＋擲骰＋角色紙（規則中立引擎，零內建內容）
Status: in-progress
Created: 2026-08-13
Updated: 2026-11-08
案別：v1 大案，核心估 600–680 行（非測試、非 i18n 譯文、非純型別連帶）；含持久化對帳、單人骰鈕與房間骰池顯示。〔模型判斷·未裁決〕

## Summary
app 是規則中立引擎：規則書＝世界書的一種，內建零規則內容，版權責任在玩家側。既有地基沿用：關鍵字觸發、ST 世界書匯入、匯入預設 visibility=GM。拍板結論、分期、驗收見 [plans/ttrpg-rules-system.md](../plans/ttrpg-rules-system.md)。

**跑團不是獨立桌種**，是聊天桌上的能力組（規則書＋骰池＋角色紙＋戰鬥數值面板），大廳以徽章標示；畫面仍是聊天桌加側欄，另立桌種會把引擎再叉一次（目前 GM 接力已合一）。〔作者裁決 2026-11-07，採納主線建議〕

## 與相鄰案的邊界
- 狀態欄容器與無卡桌預設欄位歸 [table-status-panel](table-status-panel.md)；戰鬥數值（先攻、HP 加減）歸 [ttrpg-combat-tracker](../tasks/ttrpg-combat-tracker.md)；本案管規則書、骰子、角色紙初始宣告。
- 大廳徽章與篩選歸 [lobby-table-badges](lobby-table-badges.md)。
- 原方案 v2 寫「等狀態欄二期」：二期已以 state-values-mvu 結案，前置改成 table-status-panel 的角色格。

## 目前狀態與下一步
- 分支 `ttrpg-rules-system`；方案基底為主線提交的 `0efa23e8`，所有 Git 操作仍由主線做。A 已實作待審，B 尚未施工，尚未合併／結案；本輪未呼叫真模型。〔模型判斷·未裁決〕
- v1 維持骰池＋單人骰鈕＋指南及常識審修正；房間骰鈕、角色紙結構化、嚴格模式與整理精靈留 v2。〔作者裁決 2026-11-08〕
- A 的核心是 `dice.rs`、TurnRecord／prepare_turn_append 單點掛載、GM 共用入口、TailStale 與房間投影；池列由桌面／手機共用 `features/dice` 呈現。單人／房間由 host 明確傳入，不依空名冊猜模式；開關變動透過 GM 凍結 system 改變重開線，清掉舊池。〔模型判斷·未裁決〕
- 本輪核心 359、測試 321 行（新增非空白程式行；扣註解、純型別連帶與十語系譯文；測試接線計入測試）。另有 110 行型別連帶及 122 行共享字典；既有建構式只補 None／false，不改玩法。〔模型判斷·未裁決〕
- 下一步：主線審本輪 diff、在本機重跑 verify／原生截圖，再接 B。不得提交別人的 `.ai/BACKLOG.md`、`.ai/tasks/ttrpg-combat-tracker.md`、`.ai/tasks/mobile-room-host.md`；這三份已用雜湊確認未動。〔模型判斷·未裁決〕

## 驗證與環境限制
- 骰池／保存／CLI session 相關 Rust 測試 11 項通過；桌面相關前端 18 項通過；party 相關 6 項、party typecheck／build 通過。測試涵蓋半截／代落／冪等、收回復原／正文替換保池、session 累積到 8 份重開、即時／seed／sync 相同池、全半形與空白、錯面／跳號／重號與未找到骰號。〔模型判斷·未裁決〕
- 完整 cargo test 已執行：2699 通過／86 失敗／7 忽略；86 項失敗逐項皆有 Operation not permitted（既有 localhost／環境相關測試），不能算全綠。最新骰子相關測試再次跑過 11 項全綠。〔模型判斷·未裁決〕
- npm run verify 第 1–11 步通過：桌面 2005 項、網頁 878 通過／1 跳過、結構／版本／隔離／個資／fmt／i18n／建置；第 12 步 party 224 通過／5 listen EPERM 失敗，第 13–17 步未執行。其後另跑 party typecheck／build 通過；未聲稱 verify 全綠。〔模型判斷·未裁決〕
- harness:build 成功；以隔離 `/tmp/ttrpg-a-harness` 啟動時，控制鎖檔開啟遭 EPERM、app 以 78 結束，未取得原生截圖。主線需在本機執行測試通道驗未引用／正常取用／錯面紅字與手機版池列。〔模型判斷·未裁決〕
- 本機證據在 `/tmp/ttrpg-a-*.log`；行數分類在 `/tmp/ttrpg-a-metrics.json`。未改 Git 權限／設定，未寫 `.git`。〔模型判斷·未裁決〕

## 本輪提交清單
下列是本輪自己的檔案；大量 Rust 測試／建構式僅補可選欄位。只依此清單暫存，勿整包加入工作區。〔模型判斷·未裁決〕

- `.ai/HANDOFF.md`
- `.ai/handoffs/ttrpg-rules-system.md`
- `.ai/plans/ttrpg-rules-system.md`
- `party/server/core/dice-pool.test.ts`
- `party/server/core/executor.ts`
- `party/server/core/types.ts`
- `party/server/core/validate.ts`
- `party/src/features/room/StoryView.tsx`
- `party/src/i18n/index.ts`
- `src-tauri/src/chat_assembly.rs`
- `src-tauri/src/chat_assembly/greeting/tests.rs`
- `src-tauri/src/chat_turn/arrivals.rs`
- `src-tauri/src/chat_turn/gm.rs`
- `src-tauri/src/chat_turn/party_batch.rs`
- `src-tauri/src/chat_turn/party_batch_tests.rs`
- `src-tauri/src/chat_turn/relay.rs`
- `src-tauri/src/chat_turn/solo_relay.rs`
- `src-tauri/src/chat_turn/vn_reroll_tests.rs`
- `src-tauri/src/commands/scene.rs`
- `src-tauri/src/commands/world.rs`
- `src-tauri/src/data/character/tests.rs`
- `src-tauri/src/data/format/tests.rs`
- `src-tauri/src/data/message_vars/mod.rs`
- `src-tauri/src/data/message_vars/tests.rs`
- `src-tauri/src/data/message_vars/turn.rs`
- `src-tauri/src/data/message_vars/write.rs`
- `src-tauri/src/data/scene/branches_tests.rs`
- `src-tauri/src/data/scene/export.rs`
- `src-tauri/src/data/scene/lifecycle.rs`
- `src-tauri/src/data/scene/lifecycle/tests.rs`
- `src-tauri/src/data/scene/marker/tests.rs`
- `src-tauri/src/data/scene/presence.rs`
- `src-tauri/src/data/scene/replay_tests.rs`
- `src-tauri/src/data/scene/transcript.rs`
- `src-tauri/src/data/scene/transcript/floor.rs`
- `src-tauri/src/data/scene/transcript/tests.rs`
- `src-tauri/src/data/scene/transcript/write_safety_tests.rs`
- `src-tauri/src/data/scene/visibility/tests.rs`
- `src-tauri/src/data/state.rs`
- `src-tauri/src/data/vn_playthrough_tests.rs`
- `src-tauri/src/data/world.rs`
- `src-tauri/src/data/world_info_store/tests.rs`
- `src-tauri/src/data/worldbook/tests.rs`
- `src-tauri/src/dice.rs`
- `src-tauri/src/dice_tests.rs`
- `src-tauri/src/genesis.rs`
- `src-tauri/src/import/tests/card_book.rs`
- `src-tauri/src/import/web_save/mod.rs`
- `src-tauri/src/import/web_save/tests.rs`
- `src-tauri/src/lanes/scaffold_tests.rs`
- `src-tauri/src/lanes/tests.rs`
- `src-tauri/src/lanes/tests/tail_threshold.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/mechanism/test_support.rs`
- `src-tauri/src/mvu_engine/wiring_tests.rs`
- `src-tauri/src/receipts/tests.rs`
- `src-tauri/src/refactor/tests/flat_field_reporting.rs`
- `src-tauri/src/refactor/tests/interface.rs`
- `src-tauri/src/refactor/tests/played_gate.rs`
- `src-tauri/src/refactor/tests/rerun.rs`
- `src-tauri/src/refactor/tests/takeover_flat_keys.rs`
- `src-tauri/src/refactor/tests/visibility.rs`
- `src-tauri/src/room.rs`
- `src-tauri/src/room/run.rs`
- `src-tauri/src/room/seed.rs`
- `src-tauri/src/room/tests/sync.rs`
- `src-tauri/src/scene_budget/measure.rs`
- `src-tauri/src/scene_budget/summarize/tests.rs`
- `src-tauri/src/scene_budget/tests.rs`
- `src-tauri/src/transport/test_support.rs`
- `src-tauri/src/vn_bible_tests.rs`
- `src-tauri/src/vn_branches_tests.rs`
- `src-tauri/src/vn_work/materialize.rs`
- `src-tauri/src/vn_work/tests.rs`
- `src-tauri/src/world_scan/tests.rs`
- `src/features/dice/DicePoolView.test.tsx`
- `src/features/dice/DicePoolView.tsx`
- `src/features/dice/dice-copy.json`
- `src/features/dice/dice-pool.css`
- `src/features/dice/dice-pool.test.ts`
- `src/features/dice/dice-pool.ts`
- `src/features/play/PlayView.tsx`
- `src/shared/contracts/backend-contracts.ts`

## Constraints
- app 永不內建、永不散布規則內容；指南只指開放授權來源與社群檔。
- 不替玩家花錢：燒額度的功能先報價、玩家確認才跑。
- 骰子誠實底線：骰面一律 app 真隨機產生，模型只能取用；模型自報永不做。
