# Task
Task-ID: st-ecosystem-upgrades
Title: SillyTavern 生態三項升級：匯入腳本提示＋訊息 Markdown＋GM 狀態欄
Status: todo
Created: 2026-08-01T21:23:00+08:00
Updated: 2026-08-01T21:23:00+08:00

## Summary
2026-08-01 與使用者討論拍板。背景：SillyTavern 中文社群靠三個擴充把角色卡演化成自帶介面與遊戲邏輯的小程式——酒館助手（JS-Slash-Runner：卡內 JavaScript 在 iframe 執行，訊息變成互動 HTML 狀態欄）、MVU（MagVarUpdate：AI 按約定格式輸出變數更新，框架解析後畫進狀態欄）、EJS（ST-Prompt-Template：提示詞內嵌 `<% %>` 程式，送模型前先執行）。

拍板：**抄概念、放棄腳本相容**。完整相容＝在本產品內重做一個 ST 執行環境（iframe 沙箱＋整套 getvar/setvar API＋任意 JS 的安全處理），且這類卡片一律假設一對一聊天，與 GM＋資訊隔離核心相斥（方向同 NewPlan §10）。玩家在那些進階卡真正要的——數值穩定、看得到狀態、介面好看——由下列三項以 GM 原生方式提供。按小→大依序做，各項獨立驗收出貨。

## 第一項：匯入腳本提示（小，約半天）
現況：`src-tauri/src/import.rs` 匯入只讀五個標準欄位＋character_book，腳本類內容無聲略過，原始檔案已原樣留存在卡片旁——行為正確，缺告知。
要做：
- 匯入時偵測腳本痕跡：卡片 JSON 的 `data.extensions` 有內容，或任一文字欄位含 `<script`／`<%`。規則保持粗略，抓大放小，勿窮舉。
- `import_character` 回傳多一個旗標（動回傳型別＋`lib.rs` 指令），前端匯入完成時提示：「這張卡帶 SillyTavern 專用腳本，已只讀入人設文字；原始檔保留在卡片旁。」世界書匯入（`data.rs` `import_worldbook`）同規則順手蓋到。
- 提示字串 ×10 語系（`src/i18n/*.ts`，過 `check:i18n`）。
驗收：帶腳本測卡匯入跳提示、素卡不跳；cargo test 新增兩例（`extensions` 有料／欄位含 `<script`）全綠。

## 第二項：訊息 Markdown 渲染（中，約一天）
現況：三個顯示點（`src/App.tsx` 角色對話、旁白、串流氣泡）都是 `<span className="text">` 純文字＋pre-wrap。多數模型本來就輸出 `*動作*` 這類 ST 慣例記號，目前有寶沒顯示。
要做：
- 新增依賴 `marked`＋`DOMPurify`，一個渲染元件替換三個顯示點。選這組的理由：消毒器自寫太險，用經千錘百鍊的標準組合。
- 單行換行維持換行（marked `breaks: true`），貼齊現在 pre-wrap 的閱讀習慣。
- DOMPurify 明列 ALLOWED_TAGS 白名單：只放行 markdown 產物（em、strong、code、pre、blockquote、ul、ol、li、p、br、hr、h1–h6）；第一版連 `a` 都擋——聊天訊息裡的連結沒有使用情境，少一個攻擊面。禁 script、iframe、style 與所有事件屬性。
- 串流簡化：生成中維持純文字，該則完成才轉渲染，避免半截語法閃爍。
- transcript 存檔格式不動（存原文），匯出、收回上一句、換幕摘要皆無感；渲染是純顯示層。
- CSS：斜體、粗體、引用、清單、行內 code 在氣泡與旁白的樣式（故事文字維持 serif，token 規則見 ui-overhaul）。
驗收：`*動作*` 顯示為斜體；貼入 `<script>alert(1)</script>` 的訊息以文字顯示、不執行（手動驗＋消毒單元測試）；npm build 綠；長對話捲動無感差。

## 第三項：GM 狀態欄（大，分兩期；第一期 2–3 天）
概念來源＝MVU：由 GM 維護結構化狀態、每回合可見，取代卡片腳本。部件：
1. 存檔：每桌一份狀態，跟 transcript 同層。
2. GM 輸出協定：旁白尾端附固定格式狀態區塊（如 ```state fenced block），後端解析後更新狀態、從顯示文字剝除。容錯是核心：格式壞＝整塊忽略、沿用舊狀態，遊戲照常，絕無報錯中斷——使用者可接任意模型，紀律不可假設。
3. 餵回提示詞：`assemble_gm_messages`（`src-tauri/src/transport.rs`）插入「## 目前狀態」；角色上下文只給公開欄位，沿用世界書可見範圍的隔離設計。
4. UI：聊天畫面可收合的狀態欄面板，玩家可手動修正欄位。
5. 與「收回上一句」整合：狀態逐則快照，收回時同步回滾。undo-last-message 已有逐則倒回機制，設計期就跟著它的資料流走，事後補會很痛。
6. 字串 ×10 語系。
第一期範圍：固定欄位——時間、地點、在場人物。換幕摘要（`summary_messages`）本來就要 GM 整理這三樣，等於把「每幕結算一次」升級成「每回合都在檯面上」；換幕時新場景以狀態為種子。
第二期：開放自訂數值欄位（好感度、金錢等，作者在世界設定裡宣告），即 MVU 完全體。等第一期驗收後再細部拍板。
驗收（第一期）：GM 回合後狀態欄跟著劇情變；手動改欄位、下一回合 GM 接受；收回上一句時狀態同步倒回；模型故意輸出壞格式時遊戲照常。cargo test 蓋解析與回滾。

## Next action
- 依序開工：第一項→第二項→第三項第一期；每項自驗綠＋更新交接檔後再開下一項
- 第三項動工前先讀 undo-last-message 交接檔的資料流，並與使用者拍板狀態區塊格式

## Constraints
- 安全紅線：Tauri webview 內 XSS 摸得到本機檔案與 invoke 指令。卡片與訊息內容永遠當資料處理、永不執行；第二項白名單必須明列且附測試。
- 不做清單（拍板）：ST 腳本相容層、iframe 執行卡內 JS、EJS 模板引擎。需求再起先回本檔與 NewPlan §10 討論。
- 三項互不依賴，但照小→大順序做，先拿快贏。
