# Handoff: ai-card-refactor

## Current state
2026-08-06 20:21 開工。七包規格已定案（[CARD-REFACTOR-SPEC.md](../reference/CARD-REFACTOR-SPEC.md)），從包 1 開始。執行模式：主線審查＋subagent 實作（codex 額度吃緊停派，走內部 subagent）。包 1 切兩塊：1a 後端（契約＋套用＋倒退＋測試）、1b 前端面板（結果卡＋展開細看＋i18n）。

## Completed
- 包 1a（2026-08-06）：`refactor.rs` 新檔（契約 `RefactorOutcome`／`RefactorSelection`＋`apply()`＋4 測試）；`receipts.rs` 擴充（`character_ids`／`rewritten_entries` 皆 `#[serde(default)]`、`record_refactor_apply`、undo 多角色刪除＋改寫條目還原＋舊格式相容測試）；`WorldbookEntry.is_person`（存 ST `extensions.table_tavern`）；lib.rs 註冊 command `refactor_apply(world_id, outcome, selection) -> RefactorApplySummary { new_characters, new_entries, rewritten_entries, interface_applied, mechanisms_applied }`。
- 包 1b（2026-08-06）：`src/refactor-review.ts` 新檔（契約型別＋5 純函式＋12 vitest）；App.tsx 世界書分頁「✨ 重構」按鈕（包 1 階段選產物 JSON 檔餵入，TODO 包 2 換真 AI）＋結果卡 modal（摘要只列有產物的區）＋展開細看三區 checkbox 預設全勾＋套用／不要＋undo 訊息含多張角色；十語系各 +18 鍵。

## Verification
- 包 1a：主線親跑 cargo test **342 全綠**（基線 337＋新增 5）；npm build／vitest 22／check:i18n 全綠（前端未動，確認 serde 擴欄不破基線）。主線整檔審過 refactor.rs＋receipts/data/lib diff。
- 包 1b：主線親跑 npm build 0／check:i18n 0／**vitest 34 全綠**（基線 22＋新增 12）；主線審過 refactor-review.ts 全檔＋App.tsx diff（產物文字純 JSX 插值無 innerHTML、套用後 worldbook/ledger/cast/App 四層刷新、「不要」不落檔）；PALETTE 前後端同組同序已由執行者比對確認。

## Remaining
- 包 2a（後端 AI 呼叫，subagent 實作中）：組卡脈絡（system 吃 cache_control，兩階段 system 逐字元一致）＋`refactor_survey`／`refactor_expand` 兩 command＋標記式解析器（截斷救回、raw 保底）＋提示詞（主線待親審全文）。主線設計定案：取消純前端做（不發下一條即取消）；`solo_entry_md` 由解析器拼 public＋private；盤點每條目單一分類（人物＞介面＞機制）。
- 包 2b（前端接線，等 2a）：重構按鈕從「選 JSON」換成真 AI 兩階段流程＋進度字（`盤點中…`→`整理「X」 2/4`）＋中途取消（已完成的照樣進結果卡）。
- 包 3–7 見 SPEC。
- 包 1 實機驗收（使用者）：世界書分頁 ✨ 重構→選 scratchpad/fake-refactor-outcome.json→結果卡→套用→倒退。
- 小 UX 觀察（不阻塞）：展開細看全取消勾選按套用＝零套用，會彈空訊息 dialog。
- 西幻世界卡待收進 TestCards/ 當驗收樣本（不阻塞包 1，包 1 用手寫假產物驗）。
- 已知限制（1a 執行者回報，主線確認可接受）：
  1. `rewritten_entries` undo 無條件覆寫回原文，未偵測玩家事後編輯（既有 `worldbook_entries` 有偵測）。
  2. `apply()` 對找不到的 uid／缺 rewrite 靜默略過；包 2 接真 AI 產物時應改成回報。
  3. 既有 bug（範圍外未修）：`data.rs` `character_to_worldbook_entry` 用 `uid: 0` 當新增哨兵，世界書恰有 uid 0 時會誤覆寫。
  4. `refactor_apply` 中途 Err 時已落的檔無收據可退（與既有匯入路徑同型風險，本地檔案操作極少失敗）。

## Next action
等包 2a（subagent sonnet）回報：主線驗 cargo test＋親審提示詞全文→commit→發 2b 前端接線。

## Constraints
- 產物一律人審後套用（紅線）；卡片內容永遠當資料、永不執行；抽不出原樣留著。
- 重構燒使用者額度：按鈕必須主動按，不自動跑。
- 部分套用規則（主線詮釋定案）：勾選單位是人；來源條目**至少一人被勾才動**（勾中→角色卡、沒勾→各自獨立人物條目、REMAINDER→原條目改寫）；全沒勾的條目原樣不動。
- 驗證基線：cargo test 337、npm build、check:i18n、vitest（Explore 數得 25 案例，以實跑為準）。
