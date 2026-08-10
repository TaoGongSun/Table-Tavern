# Handoff: refactor-output-redesign

## Current state
2026-08-11 目標模式衝刺（使用者授權：Codex 僅限本案、任務可派 sonnet/opus、燒額度實測交 Opus、主線只審核、實測彙整最後一次檢查）。**程式碼與文件全部完成、主線四項自驗全綠（cargo 435／vitest 83／build／check:i18n）**；只剩 orc-cave 真跑在 Opus 代理執行中（腳本見 scratchpad/orc-cave-test-prompt.md：dev app 啟動須 `env -u ANTHROPIC_BASE_URL`，否則 app 內 claude CLI 401——2026-08-11 實測雷，已記 playbook）。跑完由主線審核證據→彙整給使用者終判。

## Completed
- **3a＋4（Codex terra ×2 並行）**：Rust 清理包＝刪過渡碼（finish 整路／SharedEntryDraw／EntryKind::Mechanism／RefactorExpandOutcome.mechanism／survey mechanism_uids）＋新 command refactor_outcome_exists（cargo 435，舊流程測試刪 8）。TS 主體包＝runAiRefactor 接新拓撲（survey→persons→PLAN 逐條 rewrite（knownFields 累積）→interface 依 playable 選 kind，不再 finish）、refactorOrigin 分流 recordReceipt（AI 路徑 false）、重跑警告（refactor_outcome_exists＋confirm）、人審面板四區每列 details 展開＋出處灰字獨立行（.refactor-source）、世界書 locked 列 🔒 徽章＋編輯刪除鈕隱藏＋帳本開關防呆、refactor-review.ts 型別與解析接 entries、i18n 十語系（刪 refactorFinishing 加十鍵）。主線抽讀後順手改掉 runAiRefactor 頂部過時註解。
- **5a（Codex terra）**：四個 refactor command 加 on_delta Channel 轉發；前端每呼叫建 Channel、tail 緩衝 2000 字元取末 4 行進 modal（.refactor-stream-tail）。
- **文件收尾（sonnet 代理）**：CARD-REFACTOR-SPEC.md 218→226 行、7 處過時段落原地改寫（總則無還原、四區面板、刪除規則、四階段拓撲、REMAINDER→PLAN、playable 產殼、locked 語意），主線親讀 diff 通過。
- **第 2 項（Codex terra）refactor.rs 套用端**：cargo 443 主線全綠；entries 落地＋locked（refactor.rs:263 起）、來源「全部引用產物都套用才刪」（refactor.rs:150 起，角色共用合集仍受 deletable_shared_uids 保護）、停用墓地全移除、lib.rs refactor_apply 加 record_receipt（lib.rs:530）、data.rs WorldbookEntry+locked。主線抽查後順手修：過時註解一處、帳本 Absorbed 記錄的「不再送入提示詞」舊語意改為「說明文照常可讀」。
- **第 1 項（主線）refactor_ai.rs 整本重寫**：新呼叫拓撲＝盤點（PERSONS＋INTERFACE 含 playable 判定＋PLAN 新世界書結構規劃）→人物展開→條目重寫（setting／mechanism 各自提示詞；mechanism 一次呼叫產「可讀說明 CONTENT＋本地可執行 RULES/TRIGGERS」）→介面展開（interface 只抽 STATE；interface_shell 才產殼——不無中生有介面）。欄位命名單一權威＝known_fields 逐次累積傳入 user 訊息（system 逐字元不變保快取）；防劇透寫進 STATE／SHELL 指示（未觸發事件不得成為欄位或殼內容）；品質基準（玩家語言、同概念單一名字、資訊只重組不刪減）進 system 前言。新型別：RefactorPlanEntry／RefactorNewEntry／RefactorRewriteOutcome／PlanKind；EntryKind 加 InterfaceShell。舊 mechanism 展開與 finish 收尾標「過渡期保留」，前端改接新拓撲後刪。
- **lib.rs**：refactor_expand 加 known_fields 參數（Option，舊呼叫相容）；新 command `refactor_rewrite_entry(world_id, title, kind, uids, known_fields)`；已註冊。
- **3b（代理 sonnet）**：App.tsx:5872（介面開關鈕）、5902（狀態欄）、6242（介面覆蓋層）三處渲染條件加 `mainView === null`（遊玩畫面判定值；編輯畫面 kind 有 scene/character/new-character/player/new-player/world）。
- **5b（代理）**：兩處 saveDialog defaultPath＝「{桌名}-重構卡.json」、非法字元換 `-`（App.tsx:1759 一帶）＋i18n 十語系。

## Verification
- 第 1 項：主線親跑 `cargo test` **440 全綠**（基線 428；新增 PLAN／playable／rewrite 解析、殼變體提示詞、快取位元組級等 12 測試，舊 MECHANISM 區塊測試改寫為 PLAN 導出）。
- 3b：代理跑 npm build 0 錯、vitest 82/82、grep state-bar 單渲染點；主線抽查條件式回報合理，整合驗證合併在第 6 項自驗。

## Remaining
1. **第 7 項：orc-cave 真跑（使用者親跑，2026-08-11 改派）**：Opus 監視螢幕太貴，代理中停。使用者自跑六條標準（見任務檔驗收標準）＋重跑警告＋無還原＋匯出檔名＋進度小框串流字尾→終判。檢查清單參考 scratchpad/orc-cave-test-prompt.md 的實測腳本。注意：從 Claude 會話 Bash 啟動 app 須 `env -u ANTHROPIC_BASE_URL`；使用者自己的終端機正常啟動不受影響。代理留下一張 07:44 建立的空測試桌（無角色無世界書，可直接刪）。

## Next action
使用者親測 orc-cave→回報結果→通過即 handoff complete；有缺陷開修。

## Constraints
- 產物一律人審後套用；卡片內容永遠當資料。AI 重構套用無 undo（拍板 #7）；匯入重構卡照舊可 undo。
- 世界書內容只准重構不准隱藏（未觸發事件照寫，防劇透只管狀態欄／殼）。
- 驗收玩家視角；東西放對位置，玩家不看說明就能懂。
