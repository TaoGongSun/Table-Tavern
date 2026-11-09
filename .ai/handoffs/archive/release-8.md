# release-8 — v0.5.2 之後的說明文字、截圖與發佈

Status: v0.5.3 已公開（2026-11-09，8 產物、標 Latest、latest.json 0.5.3），房間伺服器已部署，安裝檔已備好交作者傳 pCloud。
流程沿用 [plans/release-2-ci-windows.md](../plans/release-2-ci-windows.md)。

## 已完成
- CHANGELOG `[Unreleased]`（繁中＋英文，含「你的資料去哪裡」）、README 兩語、房間說明書三語、下載頁文案十語（入口名同步「和朋友一起玩」）。
- 截圖重拍：下載頁 lobby、status、vnWizard、table、oneLine、oneLineOutline；說明書 12 張，都是三語。cardUi、vnPlay 查過介面未變動到畫面，不重拍〔模型判斷·未裁決〕。
- 順手修：`useDesktopRoom` 的房主預設暱稱在語系載入前取值，非繁中也顯示「房主」，改成跟著當下語系。
- 新規則：部署發佈前 README、Change log、網頁說明現況需要 Fable 常識審查〔作者裁決 2026-11-09〕。

分工：到 2026-11-10 22:00 前 Sol 施工、Grok 審查〔作者裁決 2026-11-09〕。

character-presence 已關案〔作者裁決 2026-11-09〕：同義欄位已有 present 同步，「駐留／交涉／集合」類名冊不代表人在現場，不接線。vn-sprite-exit 已合 main（已補 CHANGELOG 一行）。

## 已知
- 截圖頂列的版號是 v0.5.2。
- 房間伺服器已部署 2026-11-09（Version e72c9601）；真手機加入與擲骰照實測佇列由作者驗。
- room-15 是展開的自訂骰鈕列（拍照時 GM 開場沒寫骰式）。
- 拍照手法與腳本在 `TestCards/download-tour/`（本機限定）。
