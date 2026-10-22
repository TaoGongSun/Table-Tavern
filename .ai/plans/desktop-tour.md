# desktop-tour 計畫：網頁版的桌面版導覽頁（圖文並茂、以吸引下載為主）

交接檔：[handoffs/desktop-tour.md](../handoffs/desktop-tour.md)。立案 2026-10-22；第二版已納入 Sol、Grok、opus-review 第一輪意見。

## 一、目的與裁決

- 目的：玩過網頁版的人打開這頁，幾十秒內知道桌面版多了什麼、想下載。**吸引人優先，文案與版面美感其次**。〔作者裁決 2026-10-22〕
- 截圖遮罩：**只遮第三方卡的內容**（別人的卡圖、卡上文字、由那張卡產生的對話）；App 介面文字與我們自己的範例桌內容照常顯示。〔作者裁決 2026-10-22〕
- 截圖語系：**繁中＋英文兩套**；繁中、簡中頁面用繁中截圖，其餘八語系用英文截圖。〔作者裁決 2026-10-22〕
- 頁面位置、與遊玩頁的關係、特色取捨由模型決定；程式面採審查方建議，設計面主線（Fable）有否決權。〔作者裁決 2026-10-22〕
- 不在作者兩條件內、另加的遮罩：作者本人的金鑰、額度餘額、帳號或 CLI 登入狀態、用量數字一律不入鏡（避開或遮）。〔模型判斷·未裁決〕

## 二、位置與關係〔模型判斷·未裁決〕

- **沿用 `#download`**：把現有下載小對話窗（`web/src/features/funnel/DownloadPage.tsx`）擴成滿版、自己捲動的導覽頁，仍是蓋在遊玩頁上的覆蓋層、關掉回原桌、網址可分享（D36 作者裁決維持）。理由：宿主 CSP 只給 `/`（`web/site-headers.ts`）、`404.html` 不做 SPA 退回，新開路徑要另加標頭與驗證；hash 路線所有既有入口（頁首常駐連結、額度用完面板、`DesktopOnly` 提示）零改動。
- 頁內不得用 `href="#…"` 當跳轉（會改掉 hash、關掉覆蓋層）；要跳用 `scrollIntoView`。本案不需要頁內跳轉。
- 不另加新的導流節點：D10 的固定節點已夠；本案把「到了下載頁之後」做好。
- 部署不在本案：合進 main 後由作者在 Cloudflare Pages 儀表板手動部署（D12／D42）。

## 三、頁面結構（由上而下）

1. **固定頁首**：左「Table Tavern 桌面版」、右「回到遊戲」鈕（全頁只有這一顆關閉鈕，底部舊的 ghost 鈕拿掉）。
2. **主視覺**：主標＋副標＋**第一組下載鈕**（`data-testid="download-cta"`）＋版本行（`download-version`，只出現這一次）＋主截圖（鍵 `table`：範例桌遊玩畫面，GM 旁白＋三個角色各一句；全部用 `invoke append_transcript` 零 AI 寫入，`scene` 取 `read_state` 的 `current_scene`，不送真的回合）。桌機左文右圖；手機先文與下載鈕、圖在下，下載鈕留在第一屏。
   - 下載鈕契約沿用現行規則：沒有對應安裝檔就不出該平台鈕，「所有版本與檔案」永遠在；鈕上文字不變（版本號只在版本行）；macOS 鈕保留 Apple Silicon 標示。
3. **特色卡**（區塊標題「桌面版多了什麼」）。有截圖的卡＝左右交錯的圖＋標題＋一行；沒截圖的卡＝圖示卡，三欄一排。順序與文案（繁中正典，其餘九語系翻譯；主標、副標都算在內）：
   | 鍵 | 形式 | 標題 | 一行說明 | 截圖內容 |
   |---|---|---|---|---|
   | （主標） | — | 桌面版：一整桌角色，連 GM 都是 AI | — | — |
   | （副標） | — | — | 沒有每日次數上限：用自己的 OpenRouter 額度，或接上 Claude、ChatGPT、Gemini、Grok 訂閱。網頁版的存檔匯出後直接匯入，接著玩。 | — |
   | `subscription` | 截圖 | 用你已有的 AI 訂閱 | 接上你已經付費的 Claude、ChatGPT、Gemini、Grok 訂閱，或自己的 OpenRouter 額度，不受網頁版的每日次數限制。 | 設定 › AI 連線的供應商選擇畫面；拍前人眼確認沒有金鑰、帳號、CLI 已安裝／已登入狀態，有就遮或換分頁 |
   | `oneLine` | 截圖 | 一句話開一張桌 | 描述你想玩的故事，App 生出世界、角色與開場。 | 大廳「一句話開桌」對話窗，輸入框填一句我方自寫的故事描述（不送出） |
   | `status` | 截圖 | 數值由 App 記著 | 金錢、天氣、好感這類會變的數字由 App 保存，不靠模型的記憶。 | 範例桌頂部狀態欄（App 自己的 `StateBar`），用 `invoke set_table_state`（`worldId`＋`fields` 平欄 map）填我方自寫的 3–4 個欄位值；狀態欄要 `world_has_state_bar` 為 true 才出現（`data/world.rs`：世界設定、世界書條目或角色卡文字含狀態欄標記），範例桌預設沒有，先 upsert 一條內文含標記的條目再重進桌；狀態欄預設收合（只剩時間｜地點｜在場一行），拍前先 `click` 狀態列展開 |
   | `editor` | 截圖 | 自己的世界、自己的角色 | 編輯角色卡與世界書；條目可以只讓特定角色知道。 | 範例桌的世界編輯器；範例桌沒有世界書條目，先用 `invoke upsert_worldbook_entry` 加 2–3 條我方自寫條目再拍 |
   | `scenes` | 圖示 | 換幕與前情提要 | 長篇故事換幕時自動寫前情提要，故事再長也接得下去。 | 不拍（換幕一定要模型呼叫；fresh root 沒有金鑰，也不得讀正式設定） |
   | `local` | 圖示 | 資料在你自己的電腦 | 桌、角色、對話都是本機檔案，不會被瀏覽器清掉；備份就是複製一個資料夾。 | 不拍 |
   - 重構、生圖與畫廊、佈景、贊助、用量頁只進對照表：重構說不清楚就是技術名詞；生圖要付費模型、範例角色沒有立繪；其餘不是吸引點。
4. **功能對照表**：沿用 `feature-compare.ts`，移到特色卡之後。
5. **安裝被擋的繞過步驤**（文案沿用現有鍵；說明文字不得用 `.chat-note`，它在 ≤560px 會被藏起來）＋**第二組下載鈕**（`data-testid="download-cta-bottom"`，同一套契約，不重複版本行）。

### 覆蓋層行為
- 滿版：拿掉 `.download-page` 的 640px 上限；捲動只在覆蓋層內（`overscroll-behavior: contain`），開著時 `body` 鎖捲動；對照表維持 `.compare-scroll` 內部橫捲。
- 焦點：打開時焦點移到「回到遊戲」鈕，關閉時還給開啟前的元素；Esc 關閉。不做 Tab 圈禁（罕見邊際輕防）。
- 深色模式：截圖外框 1px `var(--line)`、圓角、陰影，兩種主題都用現有 token。

## 四、截圖規格

- 工具：測試通道（`Table Tavern Harness.app` 已打好；`node scripts/harness.mjs launch --fresh --config-from <設定檔> --root <root>`；`shot <out.png>`；`invoke <command> <json>`）。不碰正式 app、不讀正式資料目錄與正式設定；設定檔裡不放金鑰。
- 每個語系一個獨立 `--root`、一份設定檔（只差 `preferences.language`：`zh-TW`／`en`；theme 不設＝app 預設深色，兩套都深色），桌用 `invoke create_sample_world {"lang": …}` 建；台詞、開桌描述、狀態欄欄位、世界書條目全部我方自寫，兩語系各寫一份，角色名一律直接讀該語系範例 JSON（`src-tauri/samples/zh-TW.json`、`en.json`）取用，不手打。
- 視窗固定 800×600（通道沒有改視窗大小的指令，`core:default` 也不含 `allow-set-size`）；`shot` 出來是 1600×1200，其中內容只有 1600×1136（innerHeight 568 × dpr 2），底部 64px 是空白底色帶。
- 後製一步到位：`cwebp -crop 0 0 1600 1136 -q 82 -o <鍵>.<lang>.webp <shot.png>`（寬高先用 `sips -g pixelWidth -g pixelHeight` 確認，第一張實拍要核對頂列完整、灰帶全除）。預設輸出 1600×1136；單張 > 200 KB 就 `-q 70`，再超過允許 `-resize 1400 994`（維持比例，`<img>` 的 width/height 仍寫 1600×1136 當比例）；兩語系合計 ≤ 3 MB。不加新依賴。
- fresh root 沒有連線，桌內會有「連接 OpenRouter 就能開始玩」的 onboarding 面板蓋住對話：拍 table／status 前用 `js` 把它 `display:none`，不填金鑰，交接檔註明。
- 遮罩：預期全部是我方內容、不需要遮。若畫面不得不含第三方內容或作者帳號資訊，拍前用 `js` 對該 DOM 元素套 `filter: blur(10px)` 再 `shot`。每張截圖在交接檔列出畫面與有無遮罩。
- 落點：`web/src/features/funnel/tour/<鍵>.<zh-TW|en>.webp`（`check-structure` 只檢查 .ts/.tsx/.css）。頁面用 `import.meta.glob("./tour/*.webp", { eager: true, query: "?url", import: "default" })` 讀；選圖抽成純函式 `pickTourImage(map, key, lang)`（zh-TW／zh-CN→繁中圖，其餘→en 圖；對應語系缺圖就退圖示卡），截圖與頁面兩條施工線平行、互不阻塞。經 Vite 打包進 `/assets/*`（一年 immutable 快取），CSP `img-src 'self'` 不必改。`<img>` 一律 `width="1600" height="1136"`，CSS `max-width: 100%; height: auto`、`alt`＝該卡標題；主圖以外 `loading="lazy"`；`onerror` 時整張卡退成圖示卡（不留破圖）。
- 全部 macOS 外觀，不另拍 Windows（只有這台 Mac 能跑通道）。

## 五、驗收

- `npm run verify` 全綠（含 web vitest、web build、check-structure、i18n）；`cd web && npm run smoke:dist` 過。
- vitest `download-page.test.tsx`：連結斷言改成上下兩組（各自的 testid）；版本行只一次；關閉鈕只一顆；`pickTourImage` 餵假 map 測語系對應與缺圖退圖示；`<img>` 有 width/height/alt、主圖外 lazy。
- e2e `web/e2e/run.mjs`（B 的範圍）：無正式版時整頁連結＝兩筆發佈頁（上下各一）；有正式版 exe／dmg 斷言改用 `download-cta` 內的 `nth`；`回到遊戲` 只一顆；`/#download` 直接開導覽；`/download` 仍 404；既有串流不中斷、草稿與額度面板恢復案例保留。
- 瀏覽器實開 `vite preview`：375×667 下 zh-TW、de、fr、ru 四語系與 1280×800 下 zh-TW：下載鈕在第一屏可見、`documentElement` 無橫向溢出；深色模式截圖外框 1px `var(--line)`。
- 截圖逐張人眼核對：沒有第三方內容、沒有作者金鑰／餘額／帳號／CLI 登入狀態；兩套張數相同（各 5 張：table、subscription、oneLine、status、editor；兩套共 10 張）。
- 三方（主線、Sol、Grok）＋opus-review 驗收通過才結案。

## 六、分工

- 子代理 A（opus-medium）：兩套截圖＋後製，只動 `web/src/features/funnel/tour/` 與交接檔「截圖清單」段。零 AI 呼叫。
- 子代理 B（opus-medium）：頁面、樣式、i18n（十語系）、vitest、e2e，不動 `tour/`。
- 主線：讀 diff、重跑 verify、送審驗收、壓縮合併。

## 七、已知限制

- 截圖是 macOS 外觀；Windows 玩家看到的視窗邊框不同。
- 兩套語系：日、韓、歐語玩家看到英文介面截圖。
- 1600px 寬的截圖縮到手機寬時文字難辨，靠標題與一行說明補足。
- 換幕、重構、生圖沒有截圖。
