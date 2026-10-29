# refactor-outcome-fields — 重構產物帶盤點 FIELDS 與不收格子名單

Status: 已結案（2026-10-29 合併 main）

characters 玩法、卡片狀態欄已淘汰的非 MVU 桌：盤點 FIELDS 與 UNTRACKED 帶進產物（`Vec<String>`，舊產物不另分辨〔模型判斷·未裁決〕）；套用時移除地圖或導航、記憶或摘要類格子〔作者裁決 2026-10-28〕連同其規則、前綴、鏡像（沒有值也照路徑清），樹上沒有的 FIELDS 欄位放空值葉子由開場補齊補每回合規則；收據記被刪規則供 undo 插回。做法、實測、觀察與已知限制見 [plans/refactor-outcome-fields.md](../../plans/refactor-outcome-fields.md)。
