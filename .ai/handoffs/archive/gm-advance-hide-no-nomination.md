# gm-advance-hide-no-nomination — 不要求點名的桌藏「GM 推進」

## 拍板
- 〔作者裁決 2026-10-28〕選 A2：確定不點名的桌藏「GM 推進」（藏不反灰）；判定未讀回或讀取失敗照常顯示。理由：卡片自帶格式桌 GM 不出「下一位」行，推進只剩一次旁白、與「GM 旁白」同義。
- 〔作者裁決 2026-10-28〕全舞台卡桌（沒有發言角色，`castEmpty`）藏「請他發言」（對象只剩 GM、等同 GM 旁白）；其他桌維持，發言對象是 GM 時也照常顯示。`castEmpty`＝主區沒有非舞台卡，所以同樣藏的還有：沒有角色的桌、角色全部封存的桌、主區只剩本幕還沒出場的 auto_hidden 卡——這幾種也只剩 GM 可選，效果與全舞台卡桌相同；唯讀桌進桌時名單清空、輸入框鎖住，藏起來不影響操作。
- 〔作者裁決 2026-10-28〕全舞台卡桌後端回不點名，「GM 推進」由反灰改藏，接受。
- 「送出並接話」維持只認確定會點名（未讀回、讀取失敗都不給），不受本案影響。

## 現況
- 已施工於分支：`useGmOnlyTable` 內部點名判定改三態（true／false／從沒讀成 undefined）＋最近一次讀取失敗旗標（重讀失敗時推進恢復顯示，送出並接話與 gmOnly 沿用上次），對外 `nominates`（確定會點名，給送出並接話）＋`skipsNomination`（確定不點名，給 GM 推進），經 `App.tsx`→`AppWorkspace.tsx`→`PlayView.tsx`。後端不動（沿用 `gm_table_mode.nominates`）。
- `PlayView` 的「請他發言」條件加 `!castEmpty`。
- 測試：`gm-only-table.test.tsx`（三態、換桌每次 render 不閃藏、PlayView 藏／顯示、全舞台卡桌藏請他發言）、`app-relay-lock.test.tsx`（App 層真 PlayView：未讀回、會點名、重讀變不點名、重讀失敗、從沒讀成）。
- 已知限制：兩張都不點名的桌互換時，讀回前會先顯示再藏（作者選 A2 的代價）。

## 結案
2026-10-28 Sol、opus-review 驗收與複審 PASS，已合併 main。
