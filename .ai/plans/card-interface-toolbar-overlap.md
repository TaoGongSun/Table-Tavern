# card-interface-toolbar-overlap — 做法（三方共識：第 1 輪送審 Sol、Grok DONE，40px 常駐可接受、不壓薄；實作驗收 Sol、Grok DONE、主線重跑 verify 綠，2026-10-27 結案）

## 重現
測試通道（`harness:build`，獨立 root，零 AI）匯入 NorthHall 成世界書、開卡片介面：`.card-interface-toolbar` 以 `position:absolute; top/right:0.6rem; z-index:1` 浮在 iframe 上（800×約 568pt 視窗，工具列 682–790 × 9.6–39.6）。卡片頂列右側被「換幕」鈕壓住，關閉鈕（淺色 X）落在卡片粉色底上幾乎看不見，卡片自己的右上設定鈕被擠到工具列正下方。WestFantsy 頂部右側沒有卡片按鈕，但關閉鈕同樣疊在 iframe 捲軸上。證據：`scratchpad/shots/02-northhall-before.png`（暫存檔）。

## 參考
- SillyTavern：app 控制項在自己的 `#top-bar`（固定一列、聊天區在它下面）與底部送出列，沒有浮在聊天內容上的控制項；卡片 regex 產生的 HTML 只活在訊息氣泡內。
- Marinara Engine：沒有卡片自帶 HTML 介面（唯一的 iframe 是多人旁觀畫面，鋪滿容器、無疊控制項）。遊戲模式的右上控制項是浮在自家場景之上（`absolute right-3 top-3`），但內容是自己畫的、自己讓位，與「別人寫的卡片」不同型。
- 共同點：控制項若疊在內容上，只在內容由 app 自己掌控時才成立；內容來自第三方時，控制項佔自己的一列。

## 做法
1. 落點：工具列從絕對定位改成覆蓋層 flex 欄的第一列（frames 正上方），`flex:none`，底色 `--surface-1`、下緣 `1px solid var(--line)`，與底部輸入列對稱。與 gm-typing-covers-card-date 一致：不浮在卡片上，改佔版面。
2. 高度隨內容自然計算（按鈕 `--control-h`＋上下 padding 0.3rem，約 40px），不硬鎖數字；iframe 少約 40px，常駐可接受〔三方共識〕。
3. 版面：`justify-content:flex-end`；`.card-interface-tabs` 加 `margin-right:auto; min-width:0` 靠左、窄時只有它縮（既有 btn-shrink 截字）；右側換幕與關閉明訂 `flex:none` 不被擠窄。單卡時左側留空，不加標題或說明字。
4. 判斷範圍：「第三方卡片內容一律不疊控制項、獨立一列」是本案對卡片介面覆蓋層的判斷，不寫成全 app 通則。
5. 沒選：移進底部輸入列（與送出鈕誤觸、窄窗塞不下分頁；作者原則按鈕置頂）；半透明／自動隱藏／可拖動（本質上仍蓋住某樣東西）。
6. 其餘不動：DOM 順序（分頁→換幕→關閉）不變；frames 的 `z-index:0` 保留，註解改成現況（卡片內容的 z-index 不外漏）；不碰 `useCardInterfaceController.ts`。
7. 測試：DOM 測只當結構鎖（工具列是覆蓋層第一個直接子層、在 frames 之前；修正前也會過，不算遮擋證據）。遮擋以測試通道截圖為門檻：原尺寸 NorthHall（只用介面外殼）與 WestFantsy；最窄視窗＋多張長卡名（通道改不了視窗大小時沿用 gm-typing 案 `zoom` 模擬並註明）；toolbar 下緣不超過 frames 上緣、無水平溢出；實點齒輪、分頁、換幕、關閉；短窗下生成提示＋錯誤同時出現時卡片仍可捲動。

## 預計改動
- `src/features/card-interface/CardInterfaceOverlay.tsx`：229 行，約 ±0（結構不動，可能只改註解）。
- `src/styles/card-interface.css`：155 行 → 約 155（兩段 `.card-interface-toolbar` 規則合併成一段、改 flex 列、刪 absolute／max-width）。
- `src/features/card-interface/CardInterfaceOverlay.test.tsx`：400 → 約 410。
- 資料夾 18 檔不增；無新檔。

## 驗收
介面開著，卡片頂列與右上按鈕、捲軸不被 app 控制項遮住；多卡分頁窄窗仍可用；verify 綠；測試通道前後截圖（NorthHall 外殼）。

## 業界做法與差距
- 業界：宿主 app 的控制項放在自己的 chrome 列（VS Code webview 的分頁／標題列、瀏覽器工具列、ST `#top-bar`），嵌入的第三方內容拿到剩下的矩形；不浮在內容上，因為宿主無法知道內容哪裡有互動元件。
- 差距：我們把第三方卡片當全版面、再把控制項疊上去，等於賭卡片右上沒東西；NorthHall 賭輸。修法就是補一條 chrome 列。

## 實測（測試通道＋`harness-fake-openrouter.mjs`，測試 root 無金鑰；截圖在 scratchpad/shots）
- NorthHall 原尺寸（800×568pt，只用介面外殼、零 AI）：toolbar 0–41、frames 41–521，下緣不超過 frames 上緣、無水平溢出；卡片頂列與右上齒輪完整可見，齒輪座標 `elementFromPoint` 命中 iframe（不是 toolbar）。前後：`02-northhall-before.png`／`03-northhall-after.png`。
- WestFantsy：toolbar 36px（隨字級自然計算），不蓋捲軸；換幕（有紀錄後可點，已點出）、關閉實點通過。`06-westfantsy-after.png`。
- 最窄視窗：通道改不了視窗大小，以根元素 `zoom:1.6` 模擬約 500px 寬（只算排版壓力測試）。多卡分頁無法在單卡桌開出，改在 toolbar 注入四個長名分頁按鈕（結構與 class 同真實分頁，註明為模擬）：分頁各自截字成「…」，換幕與關閉維持原寬在右側，無溢出（zoom 下 `scrollWidth` 490 vs `clientWidth` 489 是縮放取整的 1px，按鈕右緣仍在內）。`05-tabs-sim-zoom1.6.png`。
- 短窗生成中：真實生成（假端點延遲 25 秒）＋注入容量提示與長錯誤訊息（真實錯誤路徑會關掉介面改跳對話窗，同時出現只能注入），zoom 1.6：提示＋容量＋錯誤＋輸入列疊在底部，frames 仍有約 320px，卡片可見；卡片內部捲動是 iframe 內行為、通道點不到，只驗 frames 高度大於 0 且不被蓋。`08-short-generating-stack-zoom1.6.png`。
- 證據範圍：齒輪「實點」是 `elementFromPoint` 命中檢查（沙盒 iframe 內按鈕通道點不到）；分頁是注入；捲動未實操。
- verify 12 步綠；vitest 新增 1 項結構鎖（CardInterfaceOverlay 16 項過）。

## 證據邊界（三方同意可接受）
- 齒輪只證座標：`elementFromPoint` 命中 iframe、不被工具列攔截，不是真點到卡片內按鈕。
- `zoom:1.6` 只算排版壓力測試，不是真 500px 視窗。
- 多卡分頁與錯誤訊息是注入的（真實錯誤路徑會關介面改跳對話窗）。
- 卡片內部捲動未實操。
