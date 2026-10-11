> 結案 2026-10-12：verify 12 步綠；三方驗收 PASS。

# [initvar] YAML 清單在樹模式桌被攤平

Status: done

## Summary
`[initvar]` 改用 `transport/state_yaml/` 的 initvar 清單模式（`parse_initvar_tree`）建初始樹：清單存成 JSON 葉子（與 stat_data 投影、行內陣列一致），清單項照 YAML 判鍵值、空項佔索引、引號值是字串、流式序列。狀態區塊解析不變。規格、測試、已知限制見 [plans/initvar-yaml-list.md](../../plans/initvar-yaml-list.md)。
