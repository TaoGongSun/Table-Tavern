> 結案 2026-10-12：verify 12 步綠；零 AI 重放與真模型（Sonnet 正常順序重構＋Haiku 1 回合）過；三方驗收 PASS。

# 重構接管：原卡狀態欄被當成狀態寫進樹

Status: done

## Summary
介面接管桌只認 `<UpdateVariable>`（開場白的原卡狀態區塊只記帳、GM 違約重印會回饋）；非 MVU 桌的介面重構不附既有樹、套用以產物為準；已遊玩的桌（含網頁存檔匯入桌）一律不能重構。根因、改法、測試、已知限制與驗證結果見 [plans/takeover-opening-statusbar-branch.md](../../plans/takeover-opening-statusbar-branch.md)。
