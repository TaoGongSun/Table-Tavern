> 結案 2026-10-12：進 main，verify 12 步綠、測試通道實機過、三方驗收 PASS。

# 重構初判失敗不留痕

Status: done

## Summary
重構初判失敗不再一律落到無建議的二選一：白名單內的設定／前置障礙（缺 CLI 風險告知、找不到 CLI、缺 OpenRouter key、今日免費額度用完等）直接在狀態列報原因、不開二選一；送出後失敗或解析不出照 08-14 拍板開二選一並在框內附原因〔作者裁決 2026-10-12〕。claude lane 開線失敗補掛 `AI_CALL_FAILED`；讀卡片介面、查桌況失敗不再被吞。規格與已知限制見 [plans/refactor-probe-failure-hidden.md](../../plans/refactor-probe-failure-hidden.md)。

## 驗收
- 測試通道（WestFantsy、transport=claude、缺 `cli_risk_accepted`）按重構：狀態列顯示風險告知句、不出二選一、零 AI 派送。
