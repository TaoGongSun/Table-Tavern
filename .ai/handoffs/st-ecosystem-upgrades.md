# Handoff: st-ecosystem-upgrades

## Current state
2026-08-04：六項全部實作完成（cargo test 174 綠）並**全數實機驗收通過**。2026-08-03 回報的五個問題全數修掉（見下方「實機驗收修補」），狀態列條件顯示同日驗收；第四項狀態欄後續鏈四條（GM 更新／手動改欄位／收回復原倒回／壞格式不中斷）2026-08-04 確認通過，第一期結案。只剩第四項第二期待拍板（見 Remaining）。

## Completed
- 第一項匯入補強（後端＋前端各一包 codex gpt-5.6-terra 平行實作、主線審過）：
  - `probe_import`（src-tauri/src/import.rs:23）：匯入前探測，任何解析失敗回 Default 不擋匯入。腳本痕跡標籤可並存（`extensions`＝良性四鍵 talkativeness/fav/world/depth_prompt 以外有鍵、`script_tag`＝含 `<script`、`template`＝含 `<%`；ttrpg-rules-system 日後同掛點加骰子標籤）；lorebook_heavy＝條目≥3 且 description+personality+scenario 合計<200 字；alternate_greetings 計數。指令註冊 src-tauri/src/lib.rs:382。
  - 備用開場白：import_character 把 alternate_greetings 依序併入 private_md「### 備用開場白 N」段（import.rs private_markdown）；條目維持單換行緊湊排列（主線修回 codex 規格外的 join("\n\n")）。
  - 前端（src/App.tsx importCharacter 約 3072、importWorldbook 約 1517）：匯卡先 probe——書厚身薄跳原生 confirm 改道（接受→import_worldbook 進當桌＋無角色自動選 GM、不建卡；拒絕→照匯）；匯入成功且有腳本痕跡跳提示。世界書匯入同 probe 同提示。probe invoke 失敗一律當全無、照原流程。
  - i18n 新 6 鍵 ×10 語系（importScriptNotice／worldbookScriptNotice／importLorebookRedirect／importRedirectOk／importRedirectCancel／importRedirectDone）。
- 設計偏差（主線拍板）：任務檔原寫「import_character 回傳多一個旗標」；誤匯改道必須在建卡前詢問，故改獨立 probe_import 指令、import_character 簽名不動。效果同規格：帶腳本跳提示、素卡不跳、改道不留殭屍卡。extensions 判定加良性白名單，否則素 ST 卡（人人帶 talkativeness/fav）全會誤跳。
- 第二項巨集替換（codex gpt-5.6-terra 一包、主線審過）：transport.rs——replace_st_macros（大小寫不敏感手寫掃描、無新依賴）＋player_fallback_name ×10 語系（language_rule 旁）。套用點：assemble_messages 的卡公開/私有＋player 公開＋世界書條目（char=該視角卡名）；assemble_gm_messages 的 world_md 與世界書條目（char 不替換）＋逐卡公開/私有（char=各卡名）＋player 公開。{{random}} 等其餘巨集、transcript 事件、換幕摘要一律原樣。
- 第二項範圍修正（主線拍板）：任務檔寫「前端顯示開場白與卡片文字時同規則」——實查前端唯一顯示面是編輯器（必須顯原文），無唯讀卡片檢視，故第二項純後端。畫面效果由根因解決：模型看到真名就不會照唸巨集；舊 transcript 裡歷史照唸的殘留不回溯改。
- 第三項 Markdown 渲染（codex gpt-5.6-terra 一包、主線親審安全面）：src/story-markdown.ts——marked 實例 breaks+gfm、html renderer 把原始 HTML 轉義成可見文字（blockquote 等 markdown 語法不受影響）、link renderer 降純文字，出口過 DOMPurify（ALLOWED_TAGS 17 個、ALLOWED_ATTR 空陣列，白名單常數 export 供測試共用）。App.tsx StoryText 元件（useMemo）換三個完成訊息顯示點（即時對話/旁白、前幕回看）；串流兩處維持純文字，該則完成才轉渲染。App.css .rendered 樣式區（p/清單/引用/code/pre/標題收斂 1.05em、首尾子元素去 margin）。package.json 加 test script；devDep vitest＋happy-dom（任務檔要求消毒單元測試，原專案無測試跑器——新增依賴理由）。

- 第四項第一期 GM 狀態欄（後端＋前端各一包 codex gpt-5.6-terra 平行、主線審過並補修）：
  - 資料：`TableState { table, characters }`（data.rs:185，characters 佔位給第二期）；`WorldState.state` 當目前值快取、`TranscriptEvent.state: Option<TableState>` 逐則快照。`append_transcript` 事件無快照就借用目前值、自帶就尊重（復原路徑），寫完把該快照同步回快取——不變式：目前值恆等於最後一則事件的快照。`pop_transcript` 重算（本幕往回找→前一幕→default）。`set_last_transcript_state` 供手動改欄位改寫最後一則。
  - 解析：`transport::extract_state_block`（transport.rs:404）認四種包裹——```state/status/状态栏/狀態欄 圍欄、尾端無 info 圍欄、`<details><summary>…状态…`、`<status>`／`<UpdateVariable>`（後者只剝除不解析，JSON patch 歸第二期）。鍵值行容錯：行首 `-*#+>` 剝掉、全形半形冒號皆認、壞行只丟該行；`time`／`place`／`present` 認在地化別名折回英文鍵。標籤比對一律 `to_ascii_lowercase`（主線修：full lowercase 會改變 İ 等字母長度，位移拿回原字串切片會 panic）。
  - 提示詞：`assemble_gm_messages` 多收 `&TableState`，「## 目前狀態」插在登場角色前，只進 GM；`narrate_instruction(lang)` 中英雙版要求 ```state 圍欄三鍵。`gm_narrate` 剝除後回傳顯示文字、欄位合併進快取（主線修：狀態寫檔改盡力而為，IO 失敗不再讓整段旁白連帶丟失）。
  - 前端：`.state-bar`（App.tsx:4114）在 chat-header 與 chat-body 之間、sticky 黏頂；預設收起顯示一行摘要、展開一欄一列，值點擊就地編輯送 `set_table_state`。切桌／旁白完成／收回／復原四個時機刷新。串流中的旁白遇 ```／`<details`／`<status` 就截斷顯示（主線補：否則每回合都看到圍欄閃過）。i18n 新 7 鍵 ×10 語系。
- 第四項設計偏差（主線拍板，理由已寫進 tasks 檔）：狀態存 world.json 不另立檔。
- 第四項狀態列條件顯示（2026-08-03 使用者要求，主線 Opus 5 直寫）：狀態列只給帶狀態列規則的桌，其餘桌整條不掛（看不到也點不開）。`data::world_has_state_bar`（data.rs:805）依序掃 world.md → 世界書啟用條目（content＋title）→ 全部角色卡（public_md＋private_md），任一處命中 `STATE_BAR_MARKERS` 十一個詞（`状态栏`／`狀態欄`／`状态条`／`狀態條`／`状态面板`／`狀態面板`／`status bar`／`statusbar`／`<status`／`<updatevariable`／```` ```state ````，全走 `to_ascii_lowercase`）即為真；比對詞刻意對齊 `extract_state_block` 認得的包裹——認得的才畫得出欄位。指令 `world_has_state_bar`（lib.rs:320）。前端 `hasStateBar`（App.tsx:2806）由一個 effect 在 `[table, mainView, characters]` 變動時重問，`.state-bar` 整段條件掛載（App.tsx:4247）；invoke 失敗當作沒有。三處都掃的理由：卡片可能把狀態列規則放世界書、也可能留在卡片內文（匯入角色卡時 character_book 會併進 private_md）。只提到「狀態」兩字不算（如獸人卡的「猎物状态设定」）。

- 實機驗收修補（2026-08-03，主線 Opus 5 直寫，cargo test 171＋tsc＋check:i18n 70 鈕＋build＋npm test 全綠）：
  - 狀態欄長值溢出：`.state-bar-value` 是按鈕，吃到全域 button 的 nowrap＋置中，長句子衝出面板壓到旁白。App.css:1309/1320 覆寫成 block＋white-space normal＋overflow-wrap anywhere，欄位 align-items 改 start。
  - 隱藏卡進不了編輯器（＝「轉成世界書條目」永遠按不到）：隱藏區每列加「編輯」鈕（App.tsx archive-row，複用 editBtn 鍵）；編輯器那顆鈕依 card.archived 在「隱藏角色／還原」間切換（toggleArchived），convertCardInUse 文案十語系改指向該鈕。
  - 世界書重複匯入：data.rs entry_fingerprint（comment＋content＋key＋keysecondary，trim、關鍵字排序）＋import_worldbook 回傳 `WorldbookImport { imported, skipped }`；世界書面板加「清理重複」鈕→`dedupe_worldbook`（同指紋只留 sorted_entry_keys 最前那條）。i18n 新 5 鍵×10。
  - 開場白貼成 GM 開場旁白：`import::card_opening(bytes)` 直接讀匯入檔的 first_mes（回 `(name, first_mes)`），所以世界書改道那條（不建卡）也拿得到；`transport::resolve_display_macros` 先把 {{user}}/{{char}} 換成實名。指令 `card_opening`，三個入口（世界書面板匯入、角色卡匯入改道、照樣匯成角色卡）都問一句，答好貼 `kind: narration`／speaker GM。**設計拍板**：開場是 GM 的事，不掛角色頭上（第一版曾做成角色發言，使用者當場否掉）。
  - 測卡在 open panel 變灰無法選：檔案本身完好（PNG 結構、UTI public.png、QuickLook 皆正常），複製一份乾淨副本 TestCards/orc-cave-copy.png 即可選；原因未定案，非程式問題。

- 第六項開場白選擇＋起始狀態匯入（後端＋前端各一包 codex 平行、主線審過並補註解）：
  - 後端：`import::card_openings`（import.rs:187）取代 `card_opening`，回 `(卡名, 全部開場白)`＝first_mes＋alternate_greetings 依序、空條略過、一條都沒有回 None。指令 `card_openings`（lib.rs:327）逐條過 `resolve_display_macros`。
  - 貼出走單一指令 `post_opening`（lib.rs:529）：`extract_state_block` 剝除狀態區塊 →`data::append_opening`（data.rs:1509）把欄位併進檯面、事件自帶合併後快照寫入 transcript，回傳事件給前端。前端不碰狀態，不變式（目前值＝最後一則事件快照）由後端一手守住，收回上一句自動倒回。
  - 前端：`offerOpeningLine` 改成拿清單開自製面板（App.tsx 約 4577，比照 gen-table 面板寫法），一條一列、標籤「開場白 N」＋前兩行預覽（120 字截斷、CSS 兩行夾住）；點列＝展開全文（`openingExpanded` 一次只開一條，`.opening-choice-full` max-height 18rem 自帶卷軸），展開後的「貼出這條」才呼叫 `post_opening`→推事件＋`refreshTableState`；overlay／關閉鈕／「先不要」皆＝不貼。i18n 新 2 鍵（openingChoiceTitle／openingChoiceItem）×10，openingLineOk 改當「貼出這條」。
  - 掛點只留世界書路徑（世界書面板匯入、角色卡改道）：匯成角色卡不跳面板——那張卡是要上桌的角色，開場白已在卡上（2026-08-03 使用者拍板）。
  - 卡片自訂欄（例如「沦陷天数」）不必特別處理：狀態欄本來就把基礎三欄以外的鍵一併列出（App.tsx stateFields）。

## Verification
- 第一項主線實跑：`cargo test` 143 passed（139→143：probe 三例＋開場白一例）；`npx tsc --noEmit` ✓；`npm run check:i18n` 十語系 OK（67 鈕）；全部 diff 逐段親讀；ja/ru/en 翻譯抽查自然。
- 第二項主線實跑：`cargo test` 146 passed; 0 failed（143→146：大小寫混用＋fallback 雙語＋GM 逐卡 char 三例）；diff 逐段親讀——替換函式索引只落字元邊界不會 panic、{{random}} 保留、GM 世界書 {{char}} 不誤代。
- 第三項主線驗證：親跑 `npm test`（story-markdown.test.ts 2 塊 10 情境：em/strong/br/blockquote/li/code 白名單通過；`<script>` 無標籤且轉義文字可見、`<img onerror>`／`javascript:` 連結／onclick 全滅）✓、`npx tsc --noEmit` ✓、`npm run build` ✓ 557ms；story-markdown.ts 逐行親審（雙層防線：renderer 轉義＋DOMPurify 白名單，dangerouslySetInnerHTML 只吃 sanitize 出口）。
- 第五項互轉（後端＋前端各一包 codex gpt-5.6-terra 平行、主線審過）：data.rs worldbook_entry_to_character（先驗後動：條目在、標題非空、as_player 時無現任玩家卡→寫卡→補 state.player_card_id→刪條目；後段失敗兩邊都在不遺失）＋character_to_worldbook_entry（archived 才能轉、玩家卡擋；公開＋「## 私有」併一條 constant、order 100、GM 可見、插在清單最上面；先寫條目再刪卡）。lib.rs 兩指令註冊。前端：條目表單頂列「轉成角色卡」（僅既有條目；無玩家卡且內文含 {{user}} 時先問「轉玩家卡？」，粗判只拿來發問、決定權在玩家）；卡編輯器頂列「轉成世界書條目」（isNew／isPlayer 隱藏；未儲存擋、未封存擋、warning 確認）；接線 finishRemoval／refreshCharacters／loadPlayerCard。新 10 鍵 ×10 語系。
- 第四項主線驗證：親跑 `cargo test` 162 passed; 0 failed（151→162：狀態解析七例＋快照借用／不覆寫／pop 回滾／復原放回／GM 專屬隔離）——codex 自驗回報的 5 紅是它沙箱不給開 loopback TCP，主線環境全綠；`npx tsc --noEmit` ✓、`npm run build` ✓、`npm test` ✓、`npm run check:i18n` 十語系 OK（69 鈕）。四份 diff 逐段親讀。專案既有 rustfmt 就有 41 處漂移（含未改動的 cli.rs），本次不跑全域 fmt 免動到範圍外。
- 第六項主線驗證：親跑 `cargo test` 172 passed; 0 failed（171→172：card_openings 清單／PNG 同路徑／只有備用開場白／全空回 None 一例整併，append_opening 併欄位＋快照＋pop 倒回一例）、`npx tsc --noEmit` ✓、`npm run build` ✓、`npm run check:i18n` 十語系 OK（72 鈕）、`npm test` ✓。兩份 diff 逐段親讀；主線補三處註解（post_opening 不變式、offerOpeningLine 拍板理由、import.rs 被插斷的測試註解歸位）。UI 外觀未實跑（Tauri 原生視窗，照慣例交實機驗收）。
- 第五項主線驗證：親跑 cargo test 151 passed（146→151：雙向搬移＋玩家卡 state＋空標題擋＋在桌上擋＋玩家卡擋五例）、tsc ✓、check:i18n 九非正典語系 OK（69 鈕）、npm test ✓、build ✓ 587ms；轉換順序與擋條件逐段親讀。
- 測卡 TestCards/（已 gitignore）三張皆拆內嵌 JSON 驗過規則命中：兽人的洞穴（18 條書厚身薄＋開場白 3＋tavern_helper＋{{user}} 30 處）、根源重塑app（`<script`＋{{user}} 45 處）、勇者养成指南（`<%` ×446、100 條）。
- 狀態列條件顯示主線驗證：親跑 `cargo test` 174 passed; 0 failed（172→174：只提「狀態」不算＋世界書寫出狀態列格式才算、規則寫在卡片私有段也算）、`npx tsc --noEmit` ✓、`npm test` ✓、`npm run check:i18n` 十語系 OK（73 鈕，未新增字串）、`cargo clippy` 新碼零警告。拿使用者實機資料（文件/TableTavern/worlds）逐桌跑判定：性奴世界／史萊姆的獸人世界／新的一桌 1・2・3 顯示，迷霧酒館（含範例）／My Pirate Husbandos／猛獸學園隱藏——與三張測卡（獸人洞穴 `<details><summary>状态栏</summary>`、另兩張 `<UpdateVariable>`＋`<status>`）預期一致。使用者實機驗收通過。
- 第四項狀態欄後續鏈使用者實機驗收（2026-08-04）四條全通過：GM 回合後狀態欄跟著劇情變且對話看不到圍欄、點欄位改字後下一回合 GM 照新值演、收回／復原時狀態同步倒回、模型吐壞格式時遊戲照常不中斷。第一期至此全數結案。

## Remaining / Next action
1. 第四項第二期（自訂數值欄位＋MVU 增量 patch 解析）等第一期實機驗收後細拍；掛點已備：`TableState.characters`、`extract_state_block` 的 UpdateVariable 分支。狀態欄自訂欄與基礎三欄語意重疊（地點／当前环境、在場人物／駐留角色各報一次）＝卡片自身格式造成，使用者 2026-08-03 拍板不列議程。

## Constraints
- 規格與安全紅線見 tasks/st-ecosystem-upgrades.md（XSS 紅線、不做清單、五項互不依賴、小→大順序）。
