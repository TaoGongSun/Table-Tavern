# narration-br-dice-target

小案（估核心 30–100 行）。來源：多人房間與跑團實測（實測紀錄在 `<scratchpad>/vq3/R2/RESULT.md` S7、`<scratchpad>/vq3/D/RESULT.md` D3）。

## 範圍
1. 房間故事串（桌面房間與手機共用 `party/src/features/room/StoryView.tsx`）GM／NPC 旁白裡的 `<br>` 照字面顯示：改成換行、結尾多餘的不留空行；仍維持純文字，不開放其他 HTML。
2. GM 點名別的角色（「請雷恩擲…」，卡名「雷恩（花豹）」）時，別人的手機骰鈕主鈕不該是那條骰式：`dice-suggestions.ts` 的名字比對加「卡名去結尾括號」別名。
3. 左欄「規則與擲骰指南」裡的「到桌設定加入」停用鈕：改成可按、關指南並開世界設定（規則書選單在那頁最上方）。

## 現況
- 分支 `narration-br-dice-target` 施工完成，未合 main；verify 17 步通過。
- 真 app 實測三項都過（測試通道＋本機 Node 房間伺服器跑本分支 party 建置＋Playwright WebKit；GM 旁白由本機假端點供，零真模型），截圖 `<scratchpad>/vq3/FIX5/shots/`（X1～X3）；測試房已結束、app 已 quit。
- 實測後補：`<br>` 緊跟的一個換行併進去（同 Markdown），`<br>\n\n` 不再多空一行。
- `party/` 改動要部署才生效（第 2 項手機端也經 `@desktop` 打包，一樣要部署）。

## 觀察
- VN 格子由 Rust 照空行切、不清標籤，模型寫 `<br>` 會照字面出現（可能自成一格）；實測未見，本案不改〔模型判斷·未裁決〕。
- 單人桌面旁白結尾的 `<br>` 會多一行空白（Markdown 照 HTML 換行），未處理〔模型判斷·未裁決〕。

## 下一步
- 主線讀 diff、送審；截圖待主線排測試通道時段。
