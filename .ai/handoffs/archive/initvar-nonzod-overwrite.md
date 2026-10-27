> 結案 2026-10-27：squash 進 main，Sol、Grok 驗收第 2 輪 DONE，main 上重跑 verify 12 步綠。

# initvar-nonzod-overwrite — 桌面非 zod 桌開場白 `<initvar>` 不覆寫初始值

規格與做法見 [plans/initvar-nonzod-overwrite.md](../../plans/initvar-nonzod-overwrite.md)。

## 結果
- 送審第 1 輪 Sol、Grok 同意；驗收第 1 輪兩項必改（Busy 期限內重試、zod 判定讀檔失敗記帳退回）已修，`npm run verify` 12 步綠。
- 測試通道實機過（2026-10-27）：`TestCards/clean-hero-roster.json` 複本（開場白加 `<initvar>` 與 JSONPatch delta），照預設「匯入成世界書」→ 貼開場白 → 狀態面板只剩覆寫的 Player／World（Heroes、NPCs、Enemies_List 消失），`覆寫計數` 999→1000；整個 app 關掉重開後面板與後端狀態一致；全程零 AI 派送、帳本沒有開局錯誤。
- 網頁版非 zod 早已覆寫，只補了鎖定測試。
- 「額外世界書 initvar 補回」移到 initvar-entry-yaml-semantics〔作者裁決 2026-10-27〕。
