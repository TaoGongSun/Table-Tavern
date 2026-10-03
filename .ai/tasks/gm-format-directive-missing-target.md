# Task
Task-ID: gm-format-directive-missing-target
Title: GM 導演指示指向不存在的格式條目
Status: todo
Created: 2026-10-03
Updated: 2026-10-03

## Summary
card-mvu-shim 實機驗收 2026-10-03 發現。

現象：GM 回合尾段導演指示寫「卡片已經規定了回覆的輸出格式…不要輸出規定格式以外的任何說明或狀態欄」「不要輸出格式以外的任何內容」，但 bcd368 的格式條目被收編停用、提示裡找不到它規定的標籤；DongeonMaster 沒有 app 認得的格式條目。`<UpdateVariable>` 規定（狀態更新協定 v1）仍在、沒被壓掉。

免費模型實跑：DongeonMaster 有一回合回「未附上介面規格，請補上」並帶空 patch；bcd368 回合 1 抄範例路徑 /World/Location（不存在）。

影響：模型講空話、狀態更新品質差。

## Next action
- 查導演指示的產生處，確認加入條件。
- 方向：導演指示只在格式條目實際存在於提示時才加，或改寫成不指向缺席格式。
- 不下結論；重現用測試通道、TestCards/bcd368…png 與 TestCards/DongeonMaster.png。
