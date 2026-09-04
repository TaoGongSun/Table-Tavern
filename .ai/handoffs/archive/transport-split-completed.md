# transport.rs 拆進 transport/

狀態：2026-09-03 結案（拆檔 `8f26fb7`、收尾 `587bcd2`）。

## 結果
`src-tauri/src/transport.rs` 5472 行拆成 8 檔加 `test_support.rs`（messages／state_view／context／assemble／turns／arrivals／response／client），原檔刪除，淨增 262 行（模組宣告與 use）。外部既有的 `transport::X` 路徑全部維持不變。

立案時按「組裝／解析／傳輸」畫的三段在開工前複核被推翻——前半放了很多後段才呼叫的 helper；`gm_dynamic_block` 改跟 `TreeRender`／`render_state_tree` 一起放 `state_view.rs`，避免為了跨 sibling 呼叫把 state-render 的實作細節整包升可見度。模組取名 `turns.rs` 而非 `lanes.rs`，避開 crate 根既有的 `lanes.rs`。

收尾把 facade 對外目錄從 52 個名字減成 44 個，刪掉 `PERSON_ARRIVAL_PREFIX`、`DEFAULT_IMAGE_MODEL`、`StreamOutcome`、`extract_delta`、`extract_usage`、`active_worldbook_entries`、`character_state_block`、`LaneTurn`——這 8 個在 `transport/` 之外零呼叫端，掛在 facade 上只換來 5 條 unused import 警告；本體維持 `pub`，在 `transport/` 內都有正式呼叫端，刪除後無新增 dead_code 警告。編譯警告 5 → 0。

驗收：測試函式 539 支拆前拆後同數（macOS 535 passed／0 failed，`install.rs` 那 4 支 `#[cfg(windows)]` 不編入）；`RUSTFLAGS=-Dprivate_interfaces cargo check --all-targets` 零警告零錯誤；實機 smoke test 走 CLI（claude）在既有桌完成四項——玩家送出→GM 串流回覆落檔（cached 5877／prompt 7466）、角色切換、角色發言走 chars 線、state.json 隨回合更新。

規格與白名單見 [plans/transport-split.md](../../plans/transport-split.md)。同型任務＝[mechanism-split](mechanism-split-completed.md)、[refactor-ai-split](../refactor-ai-split.md)。
