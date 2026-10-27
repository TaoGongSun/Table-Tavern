# refactor-rule-conflict-same-field — 同欄多份規則的合併與裁決

送審：Sol 串 01a17f1b-836e-7681-a8a7-bf34e113b0d9、Grok 串 01a17f1b-82f2-7dd1-8120-fcfeaf70d3b0，三方共識採「相容合併、真矛盾只裁決該欄」。

## 規格（`src-tauri/src/refactor/rule_merge.rs`）
- 適用跨條目與同條目兩種字形對齊到同一路徑兩種情況，在 `Candidates::collect` 一起處理一次。裁決結果寫回每一份候選，`apply.rs` 依序 extend 誰後寫都一樣。
- 相容：kind、update、formula 相同；branch 相同或一邊缺省（取明確值）；`same_as` 不比（套用時由盤點重寫）。number 對 counter 算不相容。
- min／max 取交集（min 取大、max 取小、相等合法），缺省照各型執行端語意：一般型＝無界；pair 下限 0、上限無界（來自現值/上限）；roll 1–100。全部缺省維持缺省；交集為空＝不相容。
- inject 取較頻繁：Turn＞Snapshot＞Rare。
- 真矛盾：整欄所有原始候選（跨條目＋同條目不同字形，`align_rules` 只分組不裁決）收齊後在 `Candidates::collect` 一次裁決，原樣取來源 uid 數值最小那份（條目多個 uid 取最小、非數字排最後），同 uid 依候選先後（條目 index、機制 index、條目內原路徑字典序）；記 warn（路徑、各來源、採用哪份）；其餘照套。字形歧義（一個名字對到多個路徑）維持整次拒套。

## 已知限制
- 套用當下不夾開場值：合併後的 min 在之後 delta 時才生效（例：現值 0、合併 min=1，下一次 delta 變 1）。
- delta 沒有單位欄，同路徑兩份 counter／delta 一律當同語意合併。
- formula 不分 kind 都要求相等（非 derived 實際都缺省，無行為差）。
- 只合併規則；兩條目對同欄的觸發照原樣並存（另案）。

## 驗收
- `refactor/tests/rule_merge.rs`：clean-orc 形狀合併成 min=1 且可撤銷、開場值低於 min 與後續 delta、三來源矛盾不受順序影響、選取順序反轉、同條目字形合併／裁決、同條目字形＋跨條目混合矛盾取原樣、inject 九種組合、kind／update／formula／branch／same_as、一般型與 pair／roll 缺省交集、裁決同 uid 與非數字 uid。
- 實機：clean-orc 真重構可一鍵全部套用。
