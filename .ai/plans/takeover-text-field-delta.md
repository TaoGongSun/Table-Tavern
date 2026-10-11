# takeover-text-field-delta：清單／文字欄被下 delta

## 根因（零 AI 重放前次產物 `scratchpad/rt/root/data/worlds/01M4NDE7TK1P3C9B57HF5BTGEM`；三方審查 PASS）
- 規則沒標錯：`神經狀態` 是 `kind: list, update: replace`，值是多行記錄 `neuro|疲勞|20|…`。
- GM 送 `{"op":"delta","path":"/神經狀態","value":"neuro|壓力|10|…"}`——只寫變動的那幾行、數字有時是絕對值有時像增量，等於把 delta 當「部分更新」。
- **缺陷 1：拒收沒回饋給 GM。** `apply_delta` 先檢查值是不是數字，失敗記 `Error`「更新值不是數字」；文字／清單欄收 delta 也記 `Error`。`build_notes` 只收 `Rejected`，這類錯誤永遠不進下一輪「上一輪被系統擋下的更新」，協定卻寫被擋下會告訴你。GM 3 回合同錯 3 次；訊息「不是數字」本身也誤導。
- **缺陷 2：提示詞沒講清單欄怎麼更新。** 協定只說「數字欄一律 delta」「文字欄 replace」，行內帶數字的清單欄落在兩句之間；GM 看不到各欄型別。
- **前案改法 4 沒解決本案**：前次實測時本桌 replace 範例恰好就是清單欄 `/主動能力`，GM 照樣 3/3 下 delta；改法 4 後範例改成 `/力量/數值`，與本案無關。
- 同類：NorthHall `/糧草`（文字欄）被下 delta，同走缺陷 1。

## 改法
### 1. `apply_delta` 判定順序（`mechanism/apply.rs`）
取節點（無 → 既有 Error「路徑不存在」）→ 是葉子（分支 → 既有 Error）→ `rule_for` → update 是 Local 或 kind 是 Roll 走既有骰值訊息、update 是 Reject 或 kind 是 ReadOnly／Derived 走既有唯讀訊息（都 Rejected）→ kind 是 Text／List 記 Rejected（新訊息）→ 最後才解析 delta 數值：
- 清單：「{path} 是清單欄，不能用 delta；改任何一行都用 replace 寫出整個欄位的完整字串（沒變的行也照抄）。」
- 文字：「{path} 是文字欄，不能用 delta；請用 replace 寫新值。」
- Number／Counter 收非數字（`"-10%"`、`"5點"`）→ Rejected「{path} 是數字欄，delta 的值要是純數字增減量（例 5、-10）。」
- Pair 收非數字 → Rejected「{path} 是「現值/上限」欄，delta 只給現值的增減量（純數字，例 -10）。」
- 現值本身不是數字（帳本壞值）維持 Error。
- 英文桌訊息沿用既有做法只寫中文〔模型判斷·未裁決〕。
### 2. insert 撞到清單葉子（`apply_insert`）
GM 寫 `insert /神經狀態/-` 時路徑中間是清單規則的葉子：改記 Rejected，路徑報清單欄本身，訊息「{path} 是清單欄，不能逐行插入；改任何一行都用 replace 寫出整個欄位的完整字串（沒變的行也照抄）。」清單另設 Local／Reject 時改回骰值／唯讀訊息。其他不回饋的 Error（路徑不存在、分支不能增減等）不在本案，見 [mechanism-error-feedback](../handoffs/archive/mechanism-error-feedback.md)。
### 3. 清單欄通用提醒（`transport/context.rs`）
- 規則裡只要有任何 `kind: list`，在本桌範例的位置（有 guide 的接管桌，或無 guide 的 DeltaOnly 增量桌；沒有可用範例時也照加）加一句不點名路徑的提醒：「值是多行字串的清單欄，就算行內有數字也不能用 delta；改任何一行都用 replace 寫出整個欄位的完整字串（沒變的行也照抄）。」中英各一。
- 只依規則，不讀即時狀態樹（system 讀樹會隨樹形變動觸發 SystemChanged 整條 GM 線重開，也違反 MECHANISM-FORMAT.md「凍結快照不含狀態」）；不改 `gm_lane_system` 參數。
- 沒有清單規則的桌提示詞逐字不變；有清單規則的既有桌在升級後 GM 線因 system 改變**重開一次**。
### 4. 文件
`.ai/reference/MECHANISM-FORMAT.md` 記帳表：`rejected` 例子補「delta 打到文字／清單欄、數字欄給了非數字增量、insert 打進清單欄」；`error` 的「型別不符」改成只指帳本現值壞掉。`context.rs` `table_update_example` 上方註解同步。

### 影響範圍
`apply_delta`／`apply_insert` 的改動作用於所有桌型（含 Upstream、全量桌、沒規則而推定為 Text 的欄）。開場白自帶的錯誤 delta 會在第一回合以「上一輪被擋下」回饋給 GM。
清單提醒句只看規則：有 guide 的未重構 MVU（Upstream）桌、規則是 list 但樹上是分支的桌（如 DongeonMaster `魔物`）也會看到；句首已限定「值是多行字串的清單欄」，對分支清單不構成錯誤指示。

### 不做：清單欄 delta 逐行合併〔模型判斷·未裁決〕
- 鍵欄位各欄不同，同一張卡內就不一致；拿第一或第二欄當鍵會靜默生出重複行。
- delta 的語意是數字增量，拿來逐行合併會把「增量還是絕對值」的歧義靜默存進樹。
- 省下的輸出有限。

## 測試（cargo）
- `apply_delta`：清單欄收字串 → Rejected、訊息含「清單欄」「沒變的行也照抄」、值不變；文字欄收數字 → Rejected；Number 收 `"-10%"`／Pair 收 `"5點"` → Rejected 各自訊息；現值壞的數字欄 → 仍 Error；readonly／roll／derived 收非數字 → Rejected、唯讀／骰值訊息；Upstream 桌＋推定 Text 欄收 delta → Rejected、值不動；`build_notes` 帶出清單欄那筆。
- `delta_on_text_field_is_a_hard_error` 改名、斷言改 Rejected 且值不變。
- insert `/神經狀態/-` → Rejected「不能逐行插入」、樹不變；清單／文字欄配 Local／Reject 收 delta 或 insert → 骰值／唯讀訊息。
- 提示詞補：英文桌提醒句、Upstream 有 guide 桌加提醒。
- 重放：前次三回合三筆 delta 原樣對 Transfur-safe 規則與樹套用，三筆都進 notes、同批其他 patch 照常套用。
- 端到端：套用 → `world.state.notes` → 下一輪 API 與 CLI 的 GM 尾段出現回饋；下一輪修正後 notes 清空。資料內嵌規則與樹（TestCards 被 gitignore）。
- 提示詞：有清單規則（guide 桌、無 guide DeltaOnly 桌、只有清單規則）→ 含提醒；無清單規則 → 逐字不變；同規則、樹形改變後 system 逐字不變。
- `npm run verify` 12 步綠。

## 驗證計畫
1. **零 AI**：上述重放；測試通道開前次 Transfur-safe 接管產物桌，讀 GM 實送提示詞確認有提醒。
2. **真模型修復測試**（Haiku 3 回合，約 $0.4）：固定用前次接管產物（不重新重構）。記 `神經狀態` 的 op 與接受與否；若第 1 回合仍 delta，第 2 回合尾段要有回饋、之後改 replace。被 replace 後：未變行保留、無重複行、數字沒重複累加、行數不減少。其他欄接受數只作觀察。

## 已知限制
- 第 1 回合仍可能下錯，靠下一輪回饋修正；回饋上限 5 條（`build_notes`）。
- 清單欄改一行就要重寫整欄（沿用 guide 既有規定，本案不增加輸出）；逐行更新見 [list-field-line-update](../tasks/list-field-line-update.md)〔作者裁決 2026-10-11〕。
- 規則是 list 但樹上是分支的桌（MVU 清單）也會看到清單提醒句，句首限定多行字串，靠模型自己分辨。

## 驗證結果（2026-10-11，測試通道，claude CLI）
- verify 12 步綠（cargo test 1694 過；第一次跑生圖下載 4 條逾時，即已立案的 generated-image-download-flaky，重跑綠）。
- 零 AI：重放前次三回合三筆 `神經狀態` delta，三筆都進 notes，同批其他 patch 照常套用（`delta_refusal_tests.rs`）；端到端回饋 API／CLI 兩路（`feedback_e2e_tests.rs`）。
- 真模型修復測試（Haiku 3 回合，$0.12）：複製前次接管桌（`scratchpad/ttfd/root`，桌 `01M4NDE7TK1P3C9B57HF5BTGEM`，不重新重構）接著玩；GM 線因 system 加了提醒重開一次，快照含提醒。
  - 第 1 回合仍下 delta（歷史裡有前三回合的錯誤示範），記 `rejected`；第 2 回合尾段帶「神經狀態 是清單欄…」回饋。
  - 第 2、3 回合改用 replace 並被接受：四行都在（疲勞／壓力／痛苦／恐懼）、無重複行、數字合理遞增（壓力 35→55→75，未重複累加）、行數不減。
  - 其他欄 patch：2/2、5/5、3/3 全收（觀察用）。
- 三方驗收（Sol、兩位 Opus）程式部分 PASS；insert 撞清單欄是審查時新增的範圍，已一併經三方看過。
