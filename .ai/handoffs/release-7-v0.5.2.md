# release-7-v0.5.2 — 桌面版 0.5.2（多人房間回饋五案＋簡中連線分流）

Status: 準備中（2026-11-07；版號與 CHANGELOG 已備、等作者說發）
Created: 2026-11-07
流程沿用 [plans/release-2-ci-windows.md](../plans/release-2-ci-windows.md)。小案（核心 0 行）。

## 這一版包什麼（v0.5.1 之後進 main 的）
room-scene-advance-bar、room-scene-capacity、room-open-from-idea、room-roster-avatar、provider-lang-split（桌面版）；party-phone-ux、room-speak-request、room-status-strip、room-round-timer、room-table-chat（手機端與伺服器，伺服器已於 2026-11-06 部署到 room-table-chat 為止）。

## 順序〔BACKLOG 部署順序，主線裁定 2026-11-03〕
1. **房間伺服器先部署**：`party/` 下 `npm run build` → `npx wrangler deploy --config server/workers/wrangler.toml`（wrangler 已登入作者帳號）。這次要上的：伺服器 `capacityHint`（room-scene-capacity）＋手機端靜態檔（手機端跟 Worker 一起部署：狀態列、換幕鈕、幕容量、頭像字母圈）。部署後驗：開房、手機加入、座位列有字母圈。
2. **桌面版打包**：作者親口說發 → 推 `v0.5.2` tag → CI 建 8 產物、標 Latest。
3. 發之後：release 頁 8 檔齊、`releases/latest/download/latest.json` 是 0.5.2；App 內「檢查更新」從 0.5.1 升上來；兩個帶版號安裝檔放 `~/Downloads/TableTavern-v0.5.2/` 交作者傳 pCloud。

## 發之前
- [x] 版本 0.5.2 五處（`node scripts/check-version.mjs v0.5.2` 綠）。
- [x] CHANGELOG `[0.5.2]` 段（繁中＋英文；資料段加矽基流動一條）。
- [x] 說明文字對照現況（README、房間說明書三語、下載頁文案）——見本檔「說明文字」。
- [x] `npm run verify` 綠（版號 commit 後，5c75edf9 已推 main）。
- [ ] 作者說發 → 推 tag。

## 說明文字（2026-11-07 對照，Opus 代理逐檔核、主線裁定）
- 改了：房間說明書三語——「回合列顯示還在等」改成狀態列小牌子（房主 §3、朋友 §5）、「跳過」改「當觀望」、「請某某發言」改「讓 NPC 某某說一句」、「行動倒數」改「每回合預設倒數」；網頁簡中額度頁 `quotaBody` 不再說 OpenRouter；桌面簡中入門卡 `errSave` 去掉「已取得授权」。
- 不改：README 兩語（讀者是英文與繁中，走 OpenRouter 正確）；下載頁特色卡與功能比較（簡中早已是硅基流动）；桌面 i18n 的換幕字串都在單人桌。
- 已知：簡中生圖來源選單仍列「OpenRouter API」（硅基流动能不能生圖未定，不在本版）；說明書沒提手機名單頁可調劇情字級；截圖不重拍〔作者 2026-11-07〕。
