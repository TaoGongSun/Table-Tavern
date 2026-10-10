# 世界書路匯入殘留別桌角色 id

Status: done〔作者裁決 2026-10-10：立案，列為網頁版公開前門檻〕。已合併 main（2026-10-11）。方案見 [plans/worldbook-path-foreign-ids.md](../../plans/worldbook-path-foreign-ids.md)（P1–P3 照建議 A 實作，作者尚未拍板）。

## 現況
- 落點：`src-tauri/src/data/worldbook/book_import.rs` 世界書路的名單只留本桌 id，濾空或讀不懂寫成 gm；來源卡只留本桌 id，但明寫 gm 的原樣保留，用來落定。
- 測試：`import/foreign_ids_tests.rs`、`import/web_save/tests.rs` 的 `worldbook_route_drops_foreign_character_ids`、`data/worldbook/book_import_tests.rs` 的壞格式那條。

## 下一步
無。Sol、opus-review 與第二位 Opus 三方驗收通過，已結案。
