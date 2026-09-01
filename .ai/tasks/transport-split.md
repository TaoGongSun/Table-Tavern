# transport.rs 拆進 transport/

## Summary
`src-tauri/src/transport.rs` 5472 行（本體 2198／同檔測試 3274），data-split 之後的次大檔。本體拆成 `transport/` 底下 8 檔：messages／state_view／context／assemble／turns／arrivals／response／client。

原以為的「組裝／解析／傳輸」三段是假的——前半其實放了很多後段會呼叫的 helper。2026-09-01 開工前複核後再調一處：`gm_dynamic_block` 跟 `TreeRender`／`render_state_tree` 一起放 `state_view.rs`，避免為了跨 sibling 呼叫把 state-render implementation detail 整包升可見度。

完整依賴與切線見 `.ai/plans/transport-split.md`。`transport/mod.rs` 當 facade，外部既有 `transport::X` 路徑全部維持不變。叫 `turns.rs` 不叫 `lanes.rs`，避開 crate 根既有的 `lanes.rs`。86 支測試同步搬，夾具抽 `transport/test_support.rs`。

## Progress
2026-08-26 與 data-split 一起立案；2026-09-01 完成開工前規格複核並修正依賴圖、`gm_dynamic_block` 落點、client 清單與可見度白名單。data-split 已完成並 commit，本案前置條件已滿足。尚未動工。

## Next action
正式動工時沿用 data-split 的基準抓取與完整驗收流程：production item byte 對帳、pub 簽名、facade 完整性、86 支測試 leaf/body、`private_interfaces`、cfg／可見度差異列帳。先純搬檔、保持原可見度編譯，再依 E0603 逐項做最小放寬。

## Constraints
- 純搬家，production body 一個字不改；不趁機重構、改名、收斂重複或改邏輯。
- 白名單只有：`transport/test_support.rs` 新檔；測試專用可見度；因 sibling module 分拆被編譯器迫使產生的 production 最小 `pub(super)`／`pub(crate)`；`transport/mod.rs` facade、module／re-export／import plumbing；搬檔造成的必要相對路徑調整。
- production 可見度不預先過度公開：以 E0603 實際要求逐項放寬到最窄範圍，最後完整列帳。
- `transport/mod.rs` 必須保住拆前所有對外 `transport::X` 路徑與可見度；編譯通過本身不能取代 facade 完整性驗證。
- 不與其他 refactor 綁同一波實作或同一 commit。
