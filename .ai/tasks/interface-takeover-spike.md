# Task
Task-ID: interface-takeover-spike
Title: 介面接管：重構把卡的每回合輸出格式照搬成骨架，app 用狀態樹組裝介面
Status: todo

## Summary
2026-08-12 立案並完成驗證。規格：**AI 永不產介面**，重構只把卡規定的每回合輸出格式照搬成骨架、變動處挖 `{{狀態樹路徑}}`，運行時 app 用狀態樹填值、過卡自己的顯示腳本渲染；`source-card.png` 原封留桌上當介面唯一來源。

西幻卡（`TestCards/WestFantsy.png`）三回合實測，三項驗收全過：畫面與原卡直玩無差別（地圖 11×7、區域點擊互動、五分頁欄位都有值）；每回合輸出 5944→2670 字（省 55%），同一次呼叫的劇情多 2.8 倍；狀態正確跟動、零拒收。commit d3f8a7e。

整案最關鍵的教訓：卡原文的規定要分三類處理——值格式（條目寫法、數量、白名單）照搬進 GUIDE，殼靠它渲染；傳輸容器（「必须只输出一个XML数据块」「必须依次输出五大模块」）一律丟掉，那是 ST 不保管狀態的產物，抄進去 GM 會每回合多印一份廢資料，還會讓卡的 regex 抓到內嵌那份（地圖整頁報廢）；固定資產（地圖矩陣、白名單表）照抄成骨架固定文字、不做狀態欄位。

`TestCards/` 分型完成：資料槽型只有西幻一張（已過）；另有 MVU 前端型（bcd368、HeroTrainingUnderSide，帶變數更新腳本，協定同源應最順）、整頁前端非 MVU（DongeonMaster／Transfur／RPGImmortal）、狀態欄＋真人物多（NorthHall，預期結論是拆角色不接管介面）、雲端載入器（TrainEmperor，該被 unsupported 擋掉）。

原待辦 1「拆角色 vs 保留介面交給玩家選擇」已於 2026-08-13 升級成獨立任務 [refactor-mode-split](../handoffs/refactor-mode-split.md)（兩段式定向＋模式專屬解析），本案不再追蹤。

## Next action
- 逐型驗其他卡（MVU 前端型 bcd368 優先，見交接檔待辦 2），最後清舊產殼路線（待辦 4）；玩家選擇那條已移交 refactor-mode-split

## Constraints
- AI 永不產介面 HTML；模板／regex 一律取自卡檔原文，每卡自訂、禁 app 統一文法。
- 拆角色與保留介面二選一交由玩家決定，app 不自動判斷（「有幾位真人物」本機猜不準，是盤點階段 AI 的工作）——具體化見 [refactor-mode-split](../handoffs/refactor-mode-split.md)。
- 與 [shell-update-flash](shell-update-flash.md) 的關聯：殼的餵入源已換成 app 組裝，閃白議題在新架構下重新評估。
