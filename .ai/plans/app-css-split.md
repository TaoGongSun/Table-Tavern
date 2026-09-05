# App.css 拆進 styles/

`src/App.css` 拆前實測為 2545 行、55,285 bytes，是 `src/` 唯一的 CSS 檔；`src/App.tsx` 只以 `import "./App.css"` 載入它。2026-09-05 立案，分支 `app-css-split`，起點 `350589b4aecaa62e1ea259b51b474f92ebb259f6`；拆分 baseline blob 為 `b2dbc0f1cd2f3d6e943074808fdd18c0d45ae601`。

> 立案時誤記為 2546 行。開工後以 `read_bytes().splitlines(keepends=True)` 對 baseline blob 實測為 **2545 行**；55,285 bytes 與 blob SHA 均一致。切線與驗收以下列實測值為準。

本案目標和前面的 Rust split 相同：**純搬家，不趁拆分改設計、修樣式或重構 selector**。但 CSS 的主要風險不是 module visibility，而是 cascade／source order；因此本案把「展開後的 CSS 規則順序與原檔完全一致」當成第一級不變量。

## 目標

把單一 `src/App.css` 拆成 `src/styles/` 下數個按現有自然區段排列的 CSS 檔，`App.css` 留作極薄 facade，只依原順序 `@import` 各檔；`App.tsx` 維持現有 `import "./App.css"`，呼叫端不用改。

拆完後應達成：

1. 每個樣式檔有清楚責任範圍，新增 UI 不再一律往 `App.css` 尾端堆；
2. selector、declaration、註解、keyframes、media query 與相對順序不變；
3. 所有既有主題、側欄、對話、Composer、設定／Worldbook／CLI／Usage、覆蓋層外觀與互動狀態不變；
4. `App.css` 只負責載入，不再承載正式樣式規則。

## 現況與已知順序敏感點

目前檔案本身已經有自然區段標記，且多處註解明文依賴「後寫覆蓋前寫」：

- `.tcard-gm, .tcard-player` 必須位於 `.tcard` 之後，同 specificity 靠 source order 蓋掉基底底色；
- `.opening-choice-head .opening-trans-status` 必須在一般 `.opening-choice-head span` 之後，解除兩行截斷；
- `.state-bar-value` 明確覆寫全域 `button` 的高度、邊框、nowrap、置中等規則；
- `.ai-gallery-pick` 明確洗掉全域 `button` 外觀；
- `.card-interface-overlay` 的 z-index 9 與 `.modal-overlay` 的 z-index 10 是刻意層級。

因此不能把「同功能 selector 蒐集到一起」當成第一步；本案優先採**連續行段搬家**，保住原始 source order，再決定檔名。

## 最終切線（2545 行 → 7 檔）

以下行號以 baseline blob `b2dbc0f...` 為準。

| 檔案 | 原 `App.css` 行號 | 內容 |
|---|---:|---|
| `styles/base.css` | 1–338 | Emblem 設計系統說明、body、`:root` tokens、7 套主題、focus、通用 container/row、全域 input/select/button/textarea、`.app-shell` |
| `styles/sidebar.css` | 339–828 | 側欄、桌列表、角色卡／玩家卡／GM 卡、avatar、tooltip、emoji/crop、archive、角色建立、sidebar footer |
| `styles/dialogs.css` | 829–1356 | modal shell、開場白翻譯／選擇、AI 開桌／生圖、act flyout、編輯 pane、角色卡媒體、lightbox、單幕閱讀 |
| `styles/workspace.css` | 1357–1751 | 主欄 header、state bar/tree、桌名、messages、StoryText render、playbill、typing animation／完整 reduced-motion `@media` |
| `styles/composer.css` | 1752–1858 | Composer 區段標頭、發言目標、writebox、送出／AI actions、undo／restore |
| `styles/settings.css` | 1859–2474 | 設定表單、主題 swatch、作者頁、Worldbook、mechanism ledger、refactor UI、onboarding、CLI 安裝／權限、Usage |
| `styles/card-interface.css` | 2475–2545 | card interface overlay／toolbar／status／frame，以及目前位於檔尾的 locked worldbook、refactor fail reason、field warn |

### 開工時修正的切線

立案草案把 `workspace.css` 寫成 1357–1753、`composer.css` 從 1754 起。Vite/PostCSS 實際解析時抓到 `workspace.css` 尾端 `Unclosed block`：1752 是 Composer 區段註解、1753 是 `.composer {`，草案把 selector 開頭留在前一檔、properties 放到後一檔。最終因此改成 **workspace 1357–1751、composer 1752–1858**；完整 `@media (prefers-reduced-motion: reduce)` 留在 workspace，Composer 從自己的區段標頭開始。這是切線校正，production CSS 原文與總順序沒有變。

### 為什麼先接受 `card-interface.css` 尾端含少量雜項

最後三組規則語意上不全屬 card interface，但它們現在位於 card interface 後方。若為了檔名漂亮把它們搬回 `settings.css`，會改變 source order。第一輪拆分不值得拿 cascade 風險換目錄潔癖；先保留連續切線。日後若要重新整理，另開重構案並做 selector-specific 驗證。

## App.css facade

最終只保留依原始順序排列的 import：

```css
@import "./styles/base.css";
@import "./styles/sidebar.css";
@import "./styles/dialogs.css";
@import "./styles/workspace.css";
@import "./styles/composer.css";
@import "./styles/settings.css";
@import "./styles/card-interface.css";
```

不導入 `@layer`，不改用 CSS Modules，不把 import 分散到 React component。這些做法都會讓本案從「搬家」變成架構重寫，增加 specificity／載入順序／chunk order 的驗證面。

## 白名單：本案允許的變更

1. 新增 `src/styles/*.css`；
2. 將原 `src/App.css` 的**連續原文區段**搬入上述檔案；
3. 把 `src/App.css` 改成純 `@import` facade；
4. 若 Vite／CSS parser 對 import 形式有必要要求，只允許做最小路徑／語法調整，且必須記錄原因；
5. 既有驗證工具若直接假設所有 CSS 都在 `App.css`，只允許做「跟隨 facade imports 讀取同一批 CSS」的最小測試 plumbing，不改驗證規則本身。

除此之外不做：

- 不改 selector 名稱、specificity 或 DOM class；
- 不合併重複規則、不抽 utility class、不改 CSS variables；
- 不整理 property 順序、不跑會改格式的 formatter；
- 不改色彩、尺寸、間距、字級、z-index、動畫；
- 不順手修 typo、無效 property、死 selector；
- 不改任何 `.tsx`／`.ts`／Rust production code，除非實際 build 證明載入入口非改不可；若真的需要，先把它視為計畫偏差而不是默默擴 scope。

## 開工 baseline 與施工發現

實作前重新確認 branch 上 `src/App.css` 仍是 baseline blob `b2dbc0f...`，55,285 bytes，未被立案後其他 commit 改動。機械切檔 workflow 額外驗出：

1. 行數是 2545，不是立案誤記的 2546；
2. baseline SHA-256：`06e9c7bcc194837911aead5f86bcfdf49cea430624d2072a2800f0952068d55b`；
3. 原草案 workspace/composer 切線會切開 `.composer {`，已如上校正；
4. `scripts/check-i18n.mjs` 原本只 `readFileSync(src/App.css)`，拆成 facade 後 layout contract 掃不到實際 selector而誤報五項缺失；改成讀 `App.css` 並依其 `@import` 載入 CSS。既有 regex、104 顆按鈕寬度規則、語系判定全部不變；
5. `src/App.tsx` 與其他 production TS/TSX/Rust 均未修改。

## 機械驗收結果（2026-09-05）

本案最強的驗收是**重組 byte equality**，結果全綠：

1. 七個 `styles/*.css` 按 facade import 順序直接串接，與拆分前 55,285-byte `App.css` baseline **逐 byte 相同**；
2. 七檔行數為 338 + 490 + 528 + 395 + 107 + 616 + 71 = **2545**，無遺失／新增 production CSS；
3. `App.css` 只剩預期七條 `@import`；
4. `npm run test`：11 test files、**157 passed / 0 failed**；
5. `npm run check:i18n`：全綠，de/en/es/fr/ja/ko/pt-BR/ru/zh-CN 皆通過，104 顆按鈕仍在既有寬度契約內；
6. `npm run build`（`tsc && vite build`）：全綠；修正切線後無 CSS import/parser error；
7. 最終 net diff 只有 `src/App.css`、7 個 `src/styles/*.css`、`scripts/check-i18n.mjs` 與本計畫文件；沒有暫存 workflow、沒有 `.tsx`、沒有 Rust。

施工時為了讓 GitHub connector 能機械地對原始 blob 切行，曾使用一次性 GitHub Actions workflow；workflow 在成功 commit 前自刪，**不在最終 tree**。失敗的前置驗證也都遵守「任一檢查失敗就不 commit CSS 結果」，所以沒有半成品混入正式 diff。

### 關於 byte equality

七個新檔沒有自行補區段標頭、沒有刪空行、沒有重排註解。檔案責任由檔名與本計畫描述，production CSS 本體維持原文；因此直接串接七檔就是可信的純搬家證據。

## 視覺 smoke test

機械驗收過後仍要實機看一輪，因為 CSS bundling／`@import` 處理屬瀏覽器最終行為。至少覆蓋：

1. **主題**：dark、light，再抽一套非預設主題，確認 tokens 生效；
2. **側欄**：桌列表、GM／玩家／一般角色卡、選中態、編輯鈕、archive；
3. **主欄**：header、state bar 展開／收合、一般訊息／旁白／system、StoryText 圖片與格式；
4. **Composer**：目標晶片、輸入框、主送出、AI actions、undo／restore；
5. **Modal／編輯器**：設定視窗層級、tabs、至少一個角色卡或世界設定編輯畫面；
6. **設定內容**：theme swatches、Worldbook、CLI 區、Usage 表格；
7. **覆蓋層**：card interface 開啟時位於主介面上方；再開 modal 時 modal 仍應蓋在 card interface 上方（z-index 10 > 9）；
8. **鍵盤／動態效果**：`:focus-visible` 仍可見，typing animation 正常；系統 reduced motion 時 typing animation 被關閉。

### 自動化 Chromium parity smoke（2026-09-05）

GitHub Actions 另以 Vite + headless Chromium 建立不碰 production code 的代表 DOM，並在**同一個 browser run** 同時載入：

- 拆分後的 `src/App.css` facade；
- 立案 commit `72d818e5c28becaee27db618db0fbd524a095bad` 的 monolithic `src/App.css` baseline。

Vite/PostCSS 會先把 `@import` 展平，因此 browser stylesheet 中兩者都得到 330 條 parsed rules。對下列代表 computed styles 做 current/baseline 比對，結果 `parityDiffs: []`，**零差異**：

- dark `--surface-0`；
- sidebar 寬度、app shell 高度、全域 button 高度；
- 一般角色卡與 GM 卡 layout／高度／背景；
- `.row`、header actions、Composer actions 折行；
- 長角色名 max-width／ellipsis；
- theme swatches layout；
- card interface / modal overlay position 與 z-index；
- field warning danger color。

另通過：

- dark／light／parchment 三套 theme token 與 baseline 相同；
- `:focus-visible` 實際為 solid 2px；
- reduced-motion 下 typing animation 實際為 `none`，且與 baseline 相同；
- browser console errors = 0；
- 三套主題代表畫面已截圖人工查看，未見拆檔造成的側欄、角色卡、主欄、Composer 或長名省略錯位。

### 原生 Tauri 視覺 smoke（2026-09-05）

本機以 `app-css-split` 分支重新打 release 包（`Table Tavern.app` 0.2.0 aarch64）實跑，作者親測：設定視窗（外觀／AI 連線／額度三分頁）、主題色票、側欄桌列表與 GM／玩家／角色卡、角色卡編輯頁、狀態欄展開收合、故事文字與圖片、Composer 與 undo／復原、單幕閱讀。畫面與拆分前一致，未見任何樣式失效。

## 完成定義

本案完成必須同時滿足：

- 七檔切分完成，`App.css` 成為純 facade；
- 重組 CSS 與拆前 baseline byte-identical；
- build／既有前端檢查全綠；
- 自動化 browser parity smoke 無 regression；
- 原生 Tauri 視覺 smoke 完成，或明列無法在遠端進入的項目；
- commit diff 不夾帶任何樣式調整或功能修正。

若拆分途中發現既有 CSS bug，**記錄但不在拆分 commit 修**；另案處理，沿用近期 Rust split 的「拆分與修 bug 分離」原則。

## 狀態

**全項驗收完成（機械、Chromium parity smoke、原生 Tauri 本機實測），可結案合併。** 分支：`app-css-split`。