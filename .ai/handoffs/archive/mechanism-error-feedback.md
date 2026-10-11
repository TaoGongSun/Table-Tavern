> 結案 2026-10-12：verify 12 步綠；Haiku 接管桌 2 回合改對；三方驗收 PASS。

# 其他機制錯誤也回饋給 GM

Status: done

## Summary
GM 可修正的機制錯誤（路徑不存在、對分支增減或替換、在單一欄位底下寫子路徑、解析層認不出的物件與壞 JSON）由 Error 改記拒收，經 notes 回饋 GM 該怎麼寫；帳本現值壞掉與衍生公式算不出來維持 Error。沒重構 MVU 卡的 `[值, 說明]` 照上游收 delta，解析照上游收 `add`、move 目的地 `path ?? to`。規格、已知限制與驗證結果見 [plans/mechanism-error-feedback.md](../../plans/mechanism-error-feedback.md)。後續：[mvu-array-index-path](../../tasks/mvu-array-index-path.md)、[mvu-command-syntax](../../tasks/mvu-command-syntax.md)。
