# web-version — 網頁版：點開網址就能玩的免費引流入口

分支：`web-version`（已合 main 到 cb9583a）。計畫：[plans/web-version.md](../plans/web-version.md)（範圍、架構、格式契約、九包＋公開前門檻、查證結論、作者拍板 D1–D18、驗收方式）。

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
- D1–D32 全部拍板，見計畫第五節。〔作者裁決 2026-10-07〕

## 現況
- **包 1（最小縱切）封板**：commit be1093d..0de69dd，Sol、Grok 驗收通過，主線 verify 12 步綠、e2e 過。
  - `web/` 是獨立 Vite＋React 專案（自己的 package.json），`@desktop/*` 別名共用桌面版純 TS（D2）。
  - 內容：OpenRouter PKCE／貼金鑰、穩定免費選模與換模兩發（不試打）、範例卡「瑟拉」、串流含停止／停滯逾時／失控上限／送出互斥／取消兩分法、頁首常駐下載連結（GitHub Releases API，404 退 releases 頁）、今日免費次數與用完導流面板（只認每日證據，每分鐘限流不算）。對話只在記憶體。
  - repo 設定：根 vitest 排除 `web/**`、check-structure 也走 `web/src`、verify 加 web vitest＋build、verify.yml 加 `npm ci --prefix web`、STRUCTURE.md 有 web 一節。
- 包 1 未驗：真 OpenRouter（金鑰無額度 403）、正式網域部署（包 9）、Chromium 與手機實機、正文持續串流中按停止（只測過吐一段後卡住再停）。
- **包 2（匯入卡）封板**：commit 45baf78..52fcd59，Sol、Grok 驗收通過，主線 verify 12 步綠、e2e 過。
  - 卡片契約：`src/shared/contracts/card-view/`（card-view.md、fixture、golden.json）；桌面版 `src-tauri/src/import/card_view_tests.rs` 與網頁版 `web/src/features/cards/card-view.ts` 對同一份黃金檔。條目展開一律走 `import::card::book_entries_keyed`。檢視含 `validity`，網頁版只收角色卡路有效的卡。
  - 網頁版 ST 行為（釘版本 SillyTavern `06bde939`）：`web/src/features/sillytavern/`（巨集引擎、變數、seedrandom、regex 三時機分流、mes_example 切段；時間巨集直接用 ST 同版 moment 2.30.1）；提示組裝 `web/src/features/chat/prompt.ts` 的 `composePrompt`（卡欄位→第 0 則寫回→其餘照 ST 先後）；每一發派送用該發模型與上限重組，副作用只在真正派送那次落地（`useChat.ts`）。
  - D21：卡內 regex 腳本預設不允許、匯入預覽有開關，結果存 `PlayCard.regexAllowed`（存檔要帶）。
  - personality 含 setvar 會跑兩次，照 ST（測試「a personality with setvar runs twice like ST」）。
  - TestCards 本機 35 檔 Rust／TS 檢視一致（`TT_CARD_VIEW_TESTCARDS`、`TT_CARD_VIEW_OUT`：先跑 cargo 的 `writes_local_testcard_views_when_requested` 再跑 web 的 card-view 測試）。
  - 未驗：真模型未實送；惡意卡 regex 的 ReDoS 風險同 ST（玩家允許後才套）；只測 WebKit；token 預算排包 4b（D23）。
- **包 3（桌檔契約＋桌面版匯入器）封板**：commit a3cca5d..6527a28，Sol、Grok 驗收通過，主線 verify 12 步綠。
  - 契約：`src/shared/contracts/web-save/`（web-save.md、fixture：short／minimal／worldbook-route／worldbook-route-mvu／future-version、共用負例 `invalid/<分類>--<案例>.json`、TS 檢查 `web-save.ts`）。
  - 桌面版匯入器 `src-tauri/src/import/web_save/`（parse／mod／pending／tests／pending_tests），指令 `import_web_save`、`confirm_web_save_import`、`discard_web_save_import`。資料層：`data/scene/import.rs`（整幕寫入）、`message_vars::publish_imported_seed`、`card_vars::fill_missing`（帶 journal 掛點，寫前先記）／`retract_filled`（rev compare-and-set，`prev_rev` 判沒落地）、`worldbook::import_worldbook_as`、`world::create_world_exclusive`、`import::card::import_character_placing`、`mechanism`／`interface` 的嚴格變體。D24 桌層旗標 `WorldState.regex_allowed`。
  - 匯入成功到前端確認之間，新桌 `web-save-pending.json` 記跨桌層補了什麼；前端寫好卡片 storage 才 confirm，放棄走 discard（撤回跨桌層再刪桌）。
  - 前端：`useImportController` 依內容認存檔；`features/import/web-save-table.ts` 進桌前寫卡片 storage（失敗問重試、放棄走 discard）。
  - 測試通道實測（claude Sonnet）：短 fixture 匯入→送一句→下一輪尾段含 constant 與觸發條目；長 fixture 容量滿→換幕中停止原幕完整→重試換幕→新幕有回覆，MVU、各層、卡片 storage 都在。
  - 未驗：Windows；網頁版真匯出的存檔（包 4 的來回測試補）；世界書路的 AI 對話只在 cargo 測過。桌面版本來就沒有送模前／玩家輸入的 regex（D24 只關掉介面顯示腳本）；角色回覆不更新 MVU 表、換幕後回覆落成 GM 旁白，都是桌面版既有行為。
- **包 4（網頁存檔）封板**：commit 59f420f..a5410b4，Sol、Grok 驗收通過，主線 verify 12 步綠、e2e 過。
  - `web/src/features/saves/`：save-store（IndexedDB：meta／data／globals 三個 store、DB 版本 2；寫入整筆撤銷、清資料後重開、同格舊版不蓋新版；global 寫入在預約當下定版本與資格——預約時還沒讀回過的那筆永遠不寫，重試成功也不放行，`loadGlobal` 補缺與標記載入同一段同步程式）、web-save-codec（契約 v1 ⇄ 一桌，用不到的欄位與時間原文原樣帶回；global 照寫入紀錄只寫這格原有＋本桌寫過的鍵）、st-chat（D3 ST 聊天檔）、download（存檔、ST 聊天檔、原卡檔）、pending-input（D28 草稿，sessionStorage）、SavesPanel／ExportFunnel。
  - useChat：自動存檔只在回合結束後、同格排隊、只有最新那次嘗試改失敗提示；新桌第一次開口先存開口前的樣子、寫成才送模（D27）；回合中重新整理退回上次完整回合、玩家那句放回輸入框（D28）。App 開站讀回 global（D29），讀不到就提示可重試。
  - 變數表上限兩端共用 `src/shared/contracts/vars-table.ts`↔`src-tauri/src/data/message_vars/json.rs`（邊界案例 `vars-table-boundary.json`）；卡片寫入原文或緊湊寫法任一通過，存檔的表只量緊湊寫法。時間兩端都驗日曆存在。`VariableScope.written` 記這桌寫過的鍵。
  - 來回測試：e2e 帶 `TT_WEB_EXPORT_OUT` 另存的真匯出 `src/shared/contracts/web-save/web-export.json`，cargo `a_real_web_export_puts_transcript_in_history_and_chat_vars_in_the_chat_layer`；測試通道 claude Sonnet 從介面匯入後送一句有接續回覆（約 0.011 美元）。ST 聊天變數只落桌面版 chat 層檔案，桌面版的 MVU 表與提示內的變數等包 6〔模型判斷·未裁決〕。
  - D27–D29 已裁決（計畫第五節）。未驗：Chromium、手機實機、真 OpenRouter（403）；世界書觸發狀態（包 5）、卡片 storage 與 MVU 種子（包 6）屆時各補來回測試。
- **包 4b（上下文預算）封板**：commit d3d475e、f1e201f，Sol r2／Grok r2 通過。估算器照 D30：ST 計數結構＋逐欄位 guesstimate，可替換。
  - `web/src/features/sillytavern/tokens.ts`：ST 計數結構（依 `architecture.tokenizer`＋模型 id 選 tiktoken／HF／sentencepiece 計法、每則開銷、name、前端 -2），文字換 token 是可替換的 `TextTokens`（目前 guesstimate）。包 5 的 WI 預算沿用它。
  - `chat/prompt.ts` 照 populateChatCompletion 裁切：預留 3、固定段落先佔（佔不下 `overflow`）、[Start a new Chat] 先預約、歷史新到舊邊代換邊放、剩下放範例。`useChat` 每支模型照自己的上限組一次（memo），`FreePool.plan(holds)` 只挑放得下固定段落的；穩定模型只因上限不夠被篩掉→`AI_PROMPT_EXCEEDS_CONTEXT`。平台 400／413 說太長→`AI_CONTEXT_TOO_LONG`（同桌面版 context_overflow）。
  - 測試：`chat/context-budget.test.ts`（表格，tiktoken 與 Qwen／DeepSeek 兩族）、`chat/context-limit.test.tsx`（選模、換模重算、平台拒收、真 FreePool）。未驗：真模型（403）、估算與真 tokenizer 的落差。
- **包 5（世界書觸發）封板**：commit 6eb4ce4..3c812a5，Sol r3、Grok r3 驗收通過，主線 verify 12 步綠、e2e 過。
  - `web/src/features/sillytavern/world-info-book.ts`：卡內 `character_book` → ST 條目（陣列形照 convertCharacterBook、物件形照 ST 世界書檔預設、@@ 裝飾）；條目先後照 ST 載入（`Object.keys`：陣列形以 `entry.id` 當鍵，整數鍵由小到大、重複 id 後蓋前；物件形照原鍵序），再照 order 穩定排序；穩定 ID `wi-<契約 key>`。
  - `world-info-scan.ts`：checkWorldInfo（掃描深度、主鍵／正則鍵＋無效正則當字面、次要鍵四邏輯、全字與大小寫、constant、機率、遞迴與延遲遞迴、25% 預算用 4b 估算器、sticky／cooldown／delay、群組優先／權重／計分）；設定照 ST 預設，最少觸發數與遞迴步數上限為 0 沒搬。
  - `chat/prompt.ts`：世界書前／後是固定段落、範例上／下併進範例、作者註記上／下（深度 4）與依深度插入照 `chat/injections.ts`；outlet 照 ST 留到下一次掃完才換（上一輪的值經 `st-text.ts` 的共用巨集上下文給所有代換，含玩家句；換模那發以 extra 蓋成回合開頭快照），只在分頁裡、不進存檔。`chat/world-info-setup.ts` 管一桌的 ID 與觸發狀態；派送那一發才落地、每一發（含換模第二發）都從回合開頭的狀態掃。
  - 契約三：`timed`＝sticky／cooldown 兩張「穩定 ID → {start,end,protected}」，數字照值判斷（2.0、5e0、-0 收，1.5、≥2^53 拒，兩端一致，`timed-number-forms.json` 正例＋`invalid--timed-*` 負例）；回退照 ST 由則數判斷；`message_effects` 網頁版寫 `{}`；outlet 不進存檔。
  - 來回測試：e2e 帶 `TT_WEB_EXPORT_WI_OUT` 另存 `src/shared/contracts/web-save/web-export-world-info.json`，cargo `a_real_web_export_with_world_info_keeps_the_trigger_state_in_the_sidecar`。
  - 測試：`sillytavern/world-info-scan.test.ts`（觸發與位置表格、載入順序、回退表格、遞迴、預算、計時、機率、群組）、`chat/world-info-prompt.test.ts`（位置、outlet 跨輪、固定段落預算）、`chat/world-info-turn.test.tsx`（存檔來回、真停止路徑、換模第二發、outlet 跨輪不進存檔、卸載晚回與 D28 重整不落地）。
  - D31：群組篩選同一條重複移除時找不到就不刪（不照 ST 誤刪）；D32：關鍵字正則 ReDoS 照 ST 不處理。未驗：真模型（403）。
- 桌面版尚未發過任何 GitHub release（`releases/latest` 回 404）。
- 測試用模型〔作者裁決 2026-10-07〕：網頁端用本機假端點，桌面端用測試通道＋claude CLI Sonnet；真免費模型實送等 OpenRouter 有額度再補，每包報告註明「真模型未實送」。
- 作者實測：OpenRouter 角色扮演排行前兩名免費模型都能輸出 NSFW（DeepSeek 較保守、GLM 很開放）。〔作者實測 2026-09-30〕

## 下一步：包 6 卡片介面（施工中）
照計畫分包表「6 卡片介面」列、2.4 節沙盒 iframe 橋接、4.1 宿主端變數語意施工；可拆子步驟多筆 commit，每步 verify＋web vitest＋e2e 綠，全部做完一次送審（主線開新審查串）。需要作者決定的從 D33 起編號、附建議與兩邊後果，先照建議做並標〔模型判斷·未裁決〕。

接手者需知：
- check-structure 會擋 `use` 開頭的 `.tsx`：hook 測試要用 JSX 就依行為命名（例：`send-cancel.test.tsx`）。用到 DOMPurify 的測試要 `// @vitest-environment happy-dom`。
- 測試用的記憶體 Storage 在 `web/src/features/openrouter/memory-storage.ts`；Node 26 的全域 localStorage 在 happy-dom 底下是 undefined，要 `vi.stubGlobal`。
- e2e（`cd web && npm run e2e`）用 `vite build --mode e2e`；只有 e2e 模式接受 `VITE_TT_*` 端點覆寫（`web/src/shared/endpoints/resolve.ts`）。假端點 `chat: "numbered"` 回「第 N 次回覆」。
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
- 「取消未完成回合」自動收回玩家句是網頁版提案〔模型判斷·未裁決〕；桌面版零字停止是由玩家手動收回。介面文案與範例卡只有繁中（十語系在包 8）。

## 等作者
- 開 Cloudflare 帳號並接上 repo（D1），包 9 上線前要好。

## 之後再談
- 中文找卡：候選只有類腦、旅程兩個 Discord 社群（簡中、要答題、邀請連結會失效）。〔作者：之後再談，2026-09-30〕
