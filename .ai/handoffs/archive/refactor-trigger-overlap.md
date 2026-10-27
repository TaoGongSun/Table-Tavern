# refactor-trigger-overlap — once 觸發跨門檻後每回合重演（已結案）

規格與已知限制見 [plans/refactor-trigger-overlap.md](../../plans/refactor-trigger-overlap.md)。

## 結論
- 根因：once 觸發求值不讀自己的旗標，只靠 `when` 帶 `Flag{expect:false}`；重構 AI 產的 once 多半沒帶，clean-orc 第 31 天「當前情境」同時印 5 段已演過的里程碑。兩條目對同欄各寫觸發本身互補，不去重。
- 修法：`mechanism/triggers.rs` once 旗標已真就跳過、路徑釘不進樹當已演並 warn；重構套用替 once 旗標補唯讀規則（不覆蓋既有）。
- Sol、Grok 驗收 DONE；verify 12 步綠。

## 剩下
無。
