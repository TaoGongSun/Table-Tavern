# structure-finish — 專案結構優化收尾（已結案）

落點表與分包見 [plans/structure-finish.md](../../plans/structure-finish.md)。

## 結果

- `views/`、`controllers/` 裡單一功能的檔案全數歸位到 `features/<name>/`（新增 `play/`、`table-state/`）；`EditPage`、`atoms`（ErrorNote／StoryText）進 `shared/ui/`。兩處只剩組合外殼：`views/` 的 AppDialogs／AppWorkspace／MainView／TableToolbar、`controllers/` 的 useWorkspaceNavigationController。
- `check-structure.mjs` 加 `SHELL_ALLOWED`：views／controllers 任意深度只准清單內的外殼，新增外殼要改 checker。
- Rust `data/worldbook.rs`、`transport/state_view.rs` 測試拆成 `tests.rs`。
- 落點由模型決定〔作者裁決 2026-10-02〕；分包 1–3 由子代理實作、主線驗收；計畫與整案經 Sol 簽收（Grok 額度暫停）。verify 八步全綠：vitest 40 檔／421、cargo 705。
