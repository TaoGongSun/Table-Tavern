# refactor-outcome-fields — 做法與觀察

[交接檔](../handoffs/archive/refactor-outcome-fields.md)。中案，不做方案送審。

## 做法
- 地圖或導航清單、記憶或摘要照一般格子收：2026-10-28 曾裁決這兩類不收，但當時的選項說明有誤導，作者 2026-10-29 撤銷，兩類照收〔作者裁決 2026-10-29〕。不收名單（UNTRACKED）整套拿掉。
- 盤點 characters 模式段：FIELDS 也列 INTERFACE／statusbar 模板的每一格。
- 產物 `RefactorOutcome.fields`：`Vec<String>`，空就不寫鍵；匯出匯入走 serde。舊產物缺欄＝空，行為相同，不另分辨〔模型判斷·未裁決〕。
- 套用（`refactor/survey_fields.rs`，只在退場桌）：替 FIELDS 裡樹上沒有的欄位放空值葉子，由開場補齊補每回合文字規則。名字先照點分路徑對齊，對不到再找末段同名的唯一節點，避免平名在頂層長出第二份。已有數字類規則、鏡像、固定鍵的欄位跳過。
- 收據（`receipts/record.rs`）：套用刪掉的規則也記原值，undo 插回。

## 實測（測試通道，claude-haiku-5-5；當時還有 UNTRACKED，撤銷後未重測）
- 西幻（有 UI 版）：盤點 FIELDS 30 格、UNTRACKED 地圖格／座標／可達地點／冒險筆記；但介面條目被判可遊玩，套用後不算退場桌，本案不作用（見觀察 1）。
- 西幻纯文字版：FIELDS 16 格、UNTRACKED 核心记忆／自动总结 7 格。套用後開增量、15 格空值葉子帶每回合文字規則，記憶與摘要不在樹上。續玩 3 回合 GM 回報 剧情、推荐行动、玩家.外表年龄／种族／职业／装备／身份／道具／金币，狀態列照顯示。

## 觀察
1. characters 模式下介面條目被判 playable 的卡（西幻有 UI 版）不標 `statusbar_retired`，開場補齊與本案都不作用；要不要把這類也算退場桌待主線判斷。
2. FIELDS 的「时间」類格子沒有開場值，`time_cells` 收不到，與圍欄 time 重複、實測一直空著。
3. FIELDS 沒有型別，金幣類格子一律文字欄；實測 GM 先用 delta 被拒、下一回合改 replace。

## 已知限制〔模型判斷·未裁決〕
- FIELDS 的名字對不到點分路徑、末段同名又有多個候選時，照原名在頂層建格，可能多出一份重複格。
- 可玩介面版的卡（西幻有 UI 版）在 characters 模式不算退場桌，本案不作用。
