> 結案 2026-10-02：macOS release 實機 zh-TW／en／de／ru 未儲存離開、de 刪角色、zh-TW 轉成世界書條目、匯入完成通知全過。

# 對話窗按鈕跟上介面語言

Status: done

## Summary
系統對話窗（confirm／message）按鈕文字已接到介面語言：22 處 confirm 帶 okLabel＋cancelLabel、11 處 message 帶 okLabel，十語系核心字典補共用與專屬鍵，`updateCancel` 併入 `dialogCancel` 後刪除。做法見 [plans/dialog-button-labels.md](../../plans/dialog-button-labels.md)。Grok 實作，Opus／Sol 驗收共識（譯文依 Sol 建議修了 fr／ko／ru／ja／de／es／pt-BR 共 10 條）；verify 全綠（vitest 249、cargo test 693）。
