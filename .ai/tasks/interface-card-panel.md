# Task
Task-ID: interface-card-panel
Title: 介面卡渲染面板：ST 介面卡原樣顯示（殼匯入＋沙盒面板）
Status: in-progress
Created: 2026-08-13T00:30:00.614239+08:00
Updated: 2026-08-13T00:30:00.614239+08:00

## Summary
2026-08-04 拆「西幻魔法世界模拟器」卡後拍板立案。目標：ST 介面卡（卡內自帶 HTML 殼）匯入後，聊天旁開一個可收放的沙盒網頁面板原樣渲染，對話與狀態照舊由本 app 驅動。定位＝相容選配：原生狀態欄仍是省 token 正解，想要原汁原味、願付 token 的玩家才開面板。搶 ST 用戶的拼圖因此湊齊三塊：吃得下他們的卡、跑得起（原生省 token）、看得到原味（本面板）。

規格細節（拆卡實據（樣本在 TestCards/，gitignored）、範圍（v1）、交界（state-values-mvu 包 4 順手帶最省）、驗收（v1））見 [plans/interface-card-panel.md](../plans/interface-card-panel.md)。

## Next action
- 2026-08-04 v1 完成且**實機驗收全數通過**（匯入→開介面→點行動→送出→GM 照卡片格式回覆→介面就地換新畫面）；聊天收合 XML 已拍板不做；v2 首要＝省額度（歷史裡每輪整包 XML 重送，要留正文砍掉重複區塊）

## Constraints
- 卡內腳本一律沙盒，無任何 app API；橋只有「文字入輸入框」一條單向道。
- 不碰 DRM（不解密、不繞驗證）。
- 面板為選配，任何失敗回退純文字對話。
