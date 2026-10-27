# card-field-overlaps-fixed-keys 做法（方案 C：App 鏡像）

兩輪送審後主線、Sol、Grok 三方共識採 C。否決 A（丟掉或改寫卡欄位）的原因：觸發條件（`mechanism/triggers.rs` 的 `evaluate_triggers`）與 `{{state:…}}` 佔位只讀樹，place／present 在平欄，A 會讓條件不成立、佔位變空。

以下細節〔模型判斷·未裁決〕。

## 規格
1. 盤點是唯一標記來源（`refactor_ai/survey.rs` 第七段＋輸出區塊 `## MIRRORS`，一行「FIELDS 欄位名 = place｜present」；`survey_parse.rs` 只收這兩個值，不動 FIELDS）。盤點看得到整張卡，含 characters 模式會零呼叫淘汰的狀態欄格式段。集合、帶額外語義（天候氣氛、遠端交涉者）的不標。present 標記不預先承諾 character-presence 的契約。機制改寫提示只說明固定鍵由 App 回報、不必寫規則，不再要求標記。對照經前端照抄進產物 `mirrors`（匯出匯入保留）。
2. 套用（`mechanism/mirror.rs`）：開回報管道（`open_rule_reporting`）之後，`mark_survey_mirrors` 先清掉規則上既有的 `same_as`，收圍欄的桌才把產物的對照蓋上去——名字只認樹上頂層葉子整段精確相等（斜線鍵是一整段），沒有規則就補文字規則承載；對不上、命中分支的行丟掉並記 warn log，不建空葉、不捏初值，不經一般規則衝突檢查。接著 `adopt_mirror_marks` 收件：place／present、樹上既有葉子、不在集合裡、非萬用路徑 → 收進 `mechanism.mirrors`（點分路徑 → 固定鍵）並拿掉該欄規則。撤銷重構由收據的 `mirrors_before` 寫回。
3. 套用當下同步一次：平欄固定鍵有值則抄進卡欄位；缺席時卡欄位留卡的初值（不反寫平欄：「托伦 (Toren)」這種值寫進 present 會被在場判定當成對不上名冊的人，且收據不還原新增的平欄鍵）。
4. 每回合（`local_rules` 開頭，觸發與衍生值之前）：平欄有固定鍵就抄進卡欄位（同輪兩邊都寫時固定鍵勝）；present 明確寫空就清空卡欄位。結果跟著事件快照走，收回照常回捲。
5. 玩家手改平欄（`commands/state.rs` 的 `set_table_state`）：改到的固定鍵抄進卡欄位，清空也照清。
6. 頂部狀態欄上的鏡像欄唯讀（改地點／在場那一格就會帶過去）。
7. 鏡像欄不列進 GM「目前狀態」（`leaf_shown`），沒有規則，不要求模型回報；觸發條件與佔位照舊讀它。（偏離送審版「目前狀態仍兩列」：列出來會讓「有變動另附更新區塊」招來雙報，待驗收裁決。）

## 已知限制
- 頂部狀態欄仍並列固定鍵與鏡像欄兩列；套用後到 GM 第一次回報固定鍵之前可能不一致（固定鍵缺席、鏡像欄留卡的初值）。
- AI 標錯時，固定鍵會蓋掉卡欄位原本的資訊。
- 舊桌不遷移（發佈前舊桌不相容）。
- 「固定鍵必勝」只限鏡像欄仍是葉子：同輪更新把它 remove 或改成分支後就停止鏡像，不會自動重建。

## 驗收
- `refactor/tests/fixed_key_mirror.rs`：套用後未續聊就一致、鏡像欄沒有回報義務、同輪雙寫固定鍵勝、觸發條件與命中文本 `{{state:}}` 讀到鏡像值、present 清空、收回回捲、固定鍵手改（走 `sync_mirrors_from_edit`，非 `set_table_state` 指令入口）、撤銷重構還原、未標記桌不受影響、不合格標記（非固定鍵、不在樹上、萬用路徑、集合裡）丟掉、接管桌不收。
- `refactor/tests/fixed_key_mirror.rs` 另含：盤點對照只認精確同名葉子（字形不同、不存在的行丟掉、AI 規則自帶的 same_as 不算）、開回報管道提前返回時整份丟掉不建葉。
- `survey_parse_tests.rs`：MIRRORS 解析、FIELDS 不受污染。`refactor-review.test.ts`：產物照抄、匯出匯入保留、壞值拒收。
- `StateBar.test.tsx`：鏡像欄唯讀。
- `npm run verify` 綠。
