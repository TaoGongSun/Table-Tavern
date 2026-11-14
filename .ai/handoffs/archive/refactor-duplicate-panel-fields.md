# refactor-duplicate-panel-fields — 重構桌右側欄的鏡像卡片欄重複

已合 main（2026-11-14）、截圖主線親看、只有幾行不送驗收；結案。小案，核心估 ≤10 行（實作 5 行）。

## 問題與根因
重構盤點已把卡片欄（例：当前环境、驻留/交涉角色）標成內建「地點／在場人物」的鏡像（`mechanism.mirrors`，card-field-overlaps-fixed-keys），值由 App 從內建欄抄過去、GM 只報內建欄；但 `StateBar` 對鏡像欄只改成唯讀，兩組照樣並列。

## 做法
`src/features/table-state/StateBar.tsx` 的 `stateLeafRow`：鏡像欄在固定鍵有值、或兩邊都空時不畫；固定鍵還沒值而卡欄位留有初值時才以唯讀列顯示。樹上資料、觸發條件、`{{state:}}`、卡片介面變數與 GM 提示詞都不動。

## 現況
- 實作與單元測試完成（`StateBar.test.tsx` 鏡像欄兩項），`npm run verify` 17 步綠。
- 測試通道用代測 C 桌副本實測：卡片兩欄不再出現；手改地點後樹上鏡像值跟著變；固定鍵缺席、卡欄位有初值時以唯讀列出現。
- 下一步：主線驗收、送審，結案 squash 進 main。

## 已知限制
- Haiku 重構可能漏標鏡像（例：代測 A 桌 `駐留/交涉角色` 沒標），那張桌仍會並列兩組；屬低階模型盤點品質，不修。
