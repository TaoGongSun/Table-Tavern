# mvu-analysis-section-replay — <Analysis>／思考區的指令也被執行

## 狀態
- 根因：共用 MVU 核心 `src/features/mvu-engine/source-commands.ts` 的 `extractCommands` 照釘版 MagVarUpdate `438f9ffc` 整段掃，`<Analysis>` 裡引用的 `_.set` 與 JSONPatch 同一條各跑一次（實測佇列第 23 項）。上游 `5cf5685a`（2026-10-06）起遮掉思考區與 JSONPatch 區塊再掃舊式指令。
- 定案（Sol、Grok 共識，做法 B）：只改 `extractCommands`——同一份等長遮罩視圖先遮 think／thinking／reasoning／analysis／analyze（不分大小寫、可帶屬性、堆疊只彈同名最近一層、未閉合遮到結尾），在遮罩視圖找 JSONPatch，再把 JSONPatch 區塊遮掉才掃舊式指令；參數、patch 內容、full_match 一律從原文切。三個宿主（桌面 QuickJS、卡片 iframe、網頁）共用同一份核心。
- 已知限制（上游已處理，本案不做）：引號／YAML／註解裡的假標籤會被當真標籤——例如值裡合法的 `"<think>"` 會被當未閉合思考區，後面真正的更新一起被遮掉不套用；巨集代換後名字碰巧含標籤同理。

## 結案
2026-10-28 結案：verify 綠；engine-source 8 項、source-zod 1 項新測試；Sol、Grok 驗收 DONE。
