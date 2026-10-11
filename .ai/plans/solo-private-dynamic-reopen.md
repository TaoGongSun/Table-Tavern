# solo-private-dynamic-reopen 方案

## 一、依據
- GM 線凍結 system 一變就整線重開，Claude 不再補丁疊加〔作者裁決 2026-10-11〕；該裁決原文寫「比照 `Hoist::All`／Grok」，前提是不抹尾段的線本來就一變就重開。
- 不抹尾段的線（Agy 一角一線、單人在場 Claude）本輪世界書全在 system、世界書一變整線重開〔作者裁決 2026-10-11，worldbook-st-trigger-parity 七〕。
- 本案把這個前提補成真的：單人在場 Claude 線的凍結 system 任何變動都重開，不只世界書段。
- 私設含每輪會變的巨集時每輪重開，比照 GM 線〔作者裁決 2026-10-11〕。

## 二、掛點
- 組裝：`commands/chat.rs` 角色回合 → `lanes::chars_lane_shape`（Claude＋單人在場＝`hoist_private`、`single_owner`、不抹、補前綴）→ `chat_assembly::character_lane_parts` → `transport::chars_lane_turn(Hoist::All)`。私設 `own.private`（`world_scan/prepare.rs` 以本人視角、`Randomness::Live` 每輪代換）接在凍結 system 後。機制不進角色線 system（`chars_lane_system` 不收 mechanism），只進尾段狀態區塊。
- 指紋：`character_lane_parts` 把 `hoisted_worldbook` 換成 `system_worldbook`（共用快照＋本輪世界書＋動態公開設定），只供帳本分辨 `worldbook-changed`；整份 system（含私設）由 `plan_turn` 比對。
- 比對：`lanes/mod.rs` `plan_turn`。
  - `WorldbookChanged`：指紋不同 → 重開。
  - `provider != Claude || lane == Gm || single_owner`：`applied != frozen_system` 或 `snapshot != applied` → `SystemChanged`。
  - 只剩 Claude 角色共線（每輪抹）走快取內補丁、過期追平。
- 清理：`run_turn` 重開前，舊線 provider 是 claude 且（GM 線、舊線 `unerased_owner` 有值、本輪 `single_owner` 有值、或原因是 `TailWorldbook`）時 `retire_claude_session`；迴圈內 `ResumeFailed` 降級在 GM 線或本輪 `single_owner` 時清。
- Agy 一角一線（同屬 `Hoist::All`）：本來就走 `provider != Claude` 那一路，system（含私設）一變就 `SystemChanged`，本案不動。Agy 的對話存在 Agy 自己那邊，app 沒有任何 Agy 線（含 GM）的清理路徑，本案不加〔模型判斷·未裁決〕。

## 三、做法
1. **單人在場 Claude 線 system 一變就重開**：`plan_turn` 的「一變就重開」條件加上 `input.single_owner.is_some()`（`run_turn` 已把非 Claude 角色線的 `single_owner` 歸零，等同「Claude 的 `Hoist::All` 線」）：
   - `applied != frozen_system` → `Reopen(SystemChanged)`，快取存活或過期都一樣（過期時重開與 rebase 成本近似）。
   - `snapshot != applied` 也重開：清掉升級前快取內補過丁、尚未追平的舊單人線（同 GM 做法）；已 rebase 過的線歷史裡可能仍有舊補丁，不另辨識、不強制重開（發佈前不保舊桌）〔模型判斷·未裁決〕。
   - 判定位置在 `WorldbookChanged` 之後、`tail_reopen` 之前，原本的未抹線三項（`OwnerChanged`／`ModeChanged`／`StateBlockGone`）順序不動。
   - 從共線轉成單人（舊線 `unerased_owner` 為 None、本輪 `single_owner` 有值）：私設一進 system 就重開，不再把私設當補丁貼進舊共線歷史；有共用快照或本輪觸發世界書時原因通常是 `worldbook-changed`（舊共線沒有世界書指紋），沒有世界書與快照時才是 `system-changed`。被取代的舊共線檔一併刪。
2. **不把私設另算指紋**：條件改成整份 system 比對後，私設、卡任何變動都涵蓋，不必再拆私設指紋。`WorldbookChanged` 保留，只為帳本原因較精確（世界書變就記 `worldbook-changed`，其餘記 `system-changed`）〔模型判斷·未裁決〕。
3. **單人線重開時刪被取代的舊 Claude session 檔**：沿用 `retire_claude_session`（先從 store 拿掉並落檔，落檔成功才刪，刪不掉記 `cleanup-failed`）。條件擴成「舊線 provider 是 claude，且（`lane == Gm`、或原因是 `TailWorldbook`、或舊線 `unerased_owner` 有值、或本輪 `single_owner` 有值）」，涵蓋所有重開原因；迴圈內 `ResumeFailed` 降級的清理條件同樣加上 `input.single_owner.is_some()`。
   - 舊線 `unerased_owner` 有值的檔只含同一角色自己的私設，刪掉沒有資料風險；`PendingRewrite`（前綴改寫中途斷掉）的檔照樣刪，store 已不再指向它。
   - 共線（每輪抹）因其他原因重開仍不刪，維持現況〔模型判斷·未裁決〕。
4. **帳本**：不加新原因；私設變動記 `system-changed`，單人線不再出現 `patched`／`rebased`。
5. **不動的部分**〔模型判斷·未裁決〕：
   - Claude 角色共線（`Hoist::None`，每輪抹機密段）的補丁／rebase 照舊：私設在尾段機密段、回合後抹掉，system 只剩共用快照與靜態公開設定，變動只來自改卡、改世界書這類低頻操作。
   - Grok 角色線本來就一變就重開；API／codex 單發無狀態，不動。
   - 不做「動態私設只求值一次、凍到下次重開」：與 ST 每輪重新求值不同，而且要另立凍結值的存放與失效規則。
6. **範圍外已知問題**〔模型判斷·未裁決〕：回合後改寫失敗（`rewrite-failed` 刪 key）與開線失敗（刪 key）兩條既有路徑仍可能留下殘檔，本案不動。

## 四、成本後果
- **私設不含會變的巨集（含只有 `{{user}}`、`{{char}}` 這類靜態巨集）、其他 system 素材也沒變的桌**：凍結 system 逐輪相同，送出內容與現在逐字相同，零影響。匯入的 ST 卡私設只收備用開場白（`import/card.rs` `private_markdown`），絕大多數屬此類。
- **改卡**：從「快取內補丁、過期追平」改成重開一次；低頻，成本與一次追平近似。
- **私設含每輪會變的巨集**（`{{random}}`、`{{roll}}`、`{{time}}`、`{{lastMessage}}` 等）：每輪重開，整段歷史每輪重寫快取（1 小時快取寫入 2 倍單價），不再吃歷史的讀快取（0.1 倍）。這類桌的角色線歷史部分輸入成本約變成原本的二十倍上下；量級與同類情況現況一致——同一條線的公開設定含這類巨集時已經每輪重開（動態公開設定在世界書指紋裡），GM 線的 world.md 含這類巨集時也已每輪重開。換來的是模型每輪只看到本輪的私設，不會看到 N 份互相矛盾的舊補丁（行為比照 ST 每輪重新求值）。
- **只在變數改變時才變的巨集**（`{{getvar}}` 等）：變數變的那輪重開一次。
- 已裁決走每輪重開〔作者裁決 2026-10-11〕；不採「動態私設放尾段機密段、回合後抹」。

## 五、測試清單
- `plan_turn` 單元（直接換字串，不靠亂數）：
  - Claude 單人線：快取內只換私設 → `Reopen(SystemChanged)`、無補丁；過期只換私設 → 重開而非 `rebased`；`snapshot != applied` 的舊單人線 → 重開；世界書變 → 仍是 `WorldbookChanged`。
  - system 相同：TTL 內、TTL 外、正常水位推進 → `Resume` 同 session、`patch: None`、`rebased: false`。
  - Claude 共線（`single_owner` None）改卡 → 照舊補丁／rebase（`scaffold_tests.rs` `upgrading_scaffold_patches_or_rebases_claude_chars_lane` 照過）。
  - Agy 一角一線換私設 → `SystemChanged`（既有行為，補一條守住）。
  - 原因優先序（`tail_threshold.rs` 一帶）：單人＋只換 system → `system-changed`；單人＋同時換世界書 → `worldbook-changed`；`system-changed` 先於 `tail-worldbook`；`owner-changed`／`mode-changed`／`state-block-gone` 先於 `worldbook-changed`。
- 改寫既有測試：
  - `owner.rs` `private_changes_patch_within_ttl_rebase_after_and_never_cross_speakers`：改名改說明；TTL 內替換、TTL 內移除、TTL 外換成另一份 system 都重開開新 session，送出 system 只有本輪私設、prompt 無補丁標頭；另驗 system 相同時續用；換角那段照舊。
  - `owner.rs` `hoisted_worldbook_change_reopens_instead_of_patching` 末段：別處變了改成重開（`system-changed`）；另加一輪 system 完全不變 → 續用。
  - 其他靠「凍結素材變了仍續用」的單人線測試跑過再看，有依賴就改成 system 不變。
- 假 claude 實跑（`lanes/tests`）：
  - 單人線換私設三輪：每輪新 session、帳本首輪 `first-turn`、之後 `system-changed`，無 `patched`／`rebased`；被取代的舊 session 檔已刪。第四輪 system 不變：續用、帳本無 `reopen`。
  - 共線→單人：私設實際進 system，新 session id、被取代的舊共線檔已刪、無補丁。
  - 換人、轉多人、`PendingRewrite`、Claude→其他 provider（本輪 `single_owner` 歸零，靠舊線 `unerased_owner` 判斷）重開：舊檔已刪。
  - 續聊失敗降級（`ResumeFailed`）：單人線舊檔已刪。
  - 舊檔刪不掉：本輪照常完成、帳本記 `cleanup-failed`。
  - 共線因 `history-edited` 重開：舊檔仍在（範圍守住）。
  - 落檔失敗不刪舊檔：沿用 `retire_claude_session` 既有 GM 測試（`gm_restart.rs` `gm_store_write_failure_keeps_the_old_session`）涵蓋，本案未另加。
- 走正式組裝一條（`chat_assembly::character_lane_parts`＋真掃描）：私設含每輪必變的巨集（`{{lastMessageId}}`），事件推進後每輪 `system-changed`；私設只有 `{{user}}` 時兩輪逐字相同、續用。
- `scaffold_baseline` 三語系：本案不動組裝，預期不變。
- `npm run verify` 全綠。

## 六、驗收
- 測試通道（claude 低階檔位、單人在場的非成人測試卡，私設手動加一句每輪必變的巨集 `{{lastMessageId}}`）：連跑三輪，首輪 `first-turn`、之後每輪 `system-changed`、`system_hash` 每輪不同、無 `patched`／`rebased`；`~/.claude` 專案目錄裡被取代的舊 session 檔已刪、現行檔無補丁標頭。
- 拿掉該句後再跑兩輪：第一輪 `system-changed` 重開、第二輪續聊同 hash、無 `reopen`。

## 七、施工結果
- 實作：`lanes/mod.rs` `plan_turn` 一變就重開的條件加 `input.single_owner.is_some()`；`run_turn` 重開清理條件加舊線 `unerased_owner`／本輪 `single_owner`，`ResumeFailed` 降級加本輪 `single_owner`；模組、`TurnInput::frozen_system`、`plan_turn`、`retire_claude_session` 註解就地改寫。組裝不動，scaffold baseline 未變。
- 測試：新檔 `lanes/tests/solo_restart.rs`（10）；改寫 `owner.rs` `private_changes_reopen_without_patch_and_never_cross_speakers`（原 `private_changes_patch_within_ttl_rebase_after_and_never_cross_speakers`）與 `hoisted_worldbook_change_reopens_instead_of_patching` 末段；`tail_threshold.rs` `existing_reasons_win_over_the_threshold` 補單人線原因優先序。拿掉 `plan_turn` 新條件時其中 8 條轉紅。
- verify 12 步綠；cargo test 1590、vitest 1141、web vitest 774。
- 測試通道（claude-haiku-4-5，自建非成人單卡「林教授」、單人在場角色線）：

| 輪 | 私設 | reopen | system_hash | session 檔 |
|---|---|---|---|---|
| 1 | 含 `{{lastMessageId}}` | first-turn | 5340b3c3… | 被取代後已刪 |
| 2 | 同上 | system-changed | d3f37754… | 被取代後已刪 |
| 3 | 同上 | system-changed | 2007af3d… | 被取代後已刪 |
| 4 | 拿掉巨集 | system-changed | 5ebd0e57… | 被取代後已刪 |
| 5 | 同上 | —（續聊） | 5ebd0e57… | 現行、無補丁標頭 |

  無 `patched`／`rebased`／`cleanup-failed`；專案目錄只剩現行一份 session 檔。haiku 提示太短未達快取門檻，續聊輪 `cache: zero` 與本案無關。
