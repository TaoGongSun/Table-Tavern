# Handoff: interface-card-panel

## 使用者實測回報與修正（2026-08-04 第一輪）
匯入西幻卡後標題列沒有出現「卡片介面」鈕。查出兩層原因，都已修：
1. **表面**：面板渲染的是「最新一則模型輸出」，空桌沒東西可畫。→ `cardInterfaceShell` 補空桌退路，改用卡片自帶的開場白當來源（`CardInterface.opening`，Rust 端一併回傳）。
2. **根因（使用者指出）**：西幻卡被判成角色卡，而**匯成角色卡會把整包世界書條目丟掉**（原本只抽數值規則），卡片自訂的輸出格式規定跟著消失，模型永遠不會吐 `<GoldenRPG_UI>`，介面自然畫不出來。判定條件本身錯了——舊條件是「條目≥3 且人設三欄合計<200 字」，西幻卡人設 988 字所以漏判。
   - 判定改看**比重**：`條目 ≥ 3 且 世界書字數 ≥ 人設字數×3`（人設含 mes_example）。真卡實測：西幻 21,678 vs 988（22 倍）、尋道與訓帝人設為 0，三張都判對。
   - 使用者拍板：判成世界書卡就**只給確認／取消**，不再提供「照樣匯成角色卡」（那條路等於保證玩不動）。
   - 世界書路徑會 `save_world_card` 留下原始卡檔（只留真的帶顯示腳本的卡），介面腳本與開場白照樣讀得到——否則這類卡走世界書路徑後介面會整個消失。
3. **順帶修**（使用者同意）：匯成角色卡時，卡片隨身的世界書條目也帶進這桌世界書（同名由既有去重擋下）。判定失手也不會再有人玩不動。
5. **介面裡按下去直接送出**（使用者拍板）：原本填完字就關掉面板讓玩家自己按送出，等於一回合把玩家踢出介面兩次，比 ST 差（ST 的介面嵌在對話裡，全程不必離開）。改成玩家在介面裡點的那一下直接送出、畫面留在介面裡，右上角顯示「GM 正在打字」，模型回覆到了 iframe 就地重畫。
   - **卡片腳本自己送**：ST 上卡片自己觸發一回合是正常用法，照送。但我們每收到新訊息就重載 iframe，「載入就送」會滾成無限迴圈燒額度 → 煞車：玩家沒動作的期間只准自動送一次（`autoSentIdle` ref），玩家一動（點介面、按送出、換桌）就歸零；被擋下的改成填輸入框、關回對話。
   - 判斷依據是墊片在 iframe 內以捕獲階段記錄的 `event.isTrusted` 時間戳（1.5 秒內算玩家動作）。
4. **匯入完自動打開介面一次**（使用者拍板）：這類卡貼出來的開場在聊天裡只是孤零零一句「请选择你的身份」，玩家不點按鈕不會知道有整頁畫面。兩條匯入路徑都做——世界書路徑在問完開場白之後開，角色卡路徑在匯入提示之後開。「該畫哪個殼」的判斷收成 `interface-card.ts` 的 `findShell(cards, texts)`，面板 memo 與匯入流程共用同一套，不會出現「按鈕說有、打開卻空的」。

## Current state
2026-08-04 v1 三包＋使用者第一輪實測的修正全部完成、自驗全綠（cargo test 282／vitest 14／tsc／check:i18n／build），真瀏覽器互動實測與真卡整條世界書路徑都驗過。**剩使用者重新匯入實卡驗收**（先前那桌的卡是舊路徑匯進去的角色卡，要在新桌重匯一次才會走到新路徑）。

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
- **包 3 收邊**（App.tsx 主線直寫，十語系文案外包 general-purpose subagent `model: sonnet`）：
  - 匯入提示依這張卡實際畫不畫得出介面分流（`App.tsx:3844-3861`）：匯完角色卡後重讀一次 `card_interfaces`，依結果挑 `importCardInterface`（有介面，告訴玩家在哪開）／`importCardScrypt`（加密卡）／`importCardRemoteLoader`（介面在作者網站）／`importScriptNotice`（只有非顯示型腳本，沿用舊句）／不提示。順手把新讀到的清單餵給 `cardInterfaces`，匯完當下入口就會出現，不必切桌。
  - 十語系各補三個鍵。文案一律講「玩家會遇到什麼」，不出現 regex／iframe／腳本這類字眼。
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
- 包 3：`npx tsc --noEmit` 無輸出、`npm run check:i18n` 十語系全 OK、`npm run build` → `✓ built in 564ms`。
- **真瀏覽器互動實測（Chromium／vite dev，2026-08-04）**：把 `buildShellDocument` 的真實產物放進 `public/__tt-test/`（驗完刪除），用一個 `sandbox="allow-scripts"` 的 iframe 載入並監聽 postMessage：
  1. 一般回合殼：五個分頁（當前視角／世界地圖／當前區域／公會終端／個人資訊）、劇情欄、環境探測、物品技能、推薦行動全部畫出來，版面完整。
  2. 點「推薦行動1」→ 宿主收到 `{"source":"table-tavern-card","kind":"input","text":"推荐行动1"}`。
  3. 再點「推薦行動2」→ 收到的是 `"推荐行动2"`（**證明誘餌清空那個修正有效**，沒有黏成「推荐行动1 推荐行动2」）。
  4. 開場殼：角色選擇畫面（命运之卷）完整渲染，點某個角色會把該角色資料填進下方自訂表單，按「踏入命运」→ 宿主收到整句「种族：月精灵族，职业：咒术法师，身份：…」。這條走的是另一套「輪流試七種輸入框選擇器＋setNativeValue」的程式碼，與推薦行動那條不同路，兩條都通。
  5. console 無任何錯誤。
- 西幻殼的按鈕代送寫法：`window.parent.document.getElementById('send_textarea')` 直接戳宿主輸入框（`wf-西幻.html:288-302` 的 `insertToInput`），外層包 try/catch，抓不到就 `console.error` → 沙盒下天然不會爆，只是按了沒反應。尋道卡則走 `triggerSlash('/send …')`。

## Remaining
**只剩使用者實機驗收**（真 app＝Tauri／macOS WKWebView；瀏覽器那關已過，剩下的是 WebKit 與真實匯入流程）：
1. **開一桌新的**，匯入西幻卡（`TestCards/WestFantsy.png`）→ 應跳出「這張卡是世界書」只給確認／取消；確認後 38 條條目進世界書，接著問要不要貼開場白；標題列出現「卡片介面」鈕。
2. 開場那則就能開出角色選擇畫面（**不必等模型回應、不花額度**——卡片 `first_mes` 就是「请选择你的身份」七個字）；選角色、按「踏入命运」→ 文字進輸入框且**沒有自動送出**；Esc／✕ 關得掉。
3. 關掉面板照常玩，原生狀態欄不受影響；沒帶介面的桌完全看不到那顆鈕。
4. SCrypt 卡（`TestCards/SCrypted_WestFantasy.png`）匯入 → 提示介面被加密保護、其餘照常匯入。
5. 訓帝卡（`TestCards/TrainEmperor.png`）匯入 → 提示介面要連作者網站、本 app 不支援。

技術上唯一沒實測到的點：`Object.defineProperty(window, 'parent', …)` 在 WKWebView 沙盒 iframe 內能不能覆寫。**不致命**——`buildShellDocument` 已先把殼裡的 `window.parent` 字面改指墊片，覆寫只是替「動態組字串取用 parent」的卡片補漏，這兩張真卡都不靠它。

## Next action
等使用者實機驗收上面五項。驗收後若要續做 v2，候選：多張卡各自的介面切換（目前是全桌腳本攤平、抓第一個對得上的殼）、面板內圖片／外部資源的離線退路、代送開關（每桌）。

## Constraints
- 卡內腳本一律沙盒、無任何 app API；橋只有「文字入輸入框」一條單向道。
- 不碰 DRM（不解密、不繞驗證）。
- 面板為選配，任何失敗一律回退純文字對話。
- 酒馆助手（JS-Slash-Runner）**只當規格書讀，禁抄碼**（Aladdin 授權禁商業散布，本 app 有贊助包銷售）。
