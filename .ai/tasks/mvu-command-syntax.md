# 未重構 MVU 卡指令式更新（_.set）被靜默略過

Status: todo

## Summary
Rust 端 `mechanism/parse.rs` 只解析 JSONPatch。未重構 MVU 卡若有抽不出規則、照原卡送給 GM 的說明條目（`import/mechanism.rs:323`）教 GM 寫 `_.set(...)` 等指令式語法，GM 照寫的整段會被靜默略過，沒有其他路徑接住。照酒館行為補上指令式語法解析（上游 MVU 的 parseMessage 行為）或至少回饋。結論來自 mechanism-error-feedback 審查時讀程式，尚未拿實卡確認。

## Next action
未排程；開工首步先找一張實際這樣寫的卡確認。
