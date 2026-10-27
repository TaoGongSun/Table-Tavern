# refactor-trigger-overlap — 規格與已知限制

## 定案（Sol、Grok 審 DONE：採 A＋附帶項，C 不做）
- 根因：once 觸發求值不讀自己的旗標，只靠 `when` 帶 `Flag{expect:false}`；重構 AI 產的 once 多半沒帶，跨門檻後每回合重演。跨條目對同欄各寫觸發本身互補，不去重。
- A：`mechanism/triggers.rs::evaluate_triggers` 對 once 先讀 `flag`：真假沿用 Flag 條件（去空白、不分大小寫、true／1），路徑不存在＝未演；已真整條跳過。
- 旗標路徑指到分支、父層是葉子或有空段（釘不進樹、或會蓋掉分支）：當作已演跳過並 warn，不動樹、不記已觸發；`local_rules` 釘旗標沒寫成也只 warn 不記。
- 重構套用比照 EJS 匯入（`import/mechanism.rs`）替有效 once 旗標補唯讀規則（branch＝首段）；該路徑已有不同規則不覆蓋、記 warn（`refactor/apply_mechanism.rs::pin_once_flag_rules`）。

## 已知限制
- `flag` 為空的 once 停不了（測試卡樣本 0 條）。
- GM 或玩家把旗標改回 false、或收回到演出前，條件仍成立就會重演一次（與 EJS 相同）。
- 已套用的舊桌要重套才有唯讀規則；A 本身下回合即生效。
- 跨門檻那一回合，跨條目或同條目 range＋once 對同一事件的文本仍可能並列（只那一回合）。
- 路徑寫不進樹的 once 每回合記一次 warn（罕見）。
- 「收回後重演」的測試以複製狀態模擬退回，未走真收回 API。
