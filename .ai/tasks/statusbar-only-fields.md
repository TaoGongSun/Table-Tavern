# statusbar-only-fields — 只為淘汰狀態欄而存在的欄位被收成 RULES

## Summary

opus-review 審 refactor-worldbook-statusbar-wording 時發現（2026-10-28）：characters 模式下，survey.rs:68 規定 statusbar 段的欄位也要列進 FIELDS。灰岩要塞的「剧情阶段/要塞事件」只為原卡狀態欄而存在，卻被收成 RULES，GM 每回合都要回報一段長文字（hroot2 transcript 看得到），違反「重構目的＝省模型輸出」。

## Next action

未排程。先盤點測試卡裡這類欄位有多少、各被收成什麼，再決定盤點規則怎麼改。
