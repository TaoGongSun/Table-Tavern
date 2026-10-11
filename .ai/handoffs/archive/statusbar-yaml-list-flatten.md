> 結案 2026-10-12：squash 進 main，三方驗收 PASS。

# statusbar-yaml-list-flatten — 狀態區塊 YAML 清單攤平

規格、測試與驗證結果見 [plans/statusbar-yaml-list-flatten.md](../../plans/statusbar-yaml-list-flatten.md)。

## 結論
- 未重構桌的狀態區塊（`<Status_block>`／狀態圍欄／狀態 `<details>`）改由 `transport/state_yaml.rs` 解析：`- ` 清單存成 `0..n-1` 索引分支，每次狀態區塊整支替換（清單身分黏著、變數模式只動不存在／空／索引分支的容器）。多個區塊依原文位置合併，最後一塊勝出。
- GM「目前狀態」在全量桌把索引分支印回 `- ` 清單、多行葉子印 `鍵：|`，與解析器往返一致；狀態欄清單項不掛分支指認。
- `[mvu_update]`、`[initvar]` 仍走 `parse_indented_fields`；`[initvar]` 清單另案 [initvar-yaml-list](../../tasks/initvar-yaml-list.md)。
- 驗證：verify 12 步綠；北境驿站零 AI 開場三人四選項、Haiku 2 回合減人加人整支替換、GM 回寫一致。
