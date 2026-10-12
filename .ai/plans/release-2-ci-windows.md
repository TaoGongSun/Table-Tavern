# release-2-ci-windows — 首個桌面正式版 0.3.0：發版流程、檢查清單

交接見 [handoffs/release-2-ci-windows.md](../handoffs/release-2-ci-windows.md)。

## 拍板〔作者裁決 2026-10-12〕
- 版本號 `0.3.0`。
- 照現行流程直接公開，不停在草稿。
- 發布說明放進 CHANGELOG `[0.3.0]`：上半繁中、下半英文（內容即 [CHANGELOG.md](../../CHANGELOG.md) 該段）。
- 首發不等端對端驗收，直接發 0.3.0；App 內更新的端對端驗收延到第二版發出時自然驗（下方「已知限制」）〔作者裁決 2026-10-12〕。
- 玩家要能直接點下載：正式 release 額外附兩個固定檔名副本 `TableTavern-Windows-Setup.exe`、`TableTavern-macOS.dmg`，`releases/latest/download/<固定檔名>` 永遠指最新版，README 用這兩個網址。

## 發布實際怎麼走（`release.yml`＋`scripts/release/`）

- 觸發：推 `v*` tag＝正式；推 `test-v*` tag 或手動觸發＝演練（打包、組 `latest.json`、驗簽，只上傳 artifact，不建 release）。
- `check`：三處版本一致；`v*` 還要 tag＝`v<版本>`，且 tag 指的 commit 在 `origin/main` 上。
- 打包：Windows 只出 NSIS；Mac 只出 Apple Silicon 的 `.app.tar.gz` 更新包與 `.dmg`。updater 簽章用 secrets `TAURI_SIGNING_PRIVATE_KEY`／`_PASSWORD`（已設定）。不做 OS 程式碼簽章。
- `finalize`：改無空格檔名 → 複製兩個固定檔名副本 → minisign 驗簽 EXE 與 `.app.tar.gz`（DMG 原檔與兩個副本都不驗，`plan.mjs` 的 `requireSig` 只列這兩個）→ 組 `latest.json` → 正式模式先建草稿、傳齊、再直接轉公開（同名已公開就失敗；同名草稿先刪再重建）。有預發布段的版本標預發布且不當 latest。
- 產物 8 個：`TableTavern_0.3.0_x64-setup.exe`＋`.sig`、`TableTavern_0.3.0_aarch64.app.tar.gz`＋`.sig`、`TableTavern_0.3.0_aarch64.dmg`、`TableTavern-Windows-Setup.exe`、`TableTavern-macOS.dmg`、`latest.json`。演練模式的 artifact 同樣含兩個副本，演練就驗得到這段。
- 固定檔名副本只給人點：不進 `latest.json`、不另附 `.sig`。Windows 副本內容與已驗簽的 EXE 相同；DMG 本來就沒有 minisign 驗簽，副本也沒有。網頁版下載鈕挑結尾 `_x64-setup.exe`／`_aarch64.dmg` 的檔，不會挑到副本。
- release 內文與 `latest.json` 的 `notes` 都是 CHANGELOG `## [0.3.0]` 段原文；正式模式找不到該段就中止。
- App 查更新打 `releases/latest/download/latest.json`（不含草稿與預發布）；回退點固定從 `releases/download/v<版本>/` 抓，只認 `v*` tag。所以 `test-v*` 演練不能當兩個 release 用。

## 發版檢查清單

發之前：
- [x] 版本 0.3.0 五處（`package.json`、`package-lock.json`、`Cargo.toml`、`Cargo.lock`、`tauri.conf.json`；`node scripts/check-version.mjs v0.3.0` 綠）、CHANGELOG `[0.3.0]` 段。
- [x] 三方驗收 PASS、已合併 main，`npm run verify` 綠。
- [ ] 首發必含兩案已在 main〔作者裁決 2026-10-04〕：interface-card-panel（已結案）、interface-takeover-spike（程式已進 main，交接檔仍掛進行中）。
- [ ] 正式 repo 同一 commit 推一次 `test-v*` 演練全綠；artifact 裡 8 個檔齊。
- [x] CHANGELOG `[0.3.0]` 日期 2026-10-12（當天發）。
- [ ] 作者親口說發 → 推 `v0.3.0` tag。

發之後：
- [ ] release 頁 8 個檔齊、標為 Latest；`releases/latest/download/latest.json`、`…/TableTavern-Windows-Setup.exe`、`…/TableTavern-macOS.dmg` 都下載得到。
- [ ] 網頁版下載頁兩顆鈕指向 `TableTavern_0.3.0_*` 檔。
- [ ] 兩平台從 release 頁用瀏覽器下載裝一次，照 CHANGELOG 繞過步驟能裝能開（Mac 是「無法驗證」不是「已損毀」）；App 內「檢查更新」回「已是最新」。
- [ ] 刪掉演練 tag。

## 已知限制

- 首版本身沒有回退點；App 內更新與回退第一次真跑在 0.3.0→下一版，端對端驗收（[desktop-update-detect 計畫「驗收」](desktop-update-detect.md#驗收)）改在第二版發出時驗〔作者裁決 2026-10-12〕。
- 跨格式更新：目前 `CURRENT_FORMAT = 1`，還沒有任何轉換步驟，首發後的更新短期內不會走格式轉換；轉換鏈與提交機制由單元測試覆蓋（`data/format/tests.rs` 用假轉換驗）。第一次真的升格式的版本，發版前要補跑跨格式端對端（轉換、備份、回退後唯讀、改用備份續玩）。〔模型判斷·未裁決〕
