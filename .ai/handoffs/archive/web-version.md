> 結案 2026-10-07：包 1–9 已進 main（一包一筆），Sol、Grok 驗收通過，`table-tavern.pages.dev` 站上實驗全過。

# web-version — 網頁版：點開網址就能玩的免費引流入口

計畫：[plans/web-version.md](../../plans/web-version.md)（範圍、架構、格式契約、九包＋公開前門檻、查證結論、作者拍板 D1–D43、驗收方式）。

## 定位
點開網址就能玩的免費入口，用來引流：玩家玩完透過網頁版的連結下載桌面版。網頁版是漏斗入口，桌面版才是主力產品〔作者裁決 2026-10-07〕。

## 已定（照辦，不翻案）
- 網頁版與桌面版分開做，只保留基礎功能；網頁版不常更新，主力更新桌面版。〔作者裁決 2026-09-30〕
- 功能架構與頁面美感全新設計，不參考舊設計；只沿用使用者看不到的底層功能。〔作者裁決 2026-09-30〕
- 重構按鈕、快取命中／用量頁不進網頁版。〔作者裁決 2026-09-30〕
- 做卡片內嵌世界書的關鍵字觸發（只觸發，不做編輯器）。〔作者裁決 2026-09-30〕
- 網頁版存檔要能匯入桌面版；桌面版要新增對應的匯入。〔作者裁決 2026-09-30〕
- 支援卡片介面（原卡直玩的介面渲染與 MVU 變數），盡量做到跟 ST 一樣。〔作者裁決 2026-10-04〕
- 要有「去哪裡找卡」的導流；爬別站、另存別站卡的站（JannyAI、CharaVault、DeepSeek Tavern 等）不收。〔作者裁決 2026-09-30〕
- 網頁版的桌檔與卡片（角色卡＋內嵌世界書的複合卡）要能跟桌面版通用。〔作者裁決 2026-10-07〕
- D1–D43 全部拍板，見計畫第五節。〔作者裁決 2026-10-07〕

## 現況
- **包 1（最小縱切）封板**：main 上「web-version: 包 1」那筆，Sol、Grok 驗收通過，主線 verify 12 步綠、e2e 過。
  - `web/` 是獨立 Vite＋React 專案（自己的 package.json），`@desktop/*` 別名共用桌面版純 TS（D2）。
  - 內容：OpenRouter PKCE／貼金鑰、穩定免費選模與換模兩發（不試打）、範例卡「瑟拉」、串流含停止／停滯逾時／失控上限／送出互斥／取消兩分法、頁首常駐下載連結（GitHub Releases API，404 退 releases 頁）、今日免費次數與用完導流面板（只認每日證據，每分鐘限流不算）。對話只在記憶體。
  - repo 設定：根 vitest 排除 `web/**`、check-structure 也走 `web/src`、verify 加 web vitest＋build、verify.yml 加 `npm ci --prefix web`、STRUCTURE.md 有 web 一節。
- 包 1 未驗：手機實機、正文持續串流中按停止（只測過吐一段後卡住再停）。真 OpenRouter、正式網域、Chromium 已在站上驗過。
- **包 2（匯入卡）封板**：main 上「web-version: 包 2」那筆，Sol、Grok 驗收通過，主線 verify 12 步綠、e2e 過。
  - 卡片契約：`src/shared/contracts/card-view/`（card-view.md、fixture、golden.json）；桌面版 `src-tauri/src/import/card_view_tests.rs` 與網頁版 `web/src/features/cards/card-view.ts` 對同一份黃金檔。條目展開一律走 `import::card::book_entries_keyed`。檢視含 `validity`，網頁版只收角色卡路有效的卡。
  - 網頁版 ST 行為（釘版本 SillyTavern `06bde939`）：`web/src/features/sillytavern/`（巨集引擎、變數、seedrandom、regex 三時機分流、mes_example 切段；時間巨集直接用 ST 同版 moment 2.30.1）；提示組裝 `web/src/features/chat/prompt.ts` 的 `composePrompt`（卡欄位→第 0 則寫回→其餘照 ST 先後）；每一發派送用該發模型與上限重組，副作用只在真正派送那次落地（`useChat.ts`）。
  - D21：卡內 regex 腳本預設不允許、匯入預覽有開關，結果存 `PlayCard.regexAllowed`（存檔要帶）。
  - personality 含 setvar 會跑兩次，照 ST（測試「a personality with setvar runs twice like ST」）。
  - TestCards 本機 35 檔 Rust／TS 檢視一致（`TT_CARD_VIEW_TESTCARDS`、`TT_CARD_VIEW_OUT`：先跑 cargo 的 `writes_local_testcard_views_when_requested` 再跑 web 的 card-view 測試）。
  - 惡意卡 regex 的 ReDoS 風險同 ST（玩家允許後才套）。
- **包 3（桌檔契約＋桌面版匯入器）封板**：main 上「web-version: 包 3」那筆，Sol、Grok 驗收通過，主線 verify 12 步綠。
  - 契約：`src/shared/contracts/web-save/`（web-save.md、fixture：short／minimal／worldbook-route／worldbook-route-mvu／future-version、共用負例 `invalid/<分類>--<案例>.json`、TS 檢查 `web-save.ts`）。
  - 桌面版匯入器 `src-tauri/src/import/web_save/`（parse／mod／pending／tests／pending_tests），指令 `import_web_save`、`confirm_web_save_import`、`discard_web_save_import`。資料層：`data/scene/import.rs`（整幕寫入）、`message_vars::publish_imported_seed`、`card_vars::fill_missing`（帶 journal 掛點，寫前先記）／`retract_filled`（rev compare-and-set，`prev_rev` 判沒落地）、`worldbook::import_worldbook_as`、`world::create_world_exclusive`、`import::card::import_character_placing`、`mechanism`／`interface` 的嚴格變體。D24 桌層旗標 `WorldState.regex_allowed`。
  - 匯入成功到前端確認之間，新桌 `web-save-pending.json` 記跨桌層補了什麼；前端寫好卡片 storage 才 confirm，放棄走 discard（撤回跨桌層再刪桌）。
  - 前端：`useImportController` 依內容認存檔；`features/import/web-save-table.ts` 進桌前寫卡片 storage（失敗問重試、放棄走 discard）。
  - 測試通道實測（claude Sonnet）：短 fixture 匯入→送一句→下一輪尾段含 constant 與觸發條目；長 fixture 容量滿→換幕中停止原幕完整→重試換幕→新幕有回覆，MVU、各層、卡片 storage 都在。
  - 未驗：Windows；網頁版真匯出的存檔（包 4 的來回測試補）；世界書路的 AI 對話只在 cargo 測過。桌面版本來就沒有送模前／玩家輸入的 regex（D24 只關掉介面顯示腳本）；角色回覆不更新 MVU 表、換幕後回覆落成 GM 旁白，都是桌面版既有行為。
- **包 4（網頁存檔）封板**：main 上「web-version: 包 4」那筆，Sol、Grok 驗收通過，主線 verify 12 步綠、e2e 過。
  - `web/src/features/saves/`：save-store（IndexedDB：meta／data／globals 三個 store、DB 版本 2；寫入整筆撤銷、清資料後重開、同格舊版不蓋新版；global 寫入在預約當下定版本與資格——預約時還沒讀回過的那筆永遠不寫，重試成功也不放行，`loadGlobal` 補缺與標記載入同一段同步程式）、web-save-codec（契約 v1 ⇄ 一桌，用不到的欄位與時間原文原樣帶回；global 照寫入紀錄只寫這格原有＋本桌寫過的鍵）、st-chat（D3 ST 聊天檔）、download（存檔、ST 聊天檔、原卡檔）、pending-input（D28 草稿，sessionStorage）、SavesPanel／ExportFunnel。
  - useChat：自動存檔只在回合結束後、同格排隊、只有最新那次嘗試改失敗提示；新桌第一次開口先存開口前的樣子、寫成才送模（D27）；回合中重新整理退回上次完整回合、玩家那句放回輸入框（D28）。App 開站讀回 global（D29），讀不到就提示可重試。
  - 變數表上限兩端共用 `src/shared/contracts/vars-table.ts`↔`src-tauri/src/data/message_vars/json.rs`（邊界案例 `vars-table-boundary.json`）；卡片寫入原文或緊湊寫法任一通過，存檔的表只量緊湊寫法。時間兩端都驗日曆存在。`VariableScope.written` 記這桌寫過的鍵。
  - 來回測試：e2e 帶 `TT_WEB_EXPORT_OUT` 另存的真匯出 `src/shared/contracts/web-save/web-export.json`，cargo `a_real_web_export_puts_transcript_in_history_and_chat_vars_in_the_chat_layer`；測試通道 claude Sonnet 從介面匯入後送一句有接續回覆（約 0.011 美元）。ST 聊天變數落桌面版 chat 層檔案；桌面版巨集只換 `{{user}}`／`{{char}}`，chat 層的值不進桌面版提示（桌面版既有限制）。
  - D27–D29 已裁決（計畫第五節）。未驗：手機實機；世界書觸發狀態（包 5）、卡片 storage 與 MVU（包 6）的來回測試已各自補上。
- **包 4b（上下文預算）封板**：main 上「web-version: 包 4b」那筆，Sol r2／Grok r2 通過。估算器照 D30：ST 計數結構＋逐欄位 guesstimate，可替換。
  - `web/src/features/sillytavern/tokens.ts`：ST 計數結構（依 `architecture.tokenizer`＋模型 id 選 tiktoken／HF／sentencepiece 計法、每則開銷、name、前端 -2），文字換 token 是可替換的 `TextTokens`（目前 guesstimate）。包 5 的 WI 預算沿用它。
  - `chat/prompt.ts` 照 populateChatCompletion 裁切：預留 3、固定段落先佔（佔不下 `overflow`）、[Start a new Chat] 先預約、歷史新到舊邊代換邊放、剩下放範例。`useChat` 每支模型照自己的上限組一次（memo），`FreePool.plan(holds)` 只挑放得下固定段落的；穩定模型只因上限不夠被篩掉→`AI_PROMPT_EXCEEDS_CONTEXT`。平台 400／413 說太長→`AI_CONTEXT_TOO_LONG`（同桌面版 context_overflow）。
  - 測試：`chat/context-budget.test.ts`（表格，tiktoken 與 Qwen／DeepSeek 兩族）、`chat/context-limit.test.tsx`（選模、換模重算、平台拒收、真 FreePool）。未驗：估算與真 tokenizer 的落差。
- **包 5（世界書觸發）封板**：main 上「web-version: 包 5」那筆，Sol r3、Grok r3 驗收通過，主線 verify 12 步綠、e2e 過。
  - `web/src/features/sillytavern/world-info-book.ts`：卡內 `character_book` → ST 條目（陣列形照 convertCharacterBook、物件形照 ST 世界書檔預設、@@ 裝飾）；條目先後照 ST 載入（`Object.keys`：陣列形以 `entry.id` 當鍵，整數鍵由小到大、重複 id 後蓋前；物件形照原鍵序），再照 order 穩定排序；穩定 ID `wi-<契約 key>`。
  - `world-info-scan.ts`：checkWorldInfo（掃描深度、主鍵／正則鍵＋無效正則當字面、次要鍵四邏輯、全字與大小寫、constant、機率、遞迴與延遲遞迴、25% 預算用 4b 估算器、sticky／cooldown／delay、群組優先／權重／計分）；設定照 ST 預設，最少觸發數與遞迴步數上限為 0 沒搬。
  - `chat/prompt.ts`：世界書前／後是固定段落、範例上／下併進範例、作者註記上／下（深度 4）與依深度插入照 `chat/injections.ts`；outlet 照 ST 留到下一次掃完才換（上一輪的值經 `st-text.ts` 的共用巨集上下文給所有代換，含玩家句；換模那發以 extra 蓋成回合開頭快照），只在分頁裡、不進存檔。`chat/world-info-setup.ts` 管一桌的 ID 與觸發狀態；派送那一發才落地、每一發（含換模第二發）都從回合開頭的狀態掃。
  - 契約三：`timed`＝sticky／cooldown 兩張「穩定 ID → {start,end,protected}」，數字照值判斷（2.0、5e0、-0 收，1.5、≥2^53 拒，兩端一致，`timed-number-forms.json` 正例＋`invalid--timed-*` 負例）；回退照 ST 由則數判斷；`message_effects` 網頁版寫 `{}`；outlet 不進存檔。
  - 來回測試：e2e 帶 `TT_WEB_EXPORT_WI_OUT` 另存 `src/shared/contracts/web-save/web-export-world-info.json`，cargo `a_real_web_export_with_world_info_keeps_the_trigger_state_in_the_sidecar`。
  - 測試：`sillytavern/world-info-scan.test.ts`（觸發與位置表格、載入順序、回退表格、遞迴、預算、計時、機率、群組）、`chat/world-info-prompt.test.ts`（位置、outlet 跨輪、固定段落預算）、`chat/world-info-turn.test.tsx`（存檔來回、真停止路徑、換模第二發、outlet 跨輪不進存檔、卸載晚回與 D28 重整不落地）。
  - D31：群組篩選同一條重複移除時找不到就不刪（不照 ST 誤刪）；D32：關鍵字正則 ReDoS 照 ST 不處理。
- **包 6（卡片介面）封板**：main 上「web-version: 包 6」那筆，Sol r4、Grok r4 驗收通過，主線 verify 12 步綠、e2e 過。
  - 介面：訊息裡含 `html>`／`<head>`／`<body` 的程式碼區塊照酒館助手畫在該則訊息裡（不是桌面版的覆蓋層），`web/src/features/card-interface/`：沙盒 iframe 載同站 `web/public/sandbox.html`（allow-scripts、不透明來源、自帶寬政策），宿主 postMessage 送進桌面版 `buildShellDocument` 組的文件（內建庫、讀訊息、MVU、橋接墊片；墊片送宿主的每種訊息都帶文件 token），iframe 依回報高度長高；`frontend-host.ts` 是唯一訊息入口（origin 必須是 "null"、來源是掛著的 iframe 視窗、token 相符、形狀對，卸載即除名）。`CardFrontend.tsx`：第 1 次 load 送文件、第 2 次是文件寫完，之後再有 load 就當被導走，除名並換新 iframe 與新 token（連續三次就停、給說明）。卡片按鈕送出照玩家句送、卡片 localStorage 進 `card_storage`；DRM／雲端載入器卡給說明不掛；第一次看到介面提示可能外連（記在 localStorage）。宿主 CSP `frame-src 'self'`。介面一包延後載入。
  - 卡片變數：`web/src/features/mvu/`。載 MVU 的卡（`loadsMvu`）照 MagVarUpdate 438f9ffc 開局（initvar → schema → 開場白 `<initvar>` 與自身指令 → 第 0 則與 seed）、每則 AI 回覆以前一張有效表（無則 seed）更新、樓尾補占位。updateVariables 不另寫：`web/mvu-engine-plugin.ts` 建置時把桌面版沙盒的 parseMessage 片段包成 `virtual:mvu-engine`（宿主不 eval），值解析走桌面版同一支 Worker（`evaluate.ts`）。酒館助手類巨集（`macro-like.ts`）送模前與顯示時代換；MVU 桌送模前拿掉占位、世界書用 MVU 推薦值（D33）。character／script 層新開時取卡上酒館助手的資料。執行期（lodash、yaml、json5、jsonrepair）只有用得到的桌才載。
  - 回合中的卡片寫入（`useChat.ts`）：卡片寫入以 `Revisions`（每個目標的流水號，內容逐字比對沒變才同一版）比版本。message 表：回合收尾以 `keepLatestVars` 保住回合中的寫入；MVU 處理回覆以合併後的最新有效表為底，值解析等完再比對底稿，被卡片改過就以新底稿重算，連續 3 次被改就不提交（這則不掛表、不補占位）。chat／global：`turnCommits` 記回合中被外部改過的鍵，每一發（含換模第二發）提交都保留它們的現值，同鍵以卡片為準。類巨集送模時讀該發組提示用的變數副本。
  - 桌面版小改：`card-interface/card-storage.ts` 從 interface-card 拆出（上限照契約量 UTF-8 位元組）、讀訊息墊片的 `chatEvents`／`floorText` 搬進 card-shell-route（墊片不再依賴語系）、橋接墊片訊息帶 token（桌面版 controller 不看）、值解析宿主 `card-mvu-eval-host.ts` 加單筆 256 KB 與排隊 1000 上限。契約格式沒改。
  - 來回測試：e2e 帶 `TT_WEB_EXPORT_MVU_OUT` 另存 `src/shared/contracts/web-save/web-export-mvu.json`，cargo `a_real_web_export_with_mvu_lands_every_table_and_the_card_storage`；測試通道 claude Sonnet 從介面匯入這份存檔，桌面版卡片介面畫出 MVU 值。本機 TestCards（bcd368、DongeonMaster、HeroTraining）開局 initvar 都解得開、開場白都切得出介面區塊。
  - 判斷〔模型判斷·未裁決〕：介面畫在各則訊息裡而不是覆蓋層；character 層初值取卡上酒館助手 variables、script 層取各腳本 data；類巨集照釘版酒館助手在預算與世界書掃描之後才代換（超量提示走平台拒收的既有提示）；顯示時類巨集讀最後一張表（釘版 demacroOnRender 不傳樓號）。
  - 未驗：手機（導走判定靠 document.open／close 照規格再觸發一次 load，只在 WebKit e2e 實測過）；卡片自己 fetch 外連；EJS 不執行（D34）；酒館助手變數函式只給載 MVU 的卡（D35）。
- **包 7（導流全套）封板**：main 上「web-version: 包 7」那筆，Sol r3、Grok r3 驗收通過，主線 verify 12 步綠、e2e 過。
  - 下載頁 `web/src/features/funnel/DownloadPage.tsx`：網址 `#download`（`download-route.ts`，開關不換頁，關掉清網址回原桌）；版本與 Windows／macOS 安裝檔照 GitHub Releases API（`releases.ts` 分 loading／ok／none（只有 404）／error：查不到只說暫時取不到，不說沒有正式版；缺哪個平台檔就不給那顆鈕），沒有（文案只說「還沒有正式版，請到發佈頁查看」，不保證發佈頁上有測試版）、查不到都只給發佈頁；繞過說明分平台講原因（Windows 沒買簽章憑證→SmartScreen「其他資訊→仍要執行」；macOS 沒送 Apple 公證→Gatekeeper「系統設定→隱私權與安全性→仍要打開」），步驟同 README、不叫玩家開終端機、不做安全保證；功能對照表資料 `feature-compare.ts`（計畫 1.3 不進的功能全列，含贊助；測試逐項對 1.3 清單）。
  - D10 節點：頁首常駐連結、額度用完面板的「前往下載頁」、匯出存檔導流都連 `#download`；匯入獨立世界書被拒時接 `DesktopOnly`。D11：額度用完面板只有平台檔與下載頁。
  - 找卡：`web/src/features/find-cards/`（`card-sites.ts` 五站資料、`FindCards.tsx` 放開始畫面，一行 18 禁標示）。
  - DRM／雲端載入器卡的說明拿掉「可以改用桌面版」（桌面版同樣不支援）。
  - 離開頁面（`useChat.ts`，D28）：beforeunload 與非往返快取的 pagehide 會作廢在途回合並停掉自動存檔（WebKit 在 pagehide 前就中斷請求，那個失敗不能當回合收尾），存檔停在上一個完整回合；草稿等含那一回合的存檔寫成才清（流水號比對，舊寫入不清新回合的草稿；沒有存檔庫就落地即清）。已知限制：往返快取的頁面回來時，或 beforeunload 之後導覽沒有真的發生（例如別的程式取消了離開），在途回合已作廢——玩家句留在畫面上、沒有回覆，要重新整理或手動收回；自動存檔要等下一次開口才恢復（往返快取回來時立即恢復）。
  - D36–D38 已拍板（計畫第五節）。
  - 測試：`funnel/download-page.test.tsx`、`funnel/funnel.test.ts`（403／429／500／斷線／讀不懂都不是「沒有正式版」）、`find-cards/find-cards.test.tsx`、`chat/autosave.test.tsx`（離開頁面時的失敗不清草稿、存檔停在上一回合、存檔寫成才清草稿、舊寫入不清新草稿）、`chat/mvu-turn.test.tsx`（MVU 等待中離開草稿仍在）；e2e 走下載頁（無版本與有版本）、串流中開關下載頁不中斷、在 `#download` 上重新整理後草稿放回、導流面板與匯出導流連下載頁、找卡清單。
- **包 8（十語系）封板**：main 上「web-version: 包 8」那筆，Sol、Grok 審查通過；D39–D41 已拍板。
  - 語系清單與首開偵測在桌面版 `src/i18n/languages.ts`（桌面版 index 從這裡轉匯出，網頁版經 `@desktop` 共用，不帶桌面字典本體）；單複數沿用桌面版 `plural.ts`。
  - 網頁字典 `web/src/i18n/`：`zh-TW.ts` 是正典（`WebMessages` 型別，鍵多鍵少都編譯失敗），另九個語系各一檔；`t()` 查不到鍵退回繁中。用語照桌面版各語系主流用語：ja 桌＝「卓」、世界書＝「ロアブック」；fr 世界書＝「Encyclopédie」、稱呼用 tu；ru 稱呼用 ты；refactor 照桌面版按鈕（es／pt Reorganizar、fr Réorganiser、ru Разобрать）。系統對話框裡的字（SmartScreen、Gatekeeper 的標題與按鈕）照各 OS 該語系原文，不跟著改稱呼。桌面版自身幾鍵用語不一致（ja refactorSummaryEntries「世界書」、fr refactor 幾鍵「livre du monde」）主線另告作者，本分支不動。
  - 預設玩家名一律用名詞（Player／Spieler／Joueur／Jugador／Jogador…）：沒填名字時它會代進 `{{user}}`、出現在第三人稱句裡。
  - 語系選擇：頁首選單（`features/language/LanguagePicker.tsx`），存 `tt-web:lang`，沒存過照瀏覽器語系（同桌面版，對不到用英文）；`i18n/lang-store.ts` 同步 `<html lang>`、標題、描述，存取 localStorage 丟例外也不炸。換語系只重繪：桌、草稿、輸入框、串流、卡片介面 iframe 都不動；畫面上的錯誤（useChat、存檔區 `SavesPanel`、匯出列 `ExportBar`）存成「怎麼說」，繪製時才翻。
  - 範例卡各語系（`features/cards/samples/`，`samplePlayCard(lang)`）；開了桌卡就固定，換語系不換卡。玩家名沒填就用該語系預設名（不寫進名字欄）。
  - `index.html` 加靜態描述與 og／twitter meta（繁中，D39）。窄螢幕（≤560px）頁首可換行、下載連結可折行（法文帶版本號的連結原本在 375px 撐出橫向捲動）。
  - 測試：`features/language/dictionaries.test.ts`（十語系鍵集合、佔位符、單複數語法含俄文 one／few／many、執行期退回繁中、偵測順序含 zh-Hans／zh-Hant／zh-SG、storage 丟例外、範例卡齊全、靜態 meta 與正典同步）、`features/language/relabel.test.tsx`（存檔區與匯出列的錯誤換語系後重翻）；e2e：主流程固定 `zh-TW`，串流中換英文再換回不中斷、MVU 卡片介面換語系不重掛（同一 iframe、同一文件）；最後另開獨立英文瀏覽器情境走登入、額度面板、開始畫面、範例卡、下載頁，桌上換日文不丟桌與草稿、重新整理維持日文，十個語系的開始畫面與下載頁在 375px 寬不橫向溢出。
  - 未驗：九個語系譯文與範例卡未經母語者審閱；系統對話框原文沒有實機逐字核對，其中不確定的四處維持現狀——es SmartScreen 標題「Windows protegió su PC」（可能是「tu PC」）、es macOS 第一鈕寫「OK」（可能是「Aceptar」）、ja Gatekeeper「検証できません」（可能是「確認できません」）、ko SmartScreen 按鈕「실행」（可能是「그래도 실행」）；按鈕寬度只靠 375px 溢出檢查，沒有桌面版 check-i18n 那種逐鍵寬度表。
- 桌面版尚未發過任何 GitHub release（`releases/latest` 回 404）。
- 測試用模型〔作者裁決 2026-10-07〕：網頁端用本機假端點，桌面端用測試通道＋claude CLI Sonnet；真免費模型已在站上實送成功（Gemma、Nemotron 忙線→換 Ling 3.0 Flash 回覆，換模提示正確）。
- 作者實測：OpenRouter 角色扮演排行前兩名免費模型都能輸出 NSFW（DeepSeek 較保守、GLM 很開放）。〔作者實測 2026-09-30〕
- **包 9（上線）封板**：main 上「web-version: 包 9」那筆，Sol r2、Grok r2 複驗通過，主線 verify 12 步綠、e2e 過、smoke:dist 過。部署走 Cloudflare Pages Git 連接、儀表板手動部署（D42，專案 `table-tavern`、網址 `table-tavern.pages.dev`（D43）；作者已建好專案，設定見計畫「部署與上線檢查」）。
  - `web/site-headers.ts` 建置時寫出 `dist/_headers`：宿主 CSP 只由標頭給（index.html 不再有 CSP meta），CSP 不掛 `/*`（Pages 會把同名標頭併成多份政策），宿主 CSP 只給 `/`、多了 `frame-ancestors 'none'`；沙盒路徑只給 `frame-ancestors 'self'`，寬政策用 `sandbox.html` 自帶的 meta；`/assets/*` 長快取；`web/public/404.html` 讓缺檔回 404。
  - `sandbox.html` 只在 `window.origin === "null"` 時收卡片（外站用一般 iframe 嵌它時來源是本站，會讀到金鑰）。
  - `web/site-headers.test.ts`：`_headers` 逐行快照與反例（vitest include 加了 web 根層 `*.test.ts`）。
  - `web/e2e/static-server.mjs` 照 Pages 行為託管產物（套 `_headers`、`.html`→無副檔名 308、缺檔回 404.html；與真 Pages 的差異寫在檔頭）；e2e 改用它（不再用 vite preview），開頭驗標頭。`npm run smoke:dist` 驗正式建置。
  - 本機託管把首頁別名（`/index`、`/index/`、`/index.html`）308 回 `/`（query 保留）、空路段（`//`）回 404，e2e 有對應反例；`//` 在真 Pages 是正規化回首頁（與 Pages 的差異列在 `static-server.mjs` 檔頭）。
  - 可部署；正式公開等下面的公開前門檻結案。

## 公開前門檻（計畫第三節原文，正式公開前要到位）
- 有可下載的桌面版 release：[release-2-ci-windows](../../tasks/release-2-ci-windows.md) 發出首個正式版。
- 介面卡兩案首發必含：[interface-card-panel](../interface-card-panel.md)、[interface-takeover-spike](../interface-takeover-spike.md)。
- 目標 release 上，完整複合卡（世界書＋介面＋MVU）的網頁存檔匯入後能續玩。
- 桌面版 WI 補齊到 ST 行為：[worldbook-st-trigger-parity](../../tasks/worldbook-st-trigger-parity.md)（D17）。
- 匯入卡的世界書角色看得到：[worldbook-character-visibility](../../tasks/worldbook-character-visibility.md)。

## 站上實驗（`table-tavern.pages.dev`，全部已驗）
- 第一次部署：首頁與回呼網址（`/?code=`）帶宿主 CSP；`/sandbox` 只帶 `frame-ancestors 'self'`；首頁別名 308 回 `/`；缺檔（`/assets/missing.js`）與 `/INDEX` 回 404（no-store）；`//` 被 Pages 正規化回首頁 200 並帶宿主 CSP（本機模擬回 404 是模擬差異）；資產 immutable；下載頁正確；外站一般 iframe 嵌 `/sandbox` 與 `/` 都被擋。
- PKCE 修正後部署（Chrome）：PKCE 登入成功（真 OpenRouter 導回 `/?code&state`）；CORS（`/key` 預檢 204、GET 200、chat/completions POST 200）；真免費模型實送（Gemma、Nemotron 忙線→換 Ling 3.0 Flash 回覆，換模提示正確、今日免費 49/50）；自動存檔（選卡頁「接著玩」）；卡片介面（匯入 `web/e2e/interface-card.json`、允許 regex，沙盒 iframe 畫出狀態與按鈕、外連提示出現、按鈕寫入 MVU，零 console error）。
- 手機實機：作者自行看。

## 結案待辦
- 作者把 Cloudflare Pages 正式分支從 `web-version` 改成 `main` 並部署一次；之後刪 `web-version` 本地與遠端分支（Pages 還指著它，先不刪）。
- 手機實機：作者自行看。
- 正式公開等上面公開前門檻四案（release-2-ci-windows、interface-card-panel／interface-takeover-spike、worldbook-st-trigger-parity、worldbook-character-visibility）。

接手者需知：
- check-structure 會擋 `use` 開頭的 `.tsx`：hook 測試要用 JSX 就依行為命名（例：`send-cancel.test.tsx`）。用到 DOMPurify 的測試要 `// @vitest-environment happy-dom`。
- 測試用的記憶體 Storage 在 `web/src/features/openrouter/memory-storage.ts`；Node 26 的全域 localStorage 在 happy-dom 底下是 undefined，要 `vi.stubGlobal`。
- e2e（`cd web && npm run e2e`）用 `vite build --mode e2e`，產物由 `e2e/static-server.mjs` 照 Pages 行為託管（沙盒 frame 網址是 `/sandbox`）；只有 e2e 模式接受 `VITE_TT_*` 端點覆寫（`web/src/shared/endpoints/resolve.ts`）。假端點 `chat: "numbered"` 回「第 N 次回覆」。
- 改契約＝桌面版、網頁版、golden.json 同一筆 commit；golden 可用上面兩個環境變數讓 Rust 寫出各 fixture 的檢視再組回。
- ST 巨集測試向量 `web/src/features/sillytavern/st-macro-cases.json` 抽自 ST 06bde939 `tests/frontend/MacroEngine.e2e.js`（只收內建巨集）。
- serde_json 開了 preserve_order（schemars 帶進來），Map 是原序，需要排序的地方自己排。
- 包 3：改契約＝`web-save.md`、fixture、`web-save.ts`、`import/web_save/parse.rs` 同一筆改；負例檔名前綴就是兩端要回的分類，Rust 與 TS 測試都掃 `invalid/`。serde 的 `Option` 欄缺鍵會默默當 None，「必填可 null」要用 parse.rs 的 `Nullable`、「可省不可 null」要用 `present`。
- 包 3 測試通道：harness 包會把 claude 實報的 context（1,000,000）寫回 `data/model-capacity.json`，要壓上限就在每次真呼叫後重寫；容量鎖要有 `summary` 校正才會鎖（`scene_budget/mod.rs` 的 lockable）。本機 shell 包裝會擋「變數展開成路徑」與部分 heredoc，路徑寫字面值、編輯用腳本檔。
- 包 4：e2e 用 Node 直接載入 `web-save.ts` 驗真匯出（Node 26 型別剝除），所以它對 `vars-table` 的 import 帶 `.ts` 副檔名，`web-save.ts` 不能用 enum 等非純型別語法。要重產 `web-export.json`：`cd web && TT_WEB_EXPORT_OUT=<絕對路徑> npm run e2e`，再複製進契約目錄，cargo 來回測試跟著看。
- 包 5：ST 參照（world-info.js 等）不在 repo，照下行網址抓到暫存再讀；世界書的 `entry.content` 代換會改到條目物件，`checkWorldInfo` 一律吃 `structuredClone` 的副本。要重產世界書來回 fixture：`cd web && TT_WEB_EXPORT_WI_OUT=<絕對路徑> npm run e2e`，複製成 `src/shared/contracts/web-save/web-export-world-info.json`。ST 原始碼對照抓 `raw.githubusercontent.com/SillyTavern/SillyTavern/06bde939/<路徑>` 到暫存（world-info.js、openai.js、script.js、authors-note.js）。
- 包 4 測試：IndexedDB 用 fake-indexeddb（`forceCloseDatabase` 模擬瀏覽器強制關連線、`IDBObjectStore.prototype.put` 可 spy 模擬配額）；sessionStorage 要 `vi.stubGlobal` 成 MemoryStorage。新桌第一次送出會先寫一筆首格，受控延遲測試要卡第二筆以後。存檔庫的 global 載入之前預約的寫入一律不寫（測試先 `restoreGlobals` 或 `loadGlobal`）。
- 包 4 測試通道：worktree 內 `npm run harness:build`、`launch --root <暫存> --config-from <自寫 config：transport claude、cli_risk_accepted、tier_models 三檔都 sonnet>`，不用 --fresh；匯入完會跳「已照網頁版存檔開了一張新桌」對話框，要先 `answer next 知道了` 輸入框才解鎖。
- 在 worktree 裡跑 verify 前先 `npm ci` 與 `npm ci --prefix web`：沒有自己的 node_modules 時會解析到主 repo 的，vite 擋外部路徑，幾支 vitest 檔會假紅。
- 包 6 測試：`card-interface/frontend-host.test.ts`（來源、origin、token、形狀）、`chat/card-frontend-turn.test.tsx`（按鈕送出、card storage 進出存檔）、`chat/mvu-turn.test.tsx`（開局、回覆更新、類巨集、存檔與接著玩、重新生成、卡片寫入版本、初始化失敗；競態：回合中寫入當底、值解析等待中寫入重算與連續被打斷、換模與 message／chat／global 寫入、同鍵以卡片為準、該發副本、編輯與刪除、開局等待時停止與卸載；mock 的 `gate` 能卡住值解析、`onRun` 能在每次值解析開始時插一手）、`sillytavern/variables.test.ts`（回合提交紀錄）、`mvu/mvu-parts.test.ts`（類巨集、parseString、版本、快照、MVU 世界書設定）；桌面版 `card-mvu-eval-host.test.ts`、`interface-card.test.ts` 有對應上限測試。e2e 的介面卡 `web/e2e/interface-card.json`（MVU＋兩支顯示腳本），假端點 `state.replies` 照順序回排好的句子；導走測試把開場那支 iframe 導到 `/sandbox.html?navigated`，新的 iframe 是延後載入的，要捲回去才會載。要重產 MVU 來回 fixture：`cd web && TT_WEB_EXPORT_MVU_OUT=<絕對路徑> npm run e2e`。測 MVU 的 hook 測試要 `vi.mock("../mvu/evaluate")` 換成直接呼叫桌面版 `runEval`（測試環境沒有 Worker），開局是非同步的，等到第 0 則掛上表或 `initError` 再動作。沙盒 MVU／酒館助手規格只當規格書讀：MagVarUpdate 438f9ffc、JS-Slash-Runner 46ec10df（用 GitHub raw 抓到暫存）。測試通道匯入入口：開新的一桌 →「新增」→「匯入卡」→ `file input[type=file] <存檔路徑>`，之後有兩個「知道了」對話框，卡片介面覆蓋層會自動打開（關閉鈕「關閉介面」）。
- 「取消未完成回合」自動收回玩家句是網頁版提案〔模型判斷·未裁決〕；桌面版零字停止是由玩家手動收回。
- 包 8 字典：新增文案先加進 `web/src/i18n/zh-TW.ts`，九個語系同一筆補齊（缺了 web build 的型別檢查就失敗，`dictionaries.test.ts` 另查佔位符與單複數語法）；單複數用 ICU 子集 `{n, plural, one {...} other {...}}`，俄文要寫 one／few／many／other。用語先 grep 桌面版 `src/i18n/<語系>.ts` 對應功能的鍵（例：`worldbookTitle`、`refactorBtn`），跟桌面版主流用語走。
- 包 8 文案要能換語系重算：元件裡不要把 `t()` 的結果存進 state；要存就存成 `() => t(...)` 之類的「怎麼說」，繪製時呼叫（參考 `useChat` 的 error、`SavesPanel`、`ExportBar`）。測試裡 `t()` 預設是繁中，要測別的語系就 `setLang` 並在 afterEach 切回 `zh-TW`。
- 包 8 範例卡：`web/src/features/cards/samples/<語系>.ts` 只寫會變的欄位（`SampleText`），其餘欄位在 `sample-card.ts` 統一；各語系的 `{{char}}`／`{{user}}` 巨集要跟繁中同欄同數（測試會擋）。`SAMPLE_PLAY_CARD` 是繁中那份，既有測試都用它。
- 包 8 e2e：主流程 `browser.newPage({ locale: "zh-TW" })` 固定繁中，所有中文選取器都靠這個；換語系的步驟換完要切回 `zh-TW`（`tt-web:lang` 會記住）。英文段在最後用 `browser.newContext({ locale: "en-US" })` 獨立情境跑，那時 /key 已回報用完，進開始畫面和重新整理後都要先關額度面板。

## 之後再談
- 中文找卡：候選只有類腦、旅程兩個 Discord 社群（簡中、要答題、邀請連結會失效）。〔作者：之後再談，2026-09-30〕
