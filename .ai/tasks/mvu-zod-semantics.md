# zod 版 MVU 卡的更新語意改以 mvu_zod 為基準

Status: todo

## Summary
mvu-array-index-path 驗收時查到：zod 版 MVU 卡（registerMvuSchema，TestCards 帶 [initvar] 的四張全是）實際由 StageDog/tavern_resource 的 util/mvu_zod.ts 在 `mag_command_parsed_for_zod` 自行 applyCommand 並清空指令，MagVarUpdate 的 applySet／applyInsert／applyDelete／applyAdd 一條都收不到。本地 mvu-replace-numeric、mechanism-error-feedback、mvu-array-index-path 都以 MagVarUpdate 為基準。mvu_zod 差異：set 直接 `_.set`（不做 VWD 第 0 項、不轉 Number、路徑不存在照建）；add 只收數字、無 toPrecision(12)、無 VWD；insert 容器 nil 先試 {} 再 []、頂層不查父路徑；delete 陣列用 pullAt；move 會搬（delete 再 set）；`_` 開頭路徑靜默忽略；每條指令最後過卡片 zod schema（safeParse，含轉型、夾值、預設值），不通過整條不套用。

## Next action
未排程，待作者定方向：照「酒館行為照做」改以 mvu_zod 為 zod 卡基準（需在 Rust 端重現 zod schema 驗證，或改走卡片腳本執行），或維持現行 MagVarUpdate 基準。與 mvu-meta-schema、mvu-command-syntax 相關。
