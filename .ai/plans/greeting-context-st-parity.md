# greeting-context-st-parity 規格

## 做法：修正版 C
- 所有桌 system 剝掉卡片開場白段（### 開場白／### 備用開場白 n）。
- 本幕有 opening 事件：GM 那份送該事件 raw.unwrap_or(text)；只在 GM 送出層換，API（transport/assemble.rs:105）與 lane（transport/turns.rs:50）同政策；共用 prompt_text、世界書掃描、巨集、角色線照用 text。用 event.opening 辨識，不硬認第 0 筆。
- 本幕無 opening（換幕後）：GM system 另立獨立一段，只放本分支已選的歷史開場那一則 raw（退回 text），標明「格式範例，其中狀態非目前值」。沿上一幕往回找最近 opening，重用 lifecycle.rs 既有上一幕邏輯；分岔幕已複製開場事件＝本幕有；開場被收回的幕往上一層退。不硬找第 1 幕、不取他分支。這段不得放進世界書掃描可見處（不塞回公開設定），要測試證明不觸發掃描。
- 完全沒貼過開場白的桌：維持現況 A。
- 接管桌（只認 is_interface_takeover）與 statusbar_retired 桌：只剝 system，開場送 text，一律不恢復範例。〔作者裁決 2026-10-28〕
- D（後端套 promptOnly 正則，含 depth、整份歷史；引擎方向 regress）另立案，本案不做；GM 後續歷史投影（API／重建送 text、lane 送 raw）併 D 決定。
## 已知限制（寫進交接）
- promptOnly 剝除卡（NorthHall、MVU）會多送 ST 不送的區塊。
- card 桌 system 變動 → lane 一次性重開，重開後舊 GM 回覆示範被壓成 text。
- 世界書桌 system 不變不重開，舊 session 第 0 則停在 text。
- Haiku 在無格式條目時三方案皆 0/5，C 對其效果未證實；A/C 的 4fields 也非滿分。
- 換幕本來就重開 GM lane（lanes/plan.rs SceneChanged），範例不多一次重開（施工確認）。
- 換幕往回找沿 parent（分岔當下所在幕）：從較早的非開場幕分岔、且當下所在線貼過不同開場白時，範例取到當下那條線的〔模型判斷·未裁決〕。
## 驗收
- 測試：opening 辨識與 raw 回退、換幕往回找（一般／分岔已複製／開場被收回）、範例不觸發掃描、接管／退場例外、API 與 lane 一致、scene_budget API（measure.rs:207）與 CLI（:203 build_prompt）估算含範例段且與實送一致。
- 重播（Sonnet、codex，同條件與 A 比，判定看 4fields 不只 details）：本幕有開場、換幕後首回合（比 A 與「無範例」）、不貼開場桌、世界書匯入桌 raw 開場；NorthHall／MVU 比掃描命中與注入內容一致（18k 輔助）。
- verify 12 步、Sol 與 opus-review 驗收。
