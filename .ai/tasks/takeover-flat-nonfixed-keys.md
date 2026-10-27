# takeover-flat-nonfixed-keys — 介面接管路線開增量時平欄非固定鍵凍住

## Summary
介面接管（殼）路線先整份換樹再開增量，`open_rule_reporting` 被跳過；接管前平欄若有 time／place／present 以外的鍵，會凍在「目前狀態」裡不更新。flat-field-reporting 只處理重構路線，三方審查都建議本項另立〔模型判斷·未裁決〕。

## 已知線索
- `mechanism/apply.rs:347` 先開增量、`:390` 因而跳過搬移；介面重建以 `state_fields` 決定欄位集合（`interface.rs:226`）。
- 全搬會重引入已被介面欄位取代的欄位，會蓋到殼的頂層鍵；全刪會掉開場值。需先定「已取代／未收編」判準。
- 依賴 flat-field-reporting 的搬移與接段規則，該案結案後再開工。

## Next action
未排程；等 flat-field-reporting 結案，開工先重現。

## 驗收方向
接管桌多輪續聊，非固定劇情欄位只有一份且會更新，介面欄位不重複、開場值不遺失。
