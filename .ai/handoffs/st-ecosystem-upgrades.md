# Handoff: st-ecosystem-upgrades

## Current state
2026-08-02 目標模式進行中（範圍：一→二→三→五；第四項到「狀態區塊格式」拍板閘門停）。第一、二項完成、主線驗收全綠，等使用者實機驗收；接著開第三項 Markdown 渲染（依賴已裝好）。

## Completed
- 第一項匯入補強（後端＋前端各一包 codex gpt-5.6-terra 平行實作、主線審過）：
  - `probe_import`（src-tauri/src/import.rs:23）：匯入前探測，任何解析失敗回 Default 不擋匯入。腳本痕跡標籤可並存（`extensions`＝良性四鍵 talkativeness/fav/world/depth_prompt 以外有鍵、`script_tag`＝含 `<script`、`template`＝含 `<%`；ttrpg-rules-system 日後同掛點加骰子標籤）；lorebook_heavy＝條目≥3 且 description+personality+scenario 合計<200 字；alternate_greetings 計數。指令註冊 src-tauri/src/lib.rs:382。
  - 備用開場白：import_character 把 alternate_greetings 依序併入 private_md「### 備用開場白 N」段（import.rs private_markdown）；條目維持單換行緊湊排列（主線修回 codex 規格外的 join("\n\n")）。
  - 前端（src/App.tsx importCharacter 約 3072、importWorldbook 約 1517）：匯卡先 probe——書厚身薄跳原生 confirm 改道（接受→import_worldbook 進當桌＋無角色自動選 GM、不建卡；拒絕→照匯）；匯入成功且有腳本痕跡跳提示。世界書匯入同 probe 同提示。probe invoke 失敗一律當全無、照原流程。
  - i18n 新 6 鍵 ×10 語系（importScriptNotice／worldbookScriptNotice／importLorebookRedirect／importRedirectOk／importRedirectCancel／importRedirectDone）。
- 設計偏差（主線拍板）：任務檔原寫「import_character 回傳多一個旗標」；誤匯改道必須在建卡前詢問，故改獨立 probe_import 指令、import_character 簽名不動。效果同規格：帶腳本跳提示、素卡不跳、改道不留殭屍卡。extensions 判定加良性白名單，否則素 ST 卡（人人帶 talkativeness/fav）全會誤跳。
- 第二項巨集替換（codex gpt-5.6-terra 一包、主線審過）：transport.rs——replace_st_macros（大小寫不敏感手寫掃描、無新依賴）＋player_fallback_name ×10 語系（language_rule 旁）。套用點：assemble_messages 的卡公開/私有＋player 公開＋世界書條目（char=該視角卡名）；assemble_gm_messages 的 world_md 與世界書條目（char 不替換）＋逐卡公開/私有（char=各卡名）＋player 公開。{{random}} 等其餘巨集、transcript 事件、換幕摘要一律原樣。
- 第二項範圍修正（主線拍板）：任務檔寫「前端顯示開場白與卡片文字時同規則」——實查前端唯一顯示面是編輯器（必須顯原文），無唯讀卡片檢視，故第二項純後端。畫面效果由根因解決：模型看到真名就不會照唸巨集；舊 transcript 裡歷史照唸的殘留不回溯改。

## Verification
- 第一項主線實跑：`cargo test` 143 passed（139→143：probe 三例＋開場白一例）；`npx tsc --noEmit` ✓；`npm run check:i18n` 十語系 OK（67 鈕）；全部 diff 逐段親讀；ja/ru/en 翻譯抽查自然。
- 第二項主線實跑：`cargo test` 146 passed; 0 failed（143→146：大小寫混用＋fallback 雙語＋GM 逐卡 char 三例）；diff 逐段親讀——替換函式索引只落字元邊界不會 panic、{{random}} 保留、GM 世界書 {{char}} 不誤代。
- 測卡 TestCards/（已 gitignore）三張皆拆內嵌 JSON 驗過規則命中：兽人的洞穴（18 條書厚身薄＋開場白 3＋tavern_helper＋{{user}} 30 處）、根源重塑app（`<script`＋{{user}} 45 處）、勇者养成指南（`<%` ×446、100 條）。

## Remaining / Next action
1. 第三項 Markdown 渲染：依賴已裝（marked ^18、dompurify ^3.4；vitest＋happy-dom devDep 供任務檔要求的消毒單元測試）；交辦規格已定稿——story-markdown.ts 渲染函式（marked html renderer 轉義原始 HTML→「以文字顯示」、DOMPurify 白名單 ALLOWED_ATTR 空陣列）＋三顯示點換 StoryText 元件＋串流兩處不動＋CSS .rendered 區。白名單與消毒測試主線親審（安全紅線）。
2. 第五項互轉可插隊 → 第四項開工前停：讀 undo-last-message 資料流＋出狀態區塊格式拍板題。
3. 使用者實機驗收（第一項）：三張測卡匯入各跳改道詢問；接受→條目進當桌世界書；拒絕→照建卡＋腳本提示；兽人的洞穴卡私有筆記見備用開場白 1–3；素卡（App 匯出的）不跳提示。（第二項）帶 {{user}} 的卡開聊，提示詞與模型回覆都用玩家名。

## Constraints
- 規格與安全紅線見 tasks/st-ecosystem-upgrades.md（XSS 紅線、不做清單、五項互不依賴、小→大順序）。
