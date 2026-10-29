# initvar-entry-yaml-semantics：[initvar] 條目挑選與 YAML 語意對齊上游

沒重構的 MVU 卡，[initvar] 的讀取與上游 MagVarUpdate 有兩處不同（initvar-yaml-list 立案時發現）：

1. **條目挑選**：上游合併所有標題含 `[initvar]` 的世界書條目（不論啟停），並剝掉 `<initvar>` 標籤與 ``` 程式碼圍欄；本地 `import/mechanism.rs` 只取第一條停用且開頭是 `[initvar]` 的條目。
2. **YAML 語意**：清單外的 `$meta:` 區塊（DongeonMaster 的 initvar 有）在樹模式留成分支；行首 `#` 在 YAML 是註解，本地卻當記號剝掉後解析成鍵。
3. **額外世界書 initvar 補回**：開場白 `<initvar>` 覆寫後，上游會重跑角色書以外世界書的 initvar 補缺；桌面（開場白前就匯入的額外世界書被一起蓋掉）與網頁一起在本案修〔作者裁決 2026-10-27〕（自 initvar-nonzod-overwrite 移入）。

## Next action
開工先確認 initvar-yaml-list 的嚴格模式解析已進 main，在其上補齊。
2026-10-29 輕防重審：縮小〔模型判斷·未裁決〕。第 3 項照〔作者裁決 2026-10-27〕做。第 1、2 項最小版＝合併所有標題含 `[initvar]` 的條目、剝 `<initvar>` 標籤與程式碼圍欄、行首 `#` 當註解。
刪掉的部分成已知限制：清單外的 `$meta:` 區塊在樹模式留成分支（跟 [mvu-meta-schema](mvu-meta-schema.md) 一起等實卡）。
