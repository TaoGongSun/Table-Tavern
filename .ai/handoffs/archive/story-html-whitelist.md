# story-html-whitelist — 訊息顯示照酒館渲染白名單 HTML

Status: 已結案（2026-10-28，進 main；Sol、Grok 第 6 輪驗收 DONE）。後續另案：story-html-style-attr、story-html-style-class、desktop-csp（皆在 BACKLOG）

## Summary
`src/shared/ui/story-markdown.ts` 的 marked `html` renderer 把所有原始 HTML 一律跳脫成可見文字。桌面 StoryText（聊天、單幕閱讀、開場白選擇框）與網頁版 `renderHostMarkdown`（聊天串流、卡片匯入預覽、卡片介面訊息段）共用這一支。

## 查證結論（2026-10-28）
- ST 1.19.0（06bde939）：showdown 2.1 放行原始 HTML，`messageFormatting` 以 DOMPurify **預設全清單**＋`custom-style` 消毒；`chats.js addDOMPurifyHooks`：class 加 `custom-` 前綴、未知元素內換行轉 `<br>`、外部媒體預設擋（`forbid_external_media: true`）；`<style>` 經 encode/decode 加 `.mes_text` 前綴。`encode_tags` 預設 false。
- 中文標籤（`<核心记忆>`、`<开局>`）在 HTML 解析器下不是標籤：ST 實際是開頭標籤露字、結尾標籤變 bogus comment 消失。「不露字」對中文標籤是比 ST 好，不是照 ST。
- marked html block（標籤獨佔一行起頭）不吃 `breaks`，多行未知標籤內容會壓成一行；ST 靠上述 hook 換 `<br>`。
- 桌面 `tauri.conf.json` `csp: null`，DOMPurify 是唯一一層；網頁版宿主 CSP `style-src 'self'`、`img-src 'self' data:`、`form-action 'none'`。
- Marinara（e5ead06）：自訂約 80 標籤允許清單，禁 svg/math/iframe/form 類，style 經 postcss 消毒（fixed→absolute、url 限縮），`<style>` 限縮在訊息框；無中文標籤處理。

## 定案（Sol、Grok 審查共識，主線裁決 2026-10-28）
採「擴充白名單」：專用 DOMPurify 實例、整份指定標籤與屬性（不放 a、表單、svg/math、影音、class/id/style、data-*/aria-*）；原始 HTML 的 img 由 hook 照 Markdown 圖片同一道來源門，拒絕時退回替代文字；未閉合註解與會吞後文的元素（script、style、textarea、svg 等）沒結尾就跳脫成字；不認得元素裡換行補 `<br>`；中文標籤成對的兩端拿掉（比 ST 好）。style 屬性、`<style>`＋class、桌面補 CSP 由主線另立案。

## 實作結果
- `src/shared/ui/story-markdown.ts`：marked、白名單、專用 DOMPurify 實例（FORBID_CONTENTS 只留 script、style 等不該給玩家看的，svg／math／影音剝標籤留字）、消毒後惰性 DOM 收尾（圖片來源、中文成對標籤）。
- `story-html-scan.ts`：進消毒器前中文結尾標記改字面；未收尾吞文結構用哨兵法（每次呼叫隨機產生，輸入冒充不了）交給瀏覽器解析器判斷，在 DOM 上拆開，不自己模仿解析規格。
- 網頁版 `host-markdown.ts`：同源判斷走消毒後 DOM。桌面 `workspace.css`／網頁 `App.css`：details 與表格樣式。
- 六輪驗收的 Sol 重現、安全審查的圖片 regex、實體編碼偽哨兵、外來元素與 noscript 結構探針都入固定回歸（jsdom＋WebKit）。
- 已知限制：中文標籤與未收尾的吞文元素，串流中結尾未到前開頭會短暫露字；一則訊息要修的未收尾結構超過 30 個時，從第一個 `<` 起整段改成字。極端巢狀（例：6,000 層未收尾 template、約 60 KB）WebKit 單次渲染約 1.8 秒，不另設防線。

## 實機（測試通道，2026-10-28，第 4 輪修正後的建置）
範例桌追加一則 GM 旁白：粗體、刪除線、details 收合／展開、表格框線、多行中文標籤（含行內碼）兩端不露字且分行、多行 status 分行、屬性含中文標籤不被改、script 內含 `<script>` 字串後文照顯示、`<svg><svg a=x/></svg>` 後文照顯示、未閉合註解後文照顯示，皆正常。

## 驗收方向
常見白名單標籤照 ST 渲染、自訂標籤不露字也不丟內容；不放寬腳本與事件屬性等安全面。
