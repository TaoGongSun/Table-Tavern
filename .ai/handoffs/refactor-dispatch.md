# Handoff: refactor-dispatch

## Current state
2026-08-11 目標與工作清單定案（本檔），**未實作**。立案（08-08）後管線經 refactor-output-redesign 改形，本次已按現行程式碼重新查核並修正目標；一項並行拓撲待拍板。實作前置照舊＝[ai-card-refactor](../tasks/ai-card-refactor.md)＋[person-promote](../tasks/person-promote.md) 實測結案（品質基線），唯包 2（取消）無前置可提前。

## 現場查核（2026-08-11，行號以此為準）
- 管線現況四階段：survey → 人物展開 → PLAN 逐條重寫（`refactor_rewrite_entry`，output-redesign 新增）→ 介面展開。**立案時的 finish 收尾階段已不存在**，原「finish 下放 fast」目標作廢。
- 四個 command 取 `transport::gm_tier`：[lib.rs:574](../../src-tauri/src/lib.rs#L574)（survey）、621（expand）、663（expand_person）、710（rewrite_entry）。`stream_via_transport` 本來就吃 tier 參數（[lib.rs:1239](../../src-tauri/src/lib.rs#L1239)）→ 檔位參數化只換呼叫端的值，不動傳輸層。
- 檔位退路慣例：[lib.rs:904-910](../../src-tauri/src/lib.rs#L904)（translate_opening：API 模式未設該檔模型→退 gm_tier；CLI 一律有內建對應）。
- 前端序列迴圈：[App.tsx:2111](../../src/App.tsx#L2111) `runAiRefactor`，三段迴圈＝人物 2148／重寫 2175／介面 2208。**立案後新增 `knownFields` 逐呼叫累積**（2145、2199、2231，欄位命名單一權威）——重寫與介面兩段有順序依賴，是並行的新約束。
- 取消現況：[App.tsx:2260](../../src/App.tsx#L2260) 只擋「還沒發的下一條」；後端無 abort。子程序 [cli.rs:707](../../src-tauri/src/cli.rs#L707) spawn 無 kill_on_drop、無在途註冊表；app 無 exit handler（[lib.rs:2662](../../src-tauri/src/lib.rs#L2662) `.run()` 無 callback）→ Cmd-Q 孤兒實錘。
- 快取紅線安全：[refactor_ai.rs:14-17](../../src-tauri/src/refactor_ai.rs#L14) 全部呼叫共用同一份 system，階段差異（含 known_fields）都在 user 訊息 → 並行與檔位下放皆不碰 system 組裝。
- 用量 JSONL 並行安全：[usage_log.rs:241-244](../../src-tauri/src/usage_log.rs#L241) O_APPEND 單次寫、行遠小於 4K，多路同寫不打架。

## 目標（確定版）
1. **並行省時**：survey 先行不變（快取首發），之後展開類呼叫改有界並行（上限 4 保守起步），分鐘級等待壓到一分鐘上下。拓撲見待拍板。
2. **檔位省費**：survey 留 GM 檔（認人／規劃是智力核心）；expand_person／rewrite_entry／expand 下放 balanced（單價 −40%）；API 模式未設 balanced 模型退 GM 檔（translate_opening 慣例）。
3. **取消真停＋孤兒清理**：取消＝中止全部在途呼叫（CLI 殺子程序、API 斷流即停止計費），已完成的照樣進結果卡；app 退出殺全部在途子程序。

## 並行拓撲（2026-08-11 拍板＝A）
人物佇列並行（彼此獨立、不碰 knownFields）；重寫→介面維持序列鏈（欄位命名單一權威零改動）；兩線同時跑。為何選 A：唯一不犧牲 knownFields 品質語意的切法；牆鐘＝max(人物線, 重寫介面線)，人物多的卡省最多。

## 工作包清單（分工＝2026-08-11 拍板，本輪限內部模型、不用 Codex）
- **包 1 檔位參數化**（後端，小；**主線直寫**——四呼叫點換值＋小 helper，委派比直寫貴）：四 command 依呼叫類型給 tier，balanced 退路照 translate_opening 慣例；cargo 單元測試（API 未設 balanced 退 GM、CLI 一律 balanced）。品質把關靠包 4 A/B，先不做使用者設定項。
- **包 2 取消中止＋孤兒清理**（後端為主，無前置可提前；**外包 general-purpose `model: sonnet`**，主線出規格＋複驗）：在途子程序註冊表＋`kill_on_drop(true)`；新 command `refactor_abort`（world 範圍殺全部在途：CLI kill child、API CancellationToken select 掉 stream）；被中止呼叫回「已取消」錯誤，前端與失敗分流（不列失敗名單）；tauri exit handler 殺註冊表全部子程序；cargo 測試＝假 CLI 長睡腳本 abort 後程序確實死、註冊表清空。
- **包 3 前端有界並行**（依賴包 2 的 abort；**外包 general-purpose `model: sonnet`**，主線出規格＋複驗，比照 opening-translation 前例）：照 A 拓撲改 runAiRefactor；**balanced 首發先行**——第一發 balanced 完成（建快取）後才放行並行，避免 N 路同時全額 cache write；進度改「完成 x/n」＋思考字尾共用緩衝（任一路增量＝活著）；單條失敗略過列名照舊；限流類錯誤單次退避重試；vitest 排程器測試（上限、失敗略過、取消不發新）。
- **包 4 A/B 驗收**（**主線＋使用者，不外包**——品質費用判讀＝驗證即重做；實機操作由使用者）：與基線同卡（orc-cave）對比牆鐘時間、prompt-cache.jsonl 費用、機制產物品質；機制品質不行→rewrite_entry 單獨升回 best 再比；取消三場景實測（盤點中取消立即停不燒完、並行中取消全停且已完成保留、Cmd-Q 後 `ps` 無殘留 CLI 程序）。

順序：包 1 → 2 → 3 → 4（1、2 互相獨立可對調；3 依賴 2；4 收尾）。

## Next action
等 ai-card-refactor＋person-promote 實測結案（基線落地）→ 照包 1 起工；包 2 若想先修取消痛點可隨時單獨開工。

## Constraints
- survey／expand 共用 system 逐位元組相同的快取紅線照舊，不碰 system 組裝。
- 下放檔位後的展開品質須與實測基線 A/B 比對後才定案；機制重寫（EJS→JSON 契約）是最敏感點。
- knownFields 欄位命名單一權威不得犧牲（除非拍板 B 明示接受）。
- 並行上限保守值 4 起步，實測穩定才考慮上調。
