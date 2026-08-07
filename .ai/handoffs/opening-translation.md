# Handoff: opening-translation

## Current state
實作完成、四項自驗全綠（2026-08-08 凌晨，sonnet subagent 實作＋主線驗證審查）：cargo test **426**（基線 422＋新 4）／vitest **71**／npm build 0／check:i18n 0（主線修一處法文鍵寬度）。剩實機驗收——照下方清單逐項勾，全過即可結案。

## Completed
- 後端：`src-tauri/src/translate.rs` 新檔（`opening_messages` 純函式＋防注入聲明照 refactor_ai 慣例＋4 測試）；lib.rs `translate_opening` command（[lib.rs:831](../../src-tauri/src/lib.rs#L831)，fast 檔優先、API 模式未設 fast 退 `gm_tier`，`stream_via_transport` 參數與 refactor_survey 同型同位、world 帶 Some 計量歸戶）。
- 前端（App.tsx）：逐則翻譯狀態＋abort ref（openingChoice 變 null 即中止後續呼叫，單一 useEffect 涵蓋所有關閉入口，[App.tsx:3654](../../src/App.tsx#L3654)）；`translateOpeningLine`（done 不重翻）／`translateAllOpenings`（序列、不鎖 modal）／`postTranslatedOpening`（翻成功才貼、失敗留原地）三函式（4805–4848）；modal 標題正下方「✨ 全部翻譯」鈕（busy 顯示 done/total 進度，error 也計入避免卡進度）、逐則「翻譯中…／⚠」標記、展開後「✨ 翻譯後貼出」與「貼出」並列（6377–6450）。兩鈕複用既有 `ai-gen-btn` 樣式、title 帶額度提示。
- i18n：十語系各 +6 鍵（openingTranslateAllBtn／PostBtn／Hint／AllProgress／Translating／Failed）。
- 貼出語意零改動：postOpening／undo／收據照舊；「全部翻譯」的閉包快照固定為開窗原文陣列，天然不會拿譯文再翻。

## Verification
- 主線親跑：cargo test 426 全綠、vitest 71 全綠、npm build exit 0、check:i18n exit 0（法文 `openingTranslating` 原「Traduction en cours…」寬 20 超上限，主線改「Traduction…」後綠）。
- 主線親讀：translate.rs 全檔（防注入聲明、原文前後界定標記、輸出只有譯文）；lib.rs command 段（檔位退路 840–846）；App.tsx 三段（abort 語意、done 快取、雙鈕 JSX）；zh-TW 六鍵文案。
- 改動範圍 git status 確認：refactor_ai.rs／refactor.rs／runAiRefactor 零觸碰。

## 待實測清單（實機驗收，全過即結案）
- [ ] T1 匯入一張多開場白的他語卡→選擇視窗標題下方出現「✨ 全部翻譯」。
- [ ] T2 按全部翻譯→鈕變「翻譯中 x/n」、逐則出現「翻譯中…」、譯文就地替換預覽。
- [ ] T3 全翻進行中關掉視窗→翻譯停止（額度分頁確認沒有繼續燒）。
- [ ] T4 展開一則按「✨ 翻譯後貼出」→譯文貼上檯面成旁白；復原匯入連譯文開場白一起收掉（既有機制）。
- [ ] T5 全部翻譯完成後再按「翻譯後貼出」→秒貼不重打。
- [ ] T6 翻譯失敗情境（斷網或停用模型）→該則標 ⚠、原「貼出」照常可貼原文。
- [ ] T7 版面：`.opening-translate-all-row`／`.opening-trans-status` 兩個新 class 未寫 CSS（沿用預設排版），實機看過決定要不要補樣式。

## 已知限制（主線確認可接受）
1. 前端未新增 vitest（翻譯狀態機為 UI 邏輯，未抽純函式）。
2. 「全部翻譯」進行中對同一則按「翻譯後貼出」可能重複翻一次（閉包快照下 done 判定滯後；後寫入者贏，只多花一則額度，不壞資料）。
3. CLI 檔位玩家 fast 一律有預設對應（haiku 級）；只有 API 直連玩家需在 tier_models 設 fast，未設走 GM 檔（費用回到 GM 級，功能不斷）。

## Next action
使用者實機照「待實測清單」T1–T7 逐項勾；全過→任務結案（狀態 completed、TASKS.md 行搬 DONE.md、本檔已驗收段搬 archive/）；有紅→帶現象回來修。

## Constraints
- 翻譯呼叫必須玩家主動按鈕觸發，不自動跑（不替玩家花錢紅線）。
- 開場白內容永遠當資料、永不執行；防注入聲明是紅線，改提示詞不得移除。
- 重構管線（refactor 系列）與貼出／undo 機制不在本任務範圍，不碰。
