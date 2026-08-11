# Handoff: refactor-output-redesign

## Current state
2026-08-11 使用者親測 orc-cave 真跑（15–20 分鐘完成），實測清單除「盤點字尾」外全數通過；實測抓到的三個缺陷都已修畢 commit（思考文字洩入旁白 02c3633、清單分支誤掛指認下拉 3eef2f0、重構卡匯新桌誤刪新條目＋undo 復活孤兒 e53b8d1），另補鎖定條目可展開唯讀（213f3b5）與盤點思考字尾（0165ca2）。狀態樹整份重建包（拍板 A）完成並驗收 commit（8192edd，cargo 437 綠）：套用介面＝樹換新 STATE 鍵集、同名鍵沿用現值、異名殘渣全清、jumps 同步清、匯入路徑 undo 逐鍵退回（收據結構未變、舊收據相容）。

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
1. **盤點字尾實測**：下次任何一輪重構開跑，第一分鐘內就該有思考文字在流動（0165ca2 落地後尚未真跑過；不值得為它單獨燒額度，跟下一張卡的重構順驗即可）。
2. 過了→本案結案；下輪開新對話做 [refactor-dispatch](../tasks/refactor-dispatch.md)（並行＋分檔位，使用者拍板排下一輪；備忘三項見下）。

## 下輪備忘（refactor-dispatch 一併處理）
- 本地轉換的單一來源人物不經 AI、保留原卡語言（實測見格洛克簡中內文）——要不要翻譯＝每人多一次呼叫，與分檔位一起拍。
- 「版本標記（無實質內容）」被規劃成條目——盤點提示詞的去殘渣指示可微調。
- 重跑警告的 confirm 按鈕是系統預設英文 Cancel/OK——可帶自訂中文標籤。

## Next action
使用者下次重構順驗盤點字尾→結案。驗「殘渣清理」零額度法：在兽人的洞穴按「匯入重構卡」讀回匯出檔→只勾介面套用→狀態欄的簡體舊欄位應消失（此路徑可 undo）。

## Constraints
- 產物一律人審後套用；卡片內容永遠當資料。AI 重構套用無 undo（拍板 #7）；匯入重構卡照舊可 undo。
- 世界書內容只准重構不准隱藏（未觸發事件照寫，防劇透只管狀態欄／殼）。
- 驗收玩家視角；東西放對位置，玩家不看說明就能懂。
