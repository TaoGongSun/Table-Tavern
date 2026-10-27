# 未重構 MVU 卡指令式更新（_.set）被靜默略過

Status: todo

## Summary
Rust 端 `mechanism/parse.rs` 只解析 JSONPatch。未重構 MVU 卡若有抽不出規則、照原卡送給 GM 的說明條目（`import/mechanism.rs:323`）教 GM 寫 `_.set(...)` 等指令式語法，GM 照寫的整段不會套用（mechanism-error-feedback 已回饋「沒有可解析的 JSONPatch 陣列，整段未套用」）。照酒館行為補上指令式語法解析（上游 MVU 的 parseMessage 行為）。結論來自 mechanism-error-feedback 審查時讀程式，尚未拿實卡確認。

## Next action
等出現實卡再開工〔模型判斷·未裁決〕。2026-10-27 掃過 TestCards 全部 55 項（含 zip、PNG chara/ccv3 區塊）：非 zod MVU 卡（DongeonMaster、HeroTrainingUnderSide_stats、clean-hero-roster 等）教的都是 `<UpdateVariable>`＋JSONPatch，沒有一張教 `_.set` 類語法。
共用核心 `src/features/mvu-engine/source-commands.ts` 已能抽取 `_.set` 類指令；缺的是桌面非 zod 路徑（Rust `mechanism/parse.rs`）沒接上。開工時優先重用核心，不另寫解析器；先重新確認 parse.rs 現況。
