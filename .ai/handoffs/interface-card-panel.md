# Handoff: interface-card-panel

## Current state
2026-08-04 開工。包 1（Rust 讀取端）完成並自驗綠。分三包：包 1 讀取端 ✅ ／包 2 前端渲染（覆蓋層＋regex 套用）／包 3 橋接與收邊。

## 拍板（2026-08-04 使用者）
- 面板形式＝**全螢幕覆蓋層**：點按鈕整個蓋掉聊天，關掉回對話。殼是整頁設計、頗高，覆蓋層才拿得到完整寬高不破版。
- 覆蓋層開著時**不必另外保留正文**：這類介面本來就把敘事文字畫在殼裡（不然 ST 玩家也看不到），文字全交給殼，體驗貼近 ST。
- 渲染範圍＝**只渲染最新一則**模型輸出。一個 iframe，往回捲看不到舊介面（舊訊息仍有純文字）。
- 橋＝單向「文字填入輸入框」，不代送（沿用任務檔 Constraints）。

## Completed
- **包 1 Rust 讀取端**（規格與驗收主線，實作外包 general-purpose subagent `model: sonnet`）：
  - `import.rs:148-286` 新增 `InterfaceScript`／`CardInterface` 與 `read_card_interfaces(root, world_id)`：走訪未封存角色，讀匯入時就存下的原始卡檔（`<id>.png`／`<id>.import.json`），抽出 `extensions.regex_scripts`。**不新增儲存格式**——原始卡檔本來就留著，直接當來源。
  - 顯示腳本篩選＝未停用＋非 promptOnly＋`placement` 含 2（模型輸出）。`promptOnly` 那類（如尋道卡『不发送前文状态栏』）本地權威天然等效，直接忽略。
  - 不支援偵測：`scrypt`（`first_mes`／`description` 含 SCRYPT 標記的 DRM 卡）、`remote_loader`（吞掉整段的萬用 find＋短 replace＋`.load(`，即訓帝卡那型雲端載入器）。偵測到就清空腳本、回傳原因，不解密也不繞過。
  - 壞卡防線：非匯入卡、PNG 無 chara chunk、JSON 壞掉一律跳過該角色，不回傳 Err——面板是選配，絕不能擋住呼叫端。
  - `lib.rs:456-462` 指令 `card_interfaces(world_id)`，`lib.rs:1957` 已註冊。

## Verification
- 包 1：`cd src-tauri && cargo test` → `test result: ok. 277 passed; 0 failed`（既有 272 ＋ 新增 5）。新測試 `import.rs:2027-2151`：顯示腳本篩選（disabled／promptOnly／placement:[1] 三種都濾掉）、scrypt 偵測、remote_loader 偵測、無 extensions 普通卡、壞卡跳過不報錯。
- 拆卡實據（本次以 TestCards 實卡再驗，樣本 gitignored）：西幻卡 2 支腳本（`西幻` find 為五個 capture group 的 `<GoldenRPG_UI>` 塊、replace 42KB；`开场白` find `\s*请选择你的身份\s*`、replace 23KB），兩份 replaceString 都是包在 ```` ```html ```` 圍欄裡的完整 HTML 文件 → **渲染方式＝抽出圍欄內的 HTML 塞 iframe srcdoc**，與酒馆助手把 html 程式碼塊轉 iframe 的作法一致。
- 西幻殼的按鈕代送寫法：`window.parent.document.getElementById('send_textarea')` 直接戳宿主輸入框（`wf-西幻.html:288-302` 的 `insertToInput`），外層包 try/catch，抓不到就 `console.error` → 沙盒下天然不會爆，只是按了沒反應。尋道卡則走 `triggerSlash('/send …')`。

## Remaining
- **包 2 前端渲染**：
  1. 新檔 `src/interface-card.ts`：`parseStRegex`（吃 `/pattern/flags` 與裸樣式兩種寫法）、`applyScripts(raw, scripts)`（`$1`–`$9`、`{{match}}` 代換、`trimStrings` 先剝）、`extractShell(rendered)`（抽 ```` ```html ```` 圍欄內容；沒圍欄就當整段 HTML）。**regex 一律在前端 JS 跑**——ST 腳本是 JS regex 語法（後向、lookahead 都用得上），Rust regex crate 吃不下。
  2. 資料來源＝最新一則 `narration` 事件的 `raw ?? text`（`raw` 是剝殼前原文，state-values-mvu 包 4 已備好）。
  3. 覆蓋層：聊天標題列一顆按鈕開／`Esc` 或關閉鈕收；`<iframe sandbox="allow-scripts" srcdoc=…>`（**不給 allow-same-origin**，卡內腳本碰不到 app 內部）。
  4. 沒腳本／`unsupported` 不為 null 的桌不出按鈕（不支援的提示歸包 3）。
- **包 3 橋接與收邊**：
  1. iframe 內注入前置墊片：假 `window.parent.document`（回傳一個誘餌 `<textarea id="send_textarea">`，卡片照原樣寫 `.value` 並 dispatch input，我們監聽誘餌的 input 事件）＋ `triggerSlash` 墊片（認 `/send`、`/trigger`）→ 一律 postMessage 給宿主 → 填入 composer 輸入框，**不自動送出**。
  2. ⚠️ 未驗證的前提：`Object.defineProperty(window, 'parent', …)` 在沙盒 iframe 內是否可覆寫。規格上 `parent` 不是 LegacyUnforgeable（`top`／`document`／`location` 才是），Chrome 可覆寫，但本 app 走 **WKWebView（macOS）／WebView2（Windows）**，必須在真 app 內實測。覆寫不成就退備案：注入前把殼原始碼裡的 `window.parent` 字面換成 `window.__ttHost`。（本次想用瀏覽器窗格先測，preview_start／navigate 兩次都 300s 逾時，改到包 3 在真 app 測。）
  3. 不支援提示（scrypt／remote_loader）、i18n 十語系字串、殼壞／regex 不合時正文照常顯示的回退。
  4. 實卡驗收：匯入西幻卡開桌，面板出現同款介面、開闔自如、點推薦行動文字進輸入框；關面板照常玩、原生狀態欄不受影響。

## Next action
從包 2 第 1 項開工：寫 `src/interface-card.ts` 的三個純函式並用西幻卡的真實 findRegex／replaceString 當測試資料（`src/story-markdown.test.ts` 是同款 vitest 測試的現成範本）。

## Constraints
- 卡內腳本一律沙盒、無任何 app API；橋只有「文字入輸入框」一條單向道。
- 不碰 DRM（不解密、不繞驗證）。
- 面板為選配，任何失敗一律回退純文字對話。
- 酒馆助手（JS-Slash-Runner）**只當規格書讀，禁抄碼**（Aladdin 授權禁商業散布，本 app 有贊助包銷售）。
