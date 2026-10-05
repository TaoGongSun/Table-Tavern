# 一般存圖接上嚴格 PNG 驗證

## Summary
`import/images.rs` 存圖（AI 生圖、手動上傳）只驗 PNG magic；refactor-card-png-export 已做出共用的嚴格串流驗證（`png_image`，含 zlib 完整性、palette 索引、APNG 拒收、資源上限）。本案把一般存圖也接上。2026-10-06 自 refactor-card-png-export 驗收衍生。

## Next action
先盤點會受影響的存圖入口與目前會被新規則拒收的情況（例如 APNG、非 PNG 上傳），決定拒收時的玩家提示。
