# refactor-outcome-fields — 重構產物帶盤點 FIELDS

Status: 已結案（2026-10-29 合併 main）

characters 玩法、卡片狀態欄已淘汰的非 MVU 桌：盤點 FIELDS 帶進產物（`Vec<String>`，舊產物不另分辨〔模型判斷·未裁決〕）；樹上沒有的 FIELDS 欄位放空值葉子由開場補齊補每回合規則；收據記被刪規則供 undo 插回。地圖導航、記憶摘要原本列不收名單移除，2026-10-29 作者撤銷（當時選項說明有誤導），兩類照一般格子收〔作者裁決 2026-10-29〕，UNTRACKED 整套拿掉（revert-untracked-fields）。做法、實測、觀察與已知限制見 [plans/refactor-outcome-fields.md](../../plans/refactor-outcome-fields.md)。
