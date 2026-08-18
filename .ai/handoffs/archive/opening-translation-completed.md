# Handoff: opening-translation

## Current state
**已結案（2026-08-18）**。開場白翻譯三顆鈕（全部翻譯／翻譯後貼出／重新翻譯）＋檔位挑選器全數完成，
實機驗收 T1–T7 全過，過程抓到的兩個洞當場修掉。自驗 cargo 501／vitest 137／build／check:i18n 十語系全綠。

## Completed
- 後端：`src-tauri/src/translate.rs`（`opening_messages` 純函式＋防注入聲明）；lib.rs `translate_opening`
  command（fast 檔優先、API 模式未設 fast 退 `gm_tier`；2026-08-18 加 `tier` 參數，未知值 fail-closed）；
  `translate_tier_models` command＋`transport::tier_model`（三檔各自實際會叫的模型，解析與 `stream_via_transport`
  同源——同樣是「低」檔，設了 `claude:fast` 的機器跑 claude-haiku-4-5，沒設的跑別名 haiku）。
- 前端（App.tsx／useImportController／ImportDialogs）：逐則翻譯狀態＋abort ref（modal 一關即中止後續呼叫）；
  「✨ 全部翻譯」序列翻譯；展開後「✨ 翻譯後貼出」與「貼出這條」；2026-08-18 加視窗頂部翻譯工具列
  （`翻譯模型 [低 · claude-haiku-4-5 ▾]`，檔位只影響本次視窗、批次途中鎖住）與展開層「重新翻譯」。
- 譯文與原文分離：`openings` 保原文（UI 不露出）、`translations[index]` 存 AI 回覆。重翻永遠拿原文當輸入，
  送出期間畫面留舊譯文、失敗不清掉。玩家看到的一律是譯文（拍板：看不懂原文的玩家留著原文沒意義）。
- i18n：十語系各 +9 鍵（translateAll／PostBtn／Hint／AllProgress／Translating／Failed／Tier／Retranslate／TierCliDefault）。

## Verification（2026-08-18 實機驗收，T1–T7 全過）
- T1 匯入多開場白他語卡 → 選擇視窗標題正下方出現「✨ 全部翻譯」。（11 則英文卡）
- T2 全部翻譯 → 鈕變「翻譯中 x/n」、逐則「翻譯中…」、譯文就地替換；跑完鈕回復。
- T3 全翻進行中關窗 → 不再發新呼叫（`prompt-cache.jsonl` 停止增長）。在途那一則會跑完並計費，與
  交接檔「中止後續呼叫」的設計一致。
- T4 「翻譯後貼出」→ 譯文貼成旁白；「復原上次匯入」把角色卡、世界書條目、譯文旁白、狀態樹一起收掉。
- T5 已翻好再按「翻譯後貼出」→ 2 秒內貼出，用量檔零新增（不重打）。
- T6 翻譯失敗（切無 key 的 API 直連製造）→ 該則標 ⚠、原文未動、「貼出這條」照常貼出原文（逐字稿核對）。
- T7 版面：`.opening-trans-status` 被 `.opening-choice-head span` 的兩行截斷規則套到——「翻譯中…」折成兩行、
  ⚠ 壓成一撇。已修（commit 7c1f33b），新 release 包複驗兩態正常。
- 追加驗收（檔位與重新翻譯，commit e5db525）：三檔顯示實際 id（低 haiku-4-5／中 sonnet-4-6／高 opus-4-7）；
  切中檔按「重新翻譯」後用量檔證實模型從 claude-haiku-4-5 換成 claude-sonnet-4-6；重翻期間舊譯文保留、兩顆 AI 鈕停用。

## 已知限制（2026-08-18 拍板接受）
1. 模型回拒絕語（而非譯文）時，app 無法可靠分辨，那段會被當譯文顯示、也能被貼出。**刻意不做關鍵詞偵測**
   （會誤傷正常譯文，也擋不住離題／截斷／翻錯語言）。玩家看到不對，自己調高檔位重翻。
2. 檔位挑選器只列三檔、只在同一個連線方式內換。不做「列出該 CLI 全部模型」（API 模式 413 項無法下拉，
   且同一家換世代已足夠——實測 Opus 5 拒的內容 4.7 可過），也不做跨供應商選擇（等於要把 API key 一起做成可選項）。
3. 翻譯用低檔位這件事不寫說明句：檔位就長在翻譯鈕旁邊，玩家自己看得到（位置優先於說明）。
4. 「全部翻譯」進行中對同一則按「翻譯後貼出」可能重複翻一次（閉包快照下 done 判定滯後；只多花一則額度）。
5. CLI 檔位玩家 fast 一律有預設對應；只有 API 直連玩家需在 tier_models 設 fast，未設走 GM 檔。
6. 「批次中選擇器變淡」那條 CSS 未實機複驗（純 opacity，功能不受影響）。

## Constraints
- 翻譯呼叫必須玩家主動按鈕觸發，不自動跑（不替玩家花錢紅線）。
- 開場白內容永遠當資料、永不執行；防注入聲明是紅線，改提示詞不得移除。
- 檔位切換只影響當次視窗，不寫回全域設定。
