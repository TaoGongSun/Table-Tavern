> 結案 2026-10-07：已進 main，Sol、Grok 驗收通過。

# 一般存圖接上嚴格 PNG 驗證

## Summary
`import/images.rs` 與 AI 生圖存圖原本只驗 PNG magic；本案讓所有存圖入口都接上 refactor-card-png-export 的共用嚴格驗證。

## 結論
- 存圖上限統一為 8192 邊、24M 像素。
- AI 生圖走嚴格版：壞檔拒收；APNG 留預設靜態圖；過大自動縮。
- 角色卡、GM 圖匯入走救圖版：先截到 IEND 保留原位元組，不行才重編並搬回卡資料；救不回就卡照匯、圖不存並提示。
- 手動上傳：後端嚴驗當防線，編輯器存檔前先驗圖，用編輯輪次防晚回覆蓋。

定案、做法與實測見 [plans/image-save-strict-validate.md](../../plans/image-save-strict-validate.md)。

## 未驗
Windows；AI 實際回 JPEG／WebP 的重編路徑只有單元測試（grok 實送回的是 PNG）。
