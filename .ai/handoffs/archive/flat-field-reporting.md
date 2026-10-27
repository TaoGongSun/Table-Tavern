> 結案 2026-10-27：Sol、Grok 驗收 DONE，主線重跑 verify 12 步綠，squash 進 main。

# flat-field-reporting — 重構桌平欄欄位隨回合更新、不長兩份

自 [acr-live-observations](acr-live-observations.md) 第 2 件拆出。分支 `flat-field-reporting`。

## 問題
clean-orc（`src-tauri/src/import/fixtures/clean-orc-cave.json`）characters 重構後，開場平欄的「剧情阶段/要塞事件」「当前环境」「驻留/交涉角色」留在平欄：GM 在「目前狀態」看得到，卻只能用更新區塊寫樹 → replace 被拒「路徑不存在…若要新增請用 insert」→ GM 改 insert，樹上長出第二份。根因見原立案檔第 2 節。作者目標：重構過的卡每個欄位都能隨回合更新，而且不能兩份。

## 結果
- 做法三方共識後實作（[plans/flat-field-reporting.md](../../plans/flat-field-reporting.md)）：開增量時平欄非固定鍵全搬進樹＋補文字規則、數字解析失敗改文字、「/」對齊層首段比對與執行期接段、insert 字形接登記葉子、狀態圍欄改走 replace。
- 單元測試：`refactor/tests/flat_field_reporting.rs`、`mechanism/slash_key_tests.rs`；`rule_reporting.rs`、`block_scope_tests.rs` 舊斷言照新行為改。

- 真實流程（2026-10-27，測試通道、claude CLI Haiku）：clean-orc 匯成角色卡、貼開場白 2、重構（characters）全部套用→平欄四鍵全進樹、平欄清空、四鍵都有規則；三回合 GM 用 `/剧情阶段~1要塞事件` replace 與 `/驻守天数` delta 都寫進樹、無拒收、樹與平欄沒有兩份。`当前环境`、`驻留/交涉角色` 三回合都沒被 GM 回報（通道是通的；Haiku 改用 ```state 圍欄的 place／present 寫同樣意思），見下。

## 已知缺口（另案）
- `当前环境`、`驻留/交涉角色` 與 place／present 同義，真實流程三回合沒被 Haiku 回報：寫入通道是通的，屬模型回報缺口。本案只保證欄位寫得進、不兩份，不宣稱所有欄位都持續更新 → [card-field-overlaps-fixed-keys](../../tasks/card-field-overlaps-fixed-keys.md)。
- 殼（介面接管）路線的平欄非固定鍵 → [takeover-flat-nonfixed-keys](takeover-flat-nonfixed-keys.md)。
