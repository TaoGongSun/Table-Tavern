# release-3-v0.4.0 — 桌面版第二個正式版 0.4.0

分支：`release-3-v0.4.0`。小案（只改版本、文件、設定；核心 0 行）。

## 拍板〔作者裁決 2026-10-29〕
- VN 模式開工前先發一版，放到下載網址；版本號 `0.4.0`；網頁版一起更新（作者在 Cloudflare 儀表板按部署）。

## 現況
- 作者追加（2026-10-29）：網頁下載頁說明與 README 兩份同步更新到 0.4.0 現況——已在分支（下載頁對照表加「更新」列、Windows 補瀏覽器「保留」步驟，十語系；README 補多角色點名／舞台卡、zod MVU 與 HTML 顯示、CLI 版本把關、更新可回上一版、Windows 瀏覽器步驟）。網頁版部署由作者在 Cloudflare 儀表板按。
- 五處版本 0.4.0（`node scripts/check-version.mjs v0.4.0` 綠）、CHANGELOG `[0.4.0]` 段（日期 2026-10-29，繁中＋英文）已在分支。
- 發版流程與檢查清單沿用 [plans/release-2-ci-windows.md](../plans/release-2-ci-windows.md)（把 0.3.0 換成 0.4.0）。

## 已知限制〔作者裁決 2026-10-29〕
- agy 首次登入（乾淨機器 app 內登入後終端機免再登入）未驗，0.4.0 放行。
- 不升格式版本：0.4.0 玩過的桌回 0.3.0 能開能玩、對話紀錄與狀態欄數值都在；但 0.3.0 寫回時會丟掉 0.4.0 新欄位，升回 0.4.0 不恢復。實測確認：角色卡 `stage`（0.3.0 存卡即丟）；state.json 的 `statusbar_retired`、`mechanism.incremental_by_refactor`、`mechanism.opening`（0.3.0 玩一輪即丟）；lanes.json 的 `codex_usage`、`effort`。同機制推斷（未逐一實測）：`scene_card_visibility`、`pending_leave`、`mechanism.collections／mirrors／display_formats／display_prefixes`、`FieldRule.same_as`。CHANGELOG 已寫給玩家。

## 升級實測（2026-10-29，測試通道，claude haiku）
- v0.3.0 建四種桌（範例多角色、單卡＋卡片介面、MVU zod 卡介面、0.3.0 AI 重構桌）各玩 1–3 輪 → 0.4.0 同 root 開：全部能開、無唯讀或格式提示、歷史與狀態欄／MVU 數值都在、各玩一輪正常、收回／復原正常、重載後保留。
- 0.4.0 → 0.3.0 → 0.4.0 來回後四桌與一張 0.4.0 重構桌都能續玩。

## 下一步
- 主線送審 → 正式 repo 推 `test-v*` 演練、artifact 8 個檔齊 → 合併 main → 作者說發才推 `v0.4.0`；tag 若不在 10/29 推，先改 CHANGELOG 日期。
- 發後照 release-2 計畫「發之後」清單核對；0.3.0→0.4.0 是 App 內更新與回退第一次真跑，順手做 [desktop-update-detect](desktop-update-detect.md) 的端對端驗收。
