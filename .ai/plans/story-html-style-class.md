# story-html-style-class — 評估與方案

案別：小（評估）。結案（2026-10-29）：受影響 0 張、照酒館做已由卡片介面 iframe 達成，不實作、列已知限制〔模型判斷·未裁決〕。

## 受影響卡（TestCards 全數清點，含 PNG 內嵌卡與 JSON）
- 帶 `<style>` 的卡：WestFantsy 系列（含 WF-*；WestFantasy_NOUI 沒有）、DongeonMaster、HeroTrainingUnderSide_stats、NorthHall、RPGImmortal、Transfur、donass、bcd368…、econ-professor-blackboard。每一處 `<style>` 都在顯示用 regex 的 `replaceString` 整頁殼裡，照 `extractShell`（`src/features/card-interface/interface-card.ts`）的規則逐處核對，全落在會切進卡片介面 iframe 的範圍內，殼外 0 處。
- 立案點名的 blackboard／Status_block／StatusData 全是上述殼（econ-professor、donass、NorthHall、RPGImmortal）：模型吐 `<Status_block>` 這類 XML，regex 換成整頁殼進 iframe，等同酒館＋酒馆助手那條路，「照酒館做」已達成。
- HeroTraining 世界書第 39、40 條命中的是 `<Style>holy</Style>` 報紙 XML 標籤，不是 CSS。
- 殼外帶 `class=` 的 HTML（開場白、世界書、描述、非殼 regex 輸出）：0 處。殼內的 `<style>` 碰不到殼外元素，殼外就算有 class 也沒有外觀可丟。
- 殼被判不支援（scrypt、雲端載入器、讀不出）時後端回空腳本，regex 不套，正文是模型原文，不會出現殼裡的 `<style>`；殼沒有大小上限。
- 遊玩紀錄（TestCards 下三份跑團紀錄）沒有 `<style>` 或 `class=`。
- 結論：正文裡會出現 `<style>`＋class 的只剩「模型自己在正文吐 `<style>`」或「卡片帶非殼的 `<style>` 片段」，TestCards 0 張。

## 已知限制
- 正文（非卡片介面）裡的 `<style>` 區塊與 class 不套用：`<style>` 內容丟、class 剝、文字照留。〔模型判斷·未裁決〕
- 三題：一般玩家會不會遇到——手上 20 多張狀態欄卡都走 iframe，遇不到；不做會怎樣——少數非殼卡外觀是素的、內容不缺；跟地基有沒有關係——無關。三題皆否，不立後續案。

## 若仍要做（備查，未送審；若做為中案，核心估 150–250 行）
- 做法：DOMPurify `uponSanitizeElement` 掛鉤在剝 `<style>` 前取文字，用瀏覽器原生 `CSSStyleSheet.replaceSync` 解析（不加 CSS 解析依賴）；只收一般樣式規則（丟 `@import`、`@font-face`、`@media` 以外的 at-rule），選擇器加訊息容器前綴（照 ST 的 `.mes_text`），class 屬性改放行並加 `custom-` 前綴（選擇器同步改）。
- 過濾：沿用 story-inline-style 的思路但放寬到排版類（margin、padding、border、border-radius、display 限 block／inline-block／flex／grid／none、flex／grid 類、width／max-width、漸層背景）；禁 `position: fixed|sticky|absolute`、`z-index`、`url()`、`image-set()`、`@import`、跳脫；字級照 clamp。
- 網頁版：宿主 CSP `style-src 'self'` 擋 `<style>` 元素，改用 `adoptedStyleSheets` 或 CSSOM `insertRule`（CSSOM 不受 style-src 管），每個訊息插入點要加 ref 與 effect 管生命週期（三個插入點）。
- 核心估：解析＋前綴約 60、宣告過濾放寬約 60、class 前綴與 hook 約 20、網頁版 CSSOM 套用與三個插入點約 50–100，合計 150–250。測試 ≤ 核心 2 倍。

## 觀察
（施工中範圍外發現寫這裡）
