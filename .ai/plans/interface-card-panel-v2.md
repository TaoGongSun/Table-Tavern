# interface-card-panel v2：多卡分頁、破圖收斂、介面內輸入列

狀態：結案（2026-10-11 進 main，三方驗收 PASS）；剩真視窗實測排實測佇列梯 1 第 4f。交接檔：[interface-card-panel](../handoffs/archive/interface-card-panel.md)。首發必含〔作者裁決 2026-10-04〕。三項取捨〔作者裁決 2026-10-11〕，細節經三方審查定案。

## 一、多張卡各自的介面

結論：腳本按卡隔離；覆蓋層工具列用卡名分頁鈕切換。預設跟著最新；玩家點某張卡的分頁就固定在那張，直到他自己再切、換桌，或那張卡畫不出殼（自動退回跟著最新，不關介面）〔作者裁決 2026-10-11〕。理由：同桌多卡時一張卡的腳本不該吃掉或改寫另一張的介面（ST 的 regex 也是卡片範圍）。

玩家情境：兩張卡用同一標籤（DongeonMaster／bcd368 都是 `<StatusPlaceHolderImpl/>`，NorthHall／RPGImmortal 都是 `<Status_block>`）時第二張永遠畫不出來；donass 的「她→他」這類替換會改寫別張卡的文字；西幻＋自帶狀態欄的角色卡同桌時，角色一開口介面就換掉。單張卡（主流）不受影響。

### 選殼規則（`interface-card.ts` `findShell`、`card-shell-route.ts` `pickCardShell`）
1. 候選帶來源，每段只試該試的卡、一張一張試，卡內多支腳本照順序接力、跨卡不接力：
   - 角色台詞樓（`dialogue`）只給說話者自己的卡；說話者沒介面才比照 GM 樓。
   - GM 旁白、system 樓：世界書卡先，其餘照桌上順序。
   - 空桌開場白與匯入後自動開介面（`openIfDrawable`）：只給所屬那張卡，候選順序也是世界書卡先。
   - 已貼出的開場白（`opening` 事件）：原文或正文去頭尾空白後等於某張卡的開場白、且那張卡畫得出東西，就只給那張卡；對不上或那張卡沒介面照 GM 樓規則。
2. 指定卡（`card` 參數）：同一套候選規則，只留那張卡輪得到的；掃整幕（跟著最新只掃近 10 樓）。最新 GM 樓兩種模式都先試、不受樓數限制。
3. 有重構骨架的桌：「原文畫得出殼就用原文、否則填骨架」的判斷也照上面的規則、指定卡時只看那張卡。骨架合成樓只歸骨架來源卡＝最新一個合成樓照 GM 樓順序第一張畫得出它的卡（骨架照搬的就是那張卡的輸出格式；重構沒記來源卡，用這個推）。同標籤的別張卡被指定時畫不出骨架；指定來源卡時別張卡畫得出的原文樓改填骨架。殼、本樓文字、讀訊息每樓用同一份文字；前情提要佔位防護照舊。
4. 分頁清單（`drawableCards`）：畫得出東西的卡不到兩張就不算；直接拿指定卡選殼逐張跑，掃整幕，「某卡套某段文字」的結果按卡清單快取（controller 持有，卡清單一換就換新；鍵與殼合計約 2 千萬字元就整份清掉，單筆超過上限的不存；帶前情提要佔位的候選每次不同、不進快取），分頁不會因最近換人說話忽隱忽現。只在面板開著時算。頭上「卡片介面」鈕與面板存廢看實際顯示的殼（固定的卡掃整幕，最近幾樓畫不出時它仍在）。
5. 殼指紋（iframe key、沙盒 token、寫入佇列）含殼所屬的卡：兩張卡產出相同 HTML 時切分頁也重掛，舊 token 與舊佇列作廢。
6. MVU character 層身分＝產生殼的那張卡（世界書卡 `world`），跨卡接力那條「找不到唯一所屬」的路整個拿掉；命中別張 MVU 卡時 `{{char}}` 換成它的名字。
7. GM 格式指示（`chat_assembly.rs` `card_format_entry`）不動。

### 分頁鈕
穩定卡 id（`character_id`，世界書卡 `world`）當 key；放在換幕與關閉鈕左邊；卡名最多約 9 字寬、超出省略、title 全名；世界書卡沒名字顯示「世界」（十語系 `cardInterfaceWorldTab`）。目前顯示的那顆 `aria-pressed`。固定的卡綁著當時的桌，離開這桌（換桌、回大廳）就回到跟著最新〔模型判斷·未裁決：回大廳再進同一桌也算離開〕。

## 二、外部資源：只做破圖收斂

結論：照 ST 線上即時抓，抓不到的圖不顯示破圖示；不做本機快取、不內建 Tailwind〔作者裁決 2026-10-11〕。理由：除本地模型外，沒網路本來就不能跑 AI。

做法：沙盒墊片注入 `img[data-tt-broken]{visibility:hidden!important}`，在 **document** 的 capture 階段聽 `error`（標上 `data-tt-broken`）與 `load`（移除自己的標記）。不掛 window：DOM 規格裡元素的 `load` 事件不傳到 window，真實 WebKit 實測掛 window 收不到恢復那一下。桌面版與網頁版共用（`buildShellDocument`）。

## 三、介面內輸入列，不做代送開關

結論：卡片按鈕照舊直接送出；覆蓋層底部固定一條輸入列，介面開著也能打自由行動（等同 ST 輸入框一直都在）〔作者裁決 2026-10-11〕。

做法（`CardInterfaceOverlay.tsx` `buildOverlayComposer`）：與聊天輸入框共用草稿（`chat.input`／`setInput`），送出走 App 包過桌面鎖的 `send`（打字送出路徑：失敗收回放回原文、換幕容量滿放回原文）。單行 `<input>`＋form submit，同聊天輸入框。停用條件同聊天輸入框（唯讀、沒對象又沒角色、`chat.busy || tableOpBusy`、換幕容量滿），另加去空白後為空時送出鈕停用；placeholder 沿用 `composerPlaceholder` 系列；錯誤訊息與換幕容量滿的提示（`sceneCapacityFull` 同一句）在輸入列上方顯示；生成中送出鈕換停止鈕。組字中的 Enter 交給原生 form（同聊天輸入框，不另外 preventDefault：WKWebView 取消 keydown 可能讓輸入法上不了字）。宿主 Esc 監聽略過組字中（`isComposing`、keyCode 229）。

## 已知限制〔模型判斷·未裁決〕
- 已貼出的開場白若跟卡上原文對不上（貼出時代換了巨集、翻譯過、玩家選的是 alternate_greetings 而非 first_mes），認不出原屬哪張卡，照 GM 樓規則試卡；多張卡開場白完全相同時歸第一張。
- 骨架來源卡是推出來的：同標籤兩張卡都畫得出骨架時歸世界書卡先、照桌上順序的第一張。
- 同桌多卡共用一份卡片 storage（設定）命名空間。
- 卡片的 `/send 文字`（沒帶 `/trigger`）在 ST 只加玩家訊息不觸發回覆，本 app 一律送出並觸發。
- 破圖收斂抓不到 Shadow DOM 與巢狀 iframe 裡的圖；CSS 背景圖、字型、腳本抓不到照 ST 一樣失效。
- 兩張 MVU 卡同桌：每樓變數表本來就一份（ST 也是）。

## 測試
單元（vitest）：`interface-card.test.ts`（按卡隔離、來源規則、快取）、`card-shell-route.test.ts`「多卡」組（同標籤台詞樓與開場、說話者的卡排後面、世界書卡排最後仍先試、雜項替換不污染、指定卡掃整幕與回 null、十則玩家樓後最新 GM 樓、相同 HTML 時身分與 `{{char}}` 換人、指定卡＋接管骨架；舊跨卡接力測試改期待畫不出）、已貼出開場白認卡、空桌開場世界書卡先、同標籤兩卡指定另一張畫不出骨架；`useCardInterfaceController.test.ts`（分頁只在面板開著且 ≥2 張、固定後新回覆不跳走、固定卡在十樓以外仍顯示且面板不關、固定卡畫不出自動退回不關介面、兩張 MVU 卡相同 HTML 切頁換 token 舊佇列結束舊 token 被拒、換桌與回大廳再進同桌重置、組字 Esc）、`CardInterfaceOverlay.test.tsx`（分頁 aria／後備名／位置、輸入列與 `buildOverlayComposer` 停用擋送與容量提示）、`broken-image.test.ts`（墊片 error 加標記、load 移除，verify 擋退步）。草稿在忙碌／失敗時不丟不重送由既有 `chat-send-failure.test.tsx` 蓋（同一支 send）。
真實 WebKit（`npm run test:webkit`，不進 verify）：`broken-image.webkit.test.tsx` 真 sandbox 驗壞圖隱藏且尺寸照留→卡片 onerror 換有效 src→恢復、正常圖不動。
測試通道（零 AI，`invoke append_transcript` 追樓）：
1. 西幻（世界書卡）＋donass 同桌：GM 旁白含 `<GoldenRPG_UI>`、donass 台詞帶 `<StatusData>` → 兩顆分頁，切換各顯示各自的殼；固定西幻後 donass 再說一句不跳走。
2. DongeonMaster＋bcd368、NorthHall＋RPGImmortal 同桌（同標籤）：空桌兩張開場都畫得出、可切換，殼內容各是自己的。
3. 單卡西幻：沒有分頁鈕；`harness-e2e.mjs` 回歸照過。
4. 輸入列：`harness-fake-openrouter.mjs` 當假端點（API 設本機 base_url＋chat_completions＋delayMs），介面開著打字送出 → 逐字稿有玩家句與回覆、請求一次、介面留著。
