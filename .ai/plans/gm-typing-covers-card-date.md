# gm-typing-covers-card-date — 做法（三方共識：第 1 輪送審同意、實作驗收 Sol／Grok DONE，2026-10-27 結案）

## 重現
測試通道＋本機假 GM 端點（OpenAI 相容 SSE，先停 20 秒再吐字），NorthHall 卡匯入成世界書、介面開著送出一句：`.card-interface-status` 以 `position:absolute; top/left:0.6rem` 浮在 iframe 左上，蓋住卡片頂列。一般聊天的打字提示是訊息列最後一則的 dots，位置正常。

## 參考
- SillyTavern（官方擴充 Extension-TypingIndicator）：元素 `appendChild` 進 `#chat`，`position: sticky; bottom: 10px; order: 9999`＝聊天捲動區最底、送出列正上方；素文字「{{char}} is typing...」＋dots；預設開串流時不顯示（串流文字本身就是提示）。核心另把送出鈕換成停止鈕。
- Marinara Engine：一般聊天把「X is typing…」小字斜體＋dots 放在訊息列末尾（＝輸入框正上方），首 token 到就清掉；氣泡模式改在待生成的那則氣泡裡放 dots。Game 模式沒有浮動 badge：生成中把輸入框 placeholder 換成「準備下一步」，旁白面板內用 spinner＋階段文字。
- 共同點：提示貼著輸入區、佔版面不浮蓋內容；沒有放在內容頂端的。我們一般聊天已同位置，差距只在介面覆蓋層。

## 做法
1. 落點：狀態列從 iframe 上方的絕對定位移進 `form.card-interface-composer`，排在輸入列（`.card-interface-composer-row`）正上方，與換幕容量提示、錯誤訊息同一個「輸入列上方」槽位（順序：容量提示 → 錯誤 → 打字提示 → 輸入列）〔作者裁決 2026-10-27：放輸入框正上方；順序三方共識〕；進 form 而不是改成 `bottom:` 浮動，是因為浮在底部一樣會蓋卡片底部內容（行動選項多在底部）〔模型判斷·未裁決〕。
2. 版面：生成開始時輸入列長高一行、iframe 縮一行，結束放回；與容量提示、錯誤訊息現行行為一致，ST／Marinara 的提示也都佔版面〔模型判斷·未裁決〕。三方共識：不預留空行。
3. 外觀：拿掉藥丸框（邊框、底色、圓角是為了浮在未知卡底色上），改成輸入列底色上的一行小字（`--fs-meta`、`--ink-3`）＋既有 `.typing` dots；保留 `role="status"` 與名字（覆蓋層看不到名牌，名字不能省）；名字單行截字（ellipsis），dots 不縮〔三方共識〕。
4. 顯示時長不變：整段生成都顯示。覆蓋層看不到逐字串流，不照 ST「串流時隱藏」、Marinara「首 token 清掉」〔模型判斷·未裁決〕。
5. 一般聊天不動：dots 已在訊息列底部、輸入框上方，與兩家同位置；驗收方向也要求無介面桌不變〔模型判斷·未裁決〕。
6. 網頁版不用 `CardInterfaceOverlay`、沒有打字提示，出範圍。
7. 測試：`CardInterfaceOverlay.test.tsx` 加一案——同一掛載切 null→GM→null，提示在 form 內、輸入列正前、名字與 status 語意正確，結束消失、form 與輸入列是同一個元素。
8. 結構：`CardInterfaceOverlay.tsx` 228 行、`card-interface.css` 150 行、資料夾 18 檔，不搬；CSS 留在 `card-interface.css`。

## 驗收
介面桌生成中，提示不遮卡片任何內容、顯示在輸入列正上方，生成結束消失；無介面桌提示位置不變；verify 綠；測試通道截圖前後對照。

## 實測（測試通道＋`scripts/harness-fake-openrouter.mjs`，WestFantsy 卡，測試 root 無金鑰）
- 生成中提示在輸入列正上方，卡片區由 526.75 縮到 506.16px，結束放回；卡片頂列不再被蓋。
- 卡片捲到底（測試時在 srcdoc 尾端注入捲到底腳本）：生成開始／結束底部「踏入命运」鈕都看得到。
- 證據範圍（替代驗證，三方同意可接受）：小視窗以根元素 `zoom:1.6` 模擬約 500px 寬，只算排版壓力測試、不算真 500px 視窗（通道 `plugin:window|set_size` 被 ACL 擋）；長名字是直接改提示文字，只驗 CSS 截字（名字截成「…」、dots 留在畫面內），真角色長名未實跑；捲到底靠 srcdoc 注入腳本，不延伸到使用者捲動操作。
