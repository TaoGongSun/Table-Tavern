# Task
Task-ID: interface-takeover-spike
Title: 介面接管可行性實測（西幻卡）：原生模板照搬＋app 代組 XML，定 AI 重構介面軌生死
Created: 2026-08-12T17:40:00+08:00
Status: todo

## Summary
2026-08-12 使用者裁決立案。T3 毛絨轉變實測揭露重構介面軌的規格層錯誤：現行 interface_shell 讓模型發明全新 HTML 殼＋改掉 GM 輸出格式＝卡變質成另一張卡，判定完全不合格。使用者定下的規格：**介面渲染永遠照搬原卡模板，AI 永不發明介面；省額度靠 app 把狀態組裝成原格式 XML 餵殼——做不到就不准動介面。**

技術事實（已從西幻卡讀證，卡＝`TestCards/WestFantsy.png`，PNG tEXt chara chunk 內嵌 JSON）：
- 卡介面＝`data.extensions.regex_scripts[0]`（scriptName「西幻」）：findRegex（235 字，捕 `<GoldenRPG_UI>…<CurrentView>…<WorldSystem>…` 群組）＋replaceString（42,235 字固定 HTML 模板，佔位填捕獲）。**模板是卡作者寫死的，AI 每回合只是填表**；regex_scripts[1] 是開場白選角殼（22,916 字）。
- GM 每回合輸出的 XML（Story／Time／Location／CurrentItems 全量道具／CurrentSkills 全量技能／SuggestedActions）中，骨架＋全量清單是燒 token 大宗；創作內容只有 Story 正文與建議行動。
- 西幻卡在 app 正玩桌（「新的一桌 4」）證明原生殼渲染鏈（interface-card-panel 的 event.raw 餵殼路線）是通的。

## Next action
三步 spike（不進重構管線，組裝器對西幻卡硬編）：
1. ~~**渲染鏈確定性（零成本）**~~ ✅ 2026-08-12 通過：正玩桌三則 GM raw 全數驗證，欄位值經 app 風格重組（排版不同、字節數 5944→5908）後過卡 regex＋模板，殼 JS 取值逐欄位與 AI 親自輸出一致。機制：模板把捕獲原文塞進 5 個 `<script type="text/xml">` 資料槽，殼自己 regex 取值且逐層 trim，排版無關。附帶量測：GM 單回合輸出中骨架＋全量狀態佔 72%，創作（Story＋SuggestedActions）僅 28%。
2. **接管實測（一桌、幾毛錢）**：新開西幻桌，GM 指示改「只出劇情正文＋建議行動＋狀態變化一行制（獲得／失去／時間推移）」；app 持有道具／技能／時間／地點，攔截 GM 輸出補全 XML 餵原生殼。
3. **對照驗收**：①畫面與原卡直玩無差別 ②GM 每回合輸出 token 顯著下降 ③連打數回合，道具增減與時間地點正確跟動。

成敗判準：三項全過＝介面接管可做，正路「原生模板照搬＋app 填表」，再談通用化（重構時從卡的回复规则產欄位對照表，產一次確定性重用）；任一不過＝介面接管廢棄，AI 重構只保留人物拆卡＋世界書整理，interface_shell 產殼路線刪除。

## Constraints
- AI 永不產介面 HTML；模板／regex 一律取自卡檔原文。
- spike 期間現行產殼路線不動（生死由本案定，處置隨結論一起做）。
- 與 [shell-update-flash](shell-update-flash.md) 的關聯：本案成功後殼的餵入源換成 app 組裝 XML，閃白議題在新架構下重新評估。
