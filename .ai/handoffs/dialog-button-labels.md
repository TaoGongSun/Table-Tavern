# 對話窗按鈕跟上介面語言

Status: awaiting-verification

## Summary
系統對話窗（confirm／message）按鈕文字已接到介面語言：22 處 confirm 帶 okLabel＋cancelLabel、11 處 message 帶 okLabel，十語系核心字典補共用與專屬鍵，`updateCancel` 併入 `dialogCancel` 後刪除。做法見 [plans/dialog-button-labels.md](../plans/dialog-button-labels.md)。Grok 實作，Opus／Sol 驗收共識（譯文依 Sol 建議修了 fr／ko／ru／ja／de／es／pt-BR 共 10 條）；verify 全綠（vitest 249、cargo test 693）。

## Next action
已進 main（分支已刪）。2026-10-02 ui-redesign 實機時 ru 的未儲存離開已過（按鈕俄文、不截字）。剩 macOS 實機：切幾個語系（至少 zh-TW、en、de、ru）點開刪角色、未儲存離開、轉成世界書條目、匯入完成通知，看按鈕是介面語言、長譯文沒被截。過了就結案。
