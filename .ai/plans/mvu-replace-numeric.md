# mvu-replace-numeric 規格

## 拍板
- 〔作者裁決 2026-10-04〕沒重構的 MVU 卡：數字欄 replace 照上游 MVU 接受；重構過的卡維持強制 delta。
- 舊桌不遷移（發佈前舊桌不用管〔作者裁決 2026-10-02〕）。

## Sol 審查共識（動工依據）
- 更新策略 `NumericUpdate`（`mechanism/policy.rs`）只在載入層算一次：
  - `Upstream`＝沒重構（`refactor_mode` 為 None、沒有重構產物檔，比照 `refactor/reset.rs` 的 `refactored()`）**且**有 MVU 來源依據；
  - MVU 來源依據＝世界書匯入過 MVU 鷹架（`mechanism.incremental`；未重構桌只有 `import_mechanism` 掃到 `[initvar]`／`[mvu_update]` 才會開）或有角色卡載入 MVU 腳本（`CardInterface.mvu`）；
  - 其餘一律 `DeltaOnly`（characters 模式、清殼後的重構桌、舊重構桌無 mode 但有產物檔）。
- 策略掛在 `Mechanism.numeric_update`（`#[serde(skip)]`，不落檔、預設 DeltaOnly）。GM 回合在 `gm_materials` 算一次，提示（`gm_system_prompt`／`gm_lane_system`／`assemble_gm_messages`）與提交（`apply_block`）用同一份；開場白在 `append_opening` 算一次。
- 放行只在 `mechanism/apply.rs` 的 `replace_existing`（insert 命中既有欄也走這裡），涵蓋 Number／Counter／Pair，只解除 Delta 限制；Local／Reject／底線唯讀照舊。Pair 未重構整值 replace；重構維持「現值拒改、上限可改」。
- replace 語意照上游 MagVarUpdate 438f9ffc `src/function/update_variables.ts` L906-956（JSONPatch replace＝set，非 strictSet）：`[值, 說明]` 只改第 0 項（舊值是數字且新值不是 null 才 `Number()`）；舊值是數字、新值是字串→`Number()`；其餘照新值寫；不夾 min/max、不拒收。
- 系統提示 `transport/context.rs`：原規則版逐字不動；上游版把「只要說這一幕變動了多少」、數字欄與「現值/上限」兩條、「上下限與拒收由系統把關」換成收 replace、replace 不夾值的說法（中英文）；每桌策略固定，不破快取。
- 前後端比對：`src/shared/contracts/mvu-replace-parity.json` 一份案例同時跑後端（GM 提交路徑）與墊片 `Mvu.parseMessage`，結果必須完全一致（不留差異標記）。

## Sol 驗收第 1 輪修正（2026-10-04）
- 〔作者裁決 2026-10-04〕replace 轉不成數字（NaN／非有限）：後端與墊片統一照上游結果寫 null。墊片在 set 路徑照上游讓非有限數留到整批做完，序列化前才轉 null；其餘來源的非有限數、既有輸入驗證與可復原的來源快照照舊。
- set 保留原 JSON 型別、不靠葉子文字猜（`mechanism/upstream_set.rs`，JS 值模型、數字可為 NaN；第 2 輪改成批內完整型別視圖，見下）。GM 提交（`apply_gm_block` 把輸入的 stat_data 交給 apply）與開場白（`opening_stat`）兩條都帶。
- 上游策略下所有 replace（含文字欄）都走 set，型別才對得上；本地擲骰、唯讀照舊擋。
- `js_number` 照 JS `Number()`：JS 空白集合（含 U+FEFF、Zs、U+2028/2029，不含 U+0085）、0x／0o／0b 任意長度照就近偶數捨入。
- 樹模式（沒有 stat_data 的桌）跨回合仍只有葉子文字，型別只能照文字還原；同一批內照帶型別值。

## Sol 驗收第 2 輪修正（2026-10-04）
- 批內完整型別視圖（`TypedView`）：一批指令維持整份帶型別 stat_data（樹模式照葉子還原一份），照指令先後同步——set 照算出的型別寫整個位置（父欄 set 作廢舊的子欄寫入，子欄 set 不動兄弟欄）；delta／insert／remove／move 與狀態欄平欄照它對樹造成的前後差異寫（還原規則同 `merge_tree_change`），move 成功則補回原值型別。舊值一律從這份取，不再用投影相等判定寫入是否有效。
- 合回：`Outcome::typed`（`TypedBatch { doc, mid }`）＝做完這批的整份帶型別 stat_data 與當時的樹；新表以 doc 為底，只把批次之後的樹改動（骰值重擲、觸發旗標、衍生值）照一般規則合上去。
- 墊片：非有限數標記依最後一次寫入的來源——每個寫入點（set、add、insert 各分支、delete 各分支）先撤銷與寫入位置重疊（同處、祖先、子孫）的標記，只有 set 寫出非有限數才重新標；陣列 splice 等會移位的寫入以整個容器為寫入位置。

## Sol 驗收第 3 輪修正（2026-10-04）
- move：只有這一筆 move 明確搬成功（`apply_move` 回報）才照原型別寫回目的欄；拒收（來源或目的是底線唯讀欄）、失敗（目的中間層被占）都不動帶型別視圖。後端 move 的 JSONPatch 寫法是 `from`＋`to`；墊片照上游不套用 move。
- 墊片 insert 合併：照 lodash.merge 實際覆寫的位置撤銷非有限數標記（兩邊都是可合併容器就往下遞迴，否則整鍵換掉；來源 undefined 不寫），沒被動到的兄弟欄保留標記。
- 比對案例可標 `backend_rejected`（後端記帳預期拒收的路徑；墊片沒有記帳），結果兩邊仍須完全一致。

## 驗收
- 雲端：`npm run verify` 全綠（含後端單元、context 兩版、墊片比對）。
- 本機真卡（測試通道）：2026-10-04 全過，見 [交接檔](../handoffs/archive/mvu-replace-numeric.md)。

## 施工中的判斷
- 策略算在呼叫點（`gm_materials`、`append_opening`）而不是 `read_state`：判定要讀卡片介面（解 PNG），`read_state` 呼叫太頻繁。
