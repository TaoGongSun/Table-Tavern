> 結案 2026-10-11：verify 12 步綠；真模型 DongeonMaster-safe 重構後 3 回合 patch 20/20；三方驗收 PASS。

# 重構接管：MVU 卡重構後 GM 對不上變數路徑

Status: done

## Summary
locked 機制條目對 GM 不可見＋重構提示詞沒附既有狀態樹，致規則路徑全對不上、GM 自創路徑。改為：帶欄位規則的機制條目照原卡觸發與位置給 GM、四種重構提示詞附既有狀態樹並規定路徑鍵逐字沿用、套用時篩掉對不上樹的新規則與觸發、無 guide 的 DeltaOnly 增量桌也附本桌範例。規格、已知限制與驗證結果見 [plans/takeover-mvu-path-mismatch.md](../../plans/takeover-mvu-path-mismatch.md)。
