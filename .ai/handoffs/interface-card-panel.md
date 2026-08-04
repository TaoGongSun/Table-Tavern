# Handoff: interface-card-panel

## Current state
2026-08-04 開工。包 1（Rust 讀取端）、包 2（前端渲染＋覆蓋層）完成並自驗綠，含西幻真卡端對端驗證。剩包 3（匯入提示分流＋真 app 實測），**尚未在真 app 裡跑過**。

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
- **包 2 前端渲染**（規格與驗收主線，實作外包兩隻 general-purpose subagent `model: opus`，一隻純函式一隻畫面）：
  - 新檔 `src/interface-card.ts`：`parseStRegex`（`/樣式/旗標` 與裸樣式兩種寫法，丟掉 JS 沒有的旗標，壞樣式回 null）、`applyScripts`（**一律用 replacer 函式**手動掃 `{{match}}`／`$1`–`$9` 單輪代換——殼有 42KB 且內含 jQuery `$(`、CSS 與 `$&`，交給 JS 原生 `$` 語意會整份壞掉；單支腳本壞掉跳過續走）、`extractShell`（先找 ```` ```html ```` 圍欄、退而找裸 `<!DOCTYPE html>`）、`buildShellDocument`（插入橋接墊片＋把殼裡 `window.parent`／`window.top` 字面改指墊片）。
  - 墊片（`interface-card.ts:129-207`）：iframe 內放隱藏誘餌 `<textarea id="send_textarea">`，卡片照原樣寫 `.value` 並 dispatch input，我們攔下來 postMessage 給宿主；另備 `triggerSlash` 墊片（認 `/send`、`/trigger`）。`window.parent`／`top` 的 `defineProperty` 覆寫包在 try/catch，**能不能覆寫仍未在真 app 實測**，備案是字面取代。
  - 主線在驗收時補的兩處（子代理沒想到的實際使用問題）：誘餌送出後**立刻清空**（卡片寫法是「舊值有東西就接在後面」，不清會讓連點兩個推薦行動黏成一串）；選擇器比對改成關鍵字比對（真卡實測有 `#send_textarea`／`textarea#send_textarea`／`#chat-input`／`#user-input`／`#prompt-textarea` 五種寫法輪流試）。
  - `App.tsx`：`cardInterfaces` 狀態＋切桌重讀（3317-3330）、`cardInterfaceShell` useMemo（由後往前掃最近 10 則、跳過玩家發言，讓開場那種只出現一次的殼也抓得到，3333-3346）、postMessage 橋接只認 `source === "table-tavern-card"` 且**只填輸入框不代送**（3349-3363）、Esc 關閉（3364-3372）、標題列開關鈕（4772-4777，沒殼的桌不出現）、覆蓋層 JSX（5114-5132，`sandbox="allow-scripts"`、無 same-origin）。`App.css` 檔尾新增覆蓋層區塊（z-index 9，低於設定視窗的 10）。i18n 十語系各補 `cardInterfaceOpen`／`cardInterfaceClose`。
- **順手修（上一個任務留下的紅燈，與本任務無關）**：`scripts/check-i18n.mjs` 的 `WRAP_SAFE_LONG` 加 `ja:playerLabel`。`playerLabel` 是「沒有玩家卡時的代稱」不是按鈕文案，它在 state-values-mvu 包 5 被放進狀態欄的值按鈕後就一直讓 `check:i18n` 紅（HEAD 版本實測同樣紅）；值欄是 1fr 彈性欄本來就放得下整句狀態，故列入白名單而非改動日文用詞。

## Verification
- 包 1：`cd src-tauri && cargo test` → `test result: ok. 277 passed; 0 failed`（既有 272 ＋ 新增 5）。新測試 `import.rs:2027-2151`：顯示腳本篩選（disabled／promptOnly／placement:[1] 三種都濾掉）、scrypt 偵測、remote_loader 偵測、無 extensions 普通卡、壞卡跳過不報錯。
- 拆卡實據（本次以 TestCards 實卡再驗，樣本 gitignored）：西幻卡 2 支腳本（`西幻` find 為五個 capture group 的 `<GoldenRPG_UI>` 塊、replace 42KB；`开场白` find `\s*请选择你的身份\s*`、replace 23KB），兩份 replaceString 都是包在 ```` ```html ```` 圍欄裡的完整 HTML 文件 → **渲染方式＝抽出圍欄內的 HTML 塞 iframe srcdoc**，與酒馆助手把 html 程式碼塊轉 iframe 的作法一致。
- 包 2：`npm test` → `Test Files 2 passed / Tests 14 passed`；`npx tsc --noEmit` 無輸出；`npm run check:i18n` 十語系全 OK；`npm run build` → `✓ built in 600ms`。`grep allow-same-origin src/App.tsx` 無結果（只在 interface-card.ts 的註解裡出現）。
- **西幻真卡端對端（主線寫的臨時測試，跑完即刪——樣本在 gitignored 的 TestCards/，測試檔留在 scratchpad）**：拿真卡兩支顯示腳本（`西幻` 42KB 殼、`开场白` 23KB 殼）餵兩種輸入，四項全過：
  1. 真開場訊息「请选择你的身份」→ 抽出的殼含 `<!DOCTYPE html>` 與「角色选择」＝開場畫面真的長出來。**這條驗收不用花模型額度**：卡片 `first_mes` 就是那七個字。
  2. 作者自寫的輸出範本（世界書『回复规则』裡的 `<GoldenRPG_UI>` 樣板，3,143 字、五大區塊齊全）→ 殼裡出現 `$1` 的劇情文字、`PlayerAt`、`阿斯加德`、`委托名称1`（即五個 capture group 都換進去了），且 `$&` 沒殘留。
  3. `buildShellDocument` 產出的文件含誘餌 `send_textarea`／`__ttHost`／`triggerSlash`，且**不再含 `window.parent.document`**。
  4. 純文字旁白 → 回 null（沒介面）；腳本清單第一支是壞樣式時後面兩支照常運作。
- 西幻殼的按鈕代送寫法：`window.parent.document.getElementById('send_textarea')` 直接戳宿主輸入框（`wf-西幻.html:288-302` 的 `insertToInput`），外層包 try/catch，抓不到就 `console.error` → 沙盒下天然不會爆，只是按了沒反應。尋道卡則走 `triggerSlash('/send …')`。

## Remaining
**包 3 收邊與實機驗收**
1. 匯入提示分流：現有的 `importScriptNotice`（`src/i18n/zh-TW.ts:275`，「已只讀入人設文字」）**已經過時**——腳本現在真的會用來顯示。匯入後依 `card_interfaces` 的結果分四種說法：有可渲染的介面／`scrypt`（加密卡，介面不顯示、其餘照常匯入）／`remote_loader`（介面要從網路載入、不支援）／只有非顯示型腳本（沿用舊句）。掛在 `App.tsx:3776` 那段（`probe.scripts.length > 0` → `showMessage`）。十語系字串一起補。
2. **真 app 實測**（`npm run tauri dev`，macOS 走 WKWebView）：
   - `Object.defineProperty(window, 'parent', …)` 在沙盒 iframe 內能不能覆寫。規格上 `parent` 不是 LegacyUnforgeable（`top`／`document`／`location` 才是），Chrome 可覆寫但 WKWebView 未知。**覆寫不成也不致命**——`buildShellDocument` 已經先把殼裡的 `window.parent` 字面換成 `window.__ttHost`，覆寫只是替動態組字串的卡片補漏。（本次想先用瀏覽器窗格測，`preview_start`／`navigate` 兩次都 300s 逾時，只好留到真 app。）
   - 匯入西幻卡開桌：開場那則就該能開出角色選擇畫面（不必等模型回應、不花額度）。
3. 實卡驗收四項：面板出現同款介面、開闔自如、點推薦行動文字進輸入框（不自動送出）；關面板照常玩、原生狀態欄不受影響；殼壞／regex 不合時正文照常顯示；SCrypt 卡匯入有提示、其餘欄位照常。

## Next action
從包 3 第 1 項開工（匯入提示分流，純前端＋i18n），接著開 `npm run tauri dev` 做第 2、3 項實測。

## Constraints
- 卡內腳本一律沙盒、無任何 app API；橋只有「文字入輸入框」一條單向道。
- 不碰 DRM（不解密、不繞驗證）。
- 面板為選配，任何失敗一律回退純文字對話。
- 酒馆助手（JS-Slash-Runner）**只當規格書讀，禁抄碼**（Aladdin 授權禁商業散布，本 app 有贊助包銷售）。
