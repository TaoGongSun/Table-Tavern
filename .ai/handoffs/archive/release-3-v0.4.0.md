# release-3-v0.4.0 — 桌面版第二個正式版 0.4.0

> 結案 2026-10-29：v0.4.0 已公開（https://github.com/TaoGongSun/Table-Tavern/releases/tag/v0.4.0 ，標 Latest、8 個產物齊，`releases/latest/download/` 的 `latest.json`（0.4.0）與兩個固定檔名都 200）；網頁版 `table-tavern.pages.dev` 已部署 main 562d741（作者按 Retry deployment）。分支已壓成一筆併 main 並刪除，演練 tag `test-v0.4.0` 已刪。小案（核心 0 行）；Sol 驗收 1 輪（必改 1 條：CLI 安裝說法過度保證，已修）。

## 拍板〔作者裁決 2026-10-29〕
- VN 模式開工前先發一版，放到下載網址；版本號 `0.4.0`；網頁版一起更新；網頁下載頁說明與 README 兩份一起更新。
- 發版流程沿用 [plans/release-2-ci-windows.md](../../plans/release-2-ci-windows.md)。

## 已知限制〔作者裁決 2026-10-29〕
- agy 首次登入（乾淨機器 app 內登入後終端機免再登入）未驗，0.4.0 放行。
- 不升格式版本：0.4.0 玩過的桌回 0.3.0 能開能玩、對話紀錄與狀態欄數值都在；但 0.3.0 寫回時會丟掉 0.4.0 新欄位，升回 0.4.0 不恢復。實測確認：角色卡 `stage`（0.3.0 存卡即丟）；state.json 的 `statusbar_retired`、`mechanism.incremental_by_refactor`、`mechanism.opening`（0.3.0 玩一輪即丟）；lanes.json 的 `codex_usage`、`effort`。同機制推斷（未逐一實測）：`scene_card_visibility`、`pending_leave`、`mechanism.collections／mirrors／display_formats／display_prefixes`、`FieldRule.same_as`。CHANGELOG 已寫給玩家。

## 升級實測（2026-10-29，測試通道，claude haiku）
- v0.3.0 建四種桌（範例多角色、單卡＋卡片介面、MVU zod 卡介面、0.3.0 AI 重構桌）各玩 1–3 輪 → 0.4.0 同 root 開：全部能開、無唯讀或格式提示、歷史與狀態欄／MVU 數值都在、各玩一輪正常、收回／復原正常、重載後保留。
- 0.4.0 → 0.3.0 → 0.4.0 來回後四桌與一張 0.4.0 重構桌都能續玩。

## 剩下
- 0.3.0→0.4.0 是 App 內更新與回退第一次真跑：[desktop-update-detect](../desktop-update-detect.md) 的端對端驗收由作者在裝著 0.3.0 的機器上做。
