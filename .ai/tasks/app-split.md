# Task
Task-ID: app-split
Title: App.tsx 拆分：三批 15 切片（元件搬移→controller hook→受控 view）
Status: in-progress
Created: 2026-08-13T11:05:03.565219+08:00
Updated: 2026-08-13T11:05:03.565219+08:00

## Summary
App.tsx 6890 行（佔前端 9438 行的七成）拆成多支檔案。目的是維修與管理，不追求任何運行時效益，拆分過程不改行為。目標尺度：一支檔案 200–800 行，App.tsx 收到 400–600 行（依據：專案除 App.tsx 外最大 583 行）。

方案經 Claude（Opus 5）與 Sol（gpt-5.6-sol，Codex CLI）四輪往返達成共識，2026-08-13 使用者拍板三批全做。規格細節（三批範圍、五個 controller 的 contract、五條硬約束、共用符號歸屬、16 項回歸清單與切片對照表）見 [plans/app-split.md](../plans/app-split.md)。

15 個切片：1 前置（check-i18n 掃描範圍）／2–5 第一批元件搬移（CardEditor+CropDialog、WorldEditor、SettingsWindow+UsageTab、atoms）／6–11 第二批 controller（cardInterface、tableState、characters、chat、imports、GenerateTableDialog）／12–15 第三批受控 view（TableSidebar、WorkspaceHeader、PlayView、MainView）。

進度量測：`node scripts/app-structure.mjs` 重跑即可，輸出 App() 行數／state 數／區塊數。立案基準＝App() 3981-6888（2908 行）、59 state／13 ref／105 區塊。

## Next action
- 切片 1（前置，必須先做）：`scripts/check-i18n.mjs:35` hardcode 只讀 src/App.tsx，改成遞迴掃 src 下所有 .tsx，改完驗證按鈕數仍為 99、全語言全綠——不先做這步，第一批搬走元件後 i18n 按鈕寬度檢查會靜默失效（照樣印 OK）

## Constraints
- 一個對話做 1–2 個切片；15 切片預估 8–12 個對話。純讀寫量第一批約 100k、第二批 200–300k、第三批 80–120k tokens，單一對話裝不下一整批。每個對話結束要產出下一棒的起手提示詞。
- 每切片收尾：`npm test`＋`npm run build`＋`npm run check:i18n` 三綠，加該切片的針對案例與固定的 A→B→A 換桌檢查。check:i18n 的按鈕數只允許持平或上升，下降即代表漏掃。
- 每批完成跑完整 16 項手動回歸（清單在 plans/app-split.md）。
- 手動回歸分工（2026-08-13 拍板）＝混合：機械項（換桌、匯入後刷新、開關面板、改設定、桌次增刪改名）由 Claude 用 computer-use 驅動 Tauri 視窗跑；需人眼判斷品質的項目（第 5 項開場白翻譯語感、第 6–7 項聊天與換幕品質）由使用者跑。Tauri app 的 invoke 需要 runtime，瀏覽器工具打不開，不可用 dev server 代替。
- 零元件測試（8 支測試全是純函式）是本任務最大風險，手動回歸清單是唯一安全網。
- App.tsx 最終落在 440–570 行，不追求更小：桌次生命週期 9 支 handler 含 enterTable 約 166 行必須留在 composition root。
- commit 以切片為單位，訊息格式 `app-split: 做了什麼（驗證結果）`。
