# 生圖來源與模型選單整理

## Summary
2026-10-06 討論立案，三項都要做〔作者裁決 2026-10-06〕：

1. **不會生圖的 CLI 不列出**：AI 生圖的來源下拉現在列出 claude，選了才灰掉按鈕再跳說明（`CardImageDialogs.tsx` 的 `NO_IMAGE_CLIS`）。改成不列；舊存檔指到 claude 時照現行退回 API。
2. **生圖模型改下拉**：設定頁「生圖模型」目前手打 OpenRouter 模型 id，容易打錯。改成從 OpenRouter 官方模型清單（`GET /api/v1/models?output_modalities=image`）篩出能出圖的模型供選，預設仍為 `DEFAULT_IMAGE_MODEL`。
3. **免費 key 實測**：只測一件事——沒儲值的 OpenRouter 免費 key 打 `/images` 能不能出圖。不能的話，補一個「免費 key 不能生圖」的專屬錯誤題型（UiMsg＋各語系），不要落到通用 HTTP 錯誤〔作者裁決 2026-10-06〕。

## Next action
先跑第 3 項實測（測試通道、免費 key），結果決定要不要加錯誤題型；再做 1、2。
