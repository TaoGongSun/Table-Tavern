# 介面桌換幕：前情提要進正文槽、面板與狀態樹原樣續存

本檔存放 [interface-scene-change](../tasks/interface-scene-change.md) 的規格細節，由任務檔 Summary 連回。2026-08-13 立案；2026-08-14 Sol 第 1 輪覆核結論併入（驗收順序矛盾修正、recap 防劫持改雙層、換幕入口定案）。

## 目的（2026-08-13 使用者定調）

機制複雜、輸出長的介面卡（毛絨卡 Transfur 型）幾輪就把 context 撐爆，沒有換幕配套就不敢長玩。目標行為：換幕後介面照常運作，【前情提要】顯示在介面呈現故事的位置（正文槽），其餘面板完整保留、數值不歸零，後續回合照舊走 patch 契約。

## 現況事實（2026-08-13 盤點）

- 換幕（[lib.rs:2587](../../src-tauri/src/lib.rs) `advance_scene`）＝把本幕 transcript 摘要成【前情提要】，`begin_next_scene`（[data.rs:2516](../../src-tauri/src/data.rs)）寫成新幕第一則 Narration 事件。
- 狀態樹的本地權威在 `state.json`（`world.state`），換幕不動它；GM 每輪系統提示由 `render_state_tree`（[transport.rs:505](../../src-tauri/src/transport.rs)）注入現值。**狀態與提示注入天生跨幕存活**——這是缺口比預想小的原因。
- 前情提要事件寫入時 `state: None`，但 `append_transcript` 對未帶快照的事件會補上現行快照（data.rs 測試覆蓋）→ 檯面樹換幕後應不變。【待實測】
- 殼的餵入是 direct-first：訊息先試卡 regex，不中就用骨架填 `{{本回合.正文}}` → 純文字前情提要理論上自然落入正文槽。【待實測】
- 三條補救路皆存在且有守門：`revert_scene`／`regenerate_scene_summary` 都要求該幕只有前情提要那一則（有新內容即擋）；`revert_scene` 會把 `state.state` 還原成前幕最後一則事件快照（抽驗確認）。

## 設計（含 Sol 補強）

1. **摘要防 regex 劫持＝雙層**：主層＝前情提要事件帶明確 origin 標記，殼渲染遇到它**跳過 direct-first、必走正文槽**（防摘要文字被卡 regex 抓走整頁報廢，「第二輪地圖全毀」教訓同源）；第二層＝接管桌的 `summary_messages` 加「純敘事、禁任何標記」約束。
2. **換幕入口**：覆蓋層工具列（[CardInterfaceOverlay.tsx](../../src/views/CardInterfaceOverlay.tsx)）是宿主 React 元件，關閉鈕旁直接加「換幕」鈕；不需替卡內 HTML 加 postMessage 橋。
3. **補救路語意明訂**：退回前幕＝樹還原成前幕最後快照（現行已如此）；重寫提要＝只在幕首一則時可用（現行守門）；**分岔＝狀態樹回到來源幕最後快照**，不保留分岔前的現值——接管桌逐條驗。

## 驗收（Sol 修正：拆兩條獨立案例，原稿「續玩後退回／重寫」會被守門擋下）

- **案例 A（換幕當下）**：西幻接管桌玩數回合→換幕→介面開著、前情提要在正文槽、五分頁與地圖原值→立即各驗一次「重寫提要」「退回前幕」，介面照常、樹正確還原。
- **案例 B（換幕後續玩）**：換幕→續玩兩回合，patch 契約正常、面板跟動。
- 每項驗收同時比對 `state.json` 與畫面，不能只看視覺。
- 同流程在 Transfur（等該型過接管後）；換幕前後每輪輸出量記錄（省 context 是本案存在理由，要有數字）。

## 開工

使用者說「開工」＝主線照下方分包發包（執行者開工當下由使用者點名）、收貨驗證留主線；包 1 兩個【待實測】需使用者在場（開 app 真換幕一次、花一次 GM 呼叫）。

## 分包（草案）

1. 兩項【待實測】零額度實測，結果回填本檔。
2. 摘要 origin 標記＋安全渲染（跳過 direct-first）＋摘要提示詞約束。
3. 三條補救路（退回／重寫／分岔）在接管桌分開逐驗。
4. overlay 換幕鈕＋WestFantsy／Transfur 端到端驗收。

## 待確認

- 前情提要是否同時仍在聊天欄顯示（現行行為）。預設維持雙處顯示。

## 關聯任務

- [refactor-mode-split](refactor-mode-split.md)：本案只服務介面優先軌；角色優先軌用現行換幕即可。
- [interface-takeover-spike](../tasks/interface-takeover-spike.md)：待辦 2 逐型驗卡決定 Transfur 驗收時點。
- [scene-fork](../handoffs/archive/scene-fork.md)：分岔互動屬本案設計第 3 點。
