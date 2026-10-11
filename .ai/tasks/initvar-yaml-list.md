# [initvar] YAML 清單在樹模式桌被攤平

Status: todo

## Summary
`[initvar]` 條目經 `parse_indented_fields` 建初始樹（`import/mechanism.rs:297-300`），YAML `- ` 清單同樣會被攤平或略過。statusbar-yaml-list-flatten 只修狀態區塊；樹模式桌的 initvar 清單應改用同一套清單解析存成數字索引分支，變數模式（stat_data 陣列）維持原樣。

## Next action
未排程；statusbar-yaml-list-flatten 結案後再排，開工先找有 `- ` 的 initvar 實卡確認。
