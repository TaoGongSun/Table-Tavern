# Handoff: st-ecosystem-upgrades

## Current state
2026-08-02 目標模式本輪結束：一、二、三、五項全部實作完成、主線驗收全綠，等使用者實機驗收。第四項照拍板停在閘門——狀態區塊格式拍板題已出給使用者（undo 資料流已讀：pop_transcript 整檔重寫＋前端復原疊、appendEvent 清疊）。

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

## Verification
- 第一項主線實跑：`cargo test` 143 passed（139→143：probe 三例＋開場白一例）；`npx tsc --noEmit` ✓；`npm run check:i18n` 十語系 OK（67 鈕）；全部 diff 逐段親讀；ja/ru/en 翻譯抽查自然。
- 第二項主線實跑：`cargo test` 146 passed; 0 failed（143→146：大小寫混用＋fallback 雙語＋GM 逐卡 char 三例）；diff 逐段親讀——替換函式索引只落字元邊界不會 panic、{{random}} 保留、GM 世界書 {{char}} 不誤代。
- 第三項主線驗證：親跑 `npm test`（story-markdown.test.ts 2 塊 10 情境：em/strong/br/blockquote/li/code 白名單通過；`<script>` 無標籤且轉義文字可見、`<img onerror>`／`javascript:` 連結／onclick 全滅）✓、`npx tsc --noEmit` ✓、`npm run build` ✓ 557ms；story-markdown.ts 逐行親審（雙層防線：renderer 轉義＋DOMPurify 白名單，dangerouslySetInnerHTML 只吃 sanitize 出口）。
- 第五項互轉（後端＋前端各一包 codex gpt-5.6-terra 平行、主線審過）：data.rs worldbook_entry_to_character（先驗後動：條目在、標題非空、as_player 時無現任玩家卡→寫卡→補 state.player_card_id→刪條目；後段失敗兩邊都在不遺失）＋character_to_worldbook_entry（archived 才能轉、玩家卡擋；公開＋「## 私有」併一條 constant、order 100、GM 可見、插在清單最上面；先寫條目再刪卡）。lib.rs 兩指令註冊。前端：條目表單頂列「轉成角色卡」（僅既有條目；無玩家卡且內文含 {{user}} 時先問「轉玩家卡？」，粗判只拿來發問、決定權在玩家）；卡編輯器頂列「轉成世界書條目」（isNew／isPlayer 隱藏；未儲存擋、未封存擋、warning 確認）；接線 finishRemoval／refreshCharacters／loadPlayerCard。新 10 鍵 ×10 語系。
- 第五項主線驗證：親跑 cargo test 151 passed（146→151：雙向搬移＋玩家卡 state＋空標題擋＋在桌上擋＋玩家卡擋五例）、tsc ✓、check:i18n 九非正典語系 OK（69 鈕）、npm test ✓、build ✓ 587ms；轉換順序與擋條件逐段親讀。
- 測卡 TestCards/（已 gitignore）三張皆拆內嵌 JSON 驗過規則命中：兽人的洞穴（18 條書厚身薄＋開場白 3＋tavern_helper＋{{user}} 30 處）、根源重塑app（`<script`＋{{user}} 45 處）、勇者养成指南（`<%` ×446、100 條）。

## Remaining / Next action
1. 開第四項第一期（三題已於 2026-08-02 拍板：```state 圍欄鍵值行協定／快照藏旁白事件裡（狀態檔＝目前值快取、手動改欄位補隱形狀態事件）／四種包裹全認，結論已併入任務檔第四項各點）。動工時照 undo 資料流：pop_transcript 整檔重寫即自動回滾；appendEvent 會清前端復原疊。
2. 使用者實機驗收：（第一項）三張測卡匯入各跳改道詢問；接受→條目進當桌世界書；拒絕→照建卡＋腳本提示；兽人的洞穴卡私有筆記見備用開場白 1–3；素卡不跳提示。（第二項）帶 {{user}} 的卡開聊，提示詞與模型回覆都用玩家名。（第三項）模型輸出 `*動作*` 顯示斜體；貼 `<script>alert(1)</script>` 進對話以文字顯示不執行；前幕回看對話列改「名字在上、內文在下」與即時聊天一致（版面小變化，順帶驗收）。（第五項）世界書條目編輯表單一鍵轉角色卡（含 {{user}} 人設條目會問要不要當玩家卡）、轉出的條目原地消失；封存卡編輯畫面一鍵轉條目（在桌上／未儲存會被擋）、轉出條目排世界書最上面。

## Constraints
- 規格與安全紅線見 tasks/st-ecosystem-upgrades.md（XSS 紅線、不做清單、五項互不依賴、小→大順序）。
