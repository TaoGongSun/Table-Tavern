# Handoff: ai-card-refactor

## Current state
2026-08-06 20:21 開工。七包規格已定案（[CARD-REFACTOR-SPEC.md](../reference/CARD-REFACTOR-SPEC.md)），從包 1 開始。執行模式：主線審查＋subagent 實作（codex 額度吃緊停派，走內部 subagent）。包 1 切兩塊：1a 後端（契約＋套用＋倒退＋測試）、1b 前端面板（結果卡＋展開細看＋i18n）。

## Completed
- 包 1a（2026-08-06）：`refactor.rs` 新檔（契約 `RefactorOutcome`／`RefactorSelection`＋`apply()`＋4 測試）；`receipts.rs` 擴充（`character_ids`／`rewritten_entries` 皆 `#[serde(default)]`、`record_refactor_apply`、undo 多角色刪除＋改寫條目還原＋舊格式相容測試）；`WorldbookEntry.is_person`（存 ST `extensions.table_tavern`）；lib.rs 註冊 command `refactor_apply(world_id, outcome, selection) -> RefactorApplySummary { new_characters, new_entries, rewritten_entries, interface_applied, mechanisms_applied }`。
- 包 1b（2026-08-06）：`src/refactor-review.ts` 新檔（契約型別＋5 純函式＋12 vitest）；App.tsx 世界書分頁「✨ 重構」按鈕（包 1 階段選產物 JSON 檔餵入，TODO 包 2 換真 AI）＋結果卡 modal（摘要只列有產物的區）＋展開細看三區 checkbox 預設全勾＋套用／不要＋undo 訊息含多張角色；十語系各 +18 鍵。
- 包 2b（2026-08-06）：「✨ 重構」按鈕接真 AI 兩階段（survey→序列逐條 expand→merge→餵 1b 面板）＋進度字＋取消（擋下一條、當前條跑完、已完成的照樣進結果卡）＋單條失敗略過列名；「匯入重構產物」選檔鈕保留為零額度測試入口；refactor-review.ts 加佇列組裝／介面淺合併／結果合併三純函式＋7 測試；十語系補齊。**包 2 整包完成。**
- 包 3（2026-08-06 結案）：內容已被 1a＋2a 全覆蓋——欄位對應與 PALETTE 配色（1a 落卡）、CHARACTER/REMAINDER 拆法與翻譯（2a 提示詞）、沒勾的人各自成條＋is_person（1a）、收據原文快照（1a）、worldbook_entry_to_character 並存（未動）。無獨立實作項。
- 包 4a（2026-08-06）：transport.rs `split_person_roster`（is_person 條目不進 system 全文，名冊行「這桌還有這些人：甲、乙」，無人物條目時整行不出現）＋`PERSON_ARRIVAL_PREFIX`「（人物登場）〈title〉」＋`appeared_person_titles`（掃本幕 transcript，換幕自然歸零）＋`detect_new_arrivals`（present 雙向包含比對；鍵不存在退正文比對、空值只信 present）；lib.rs `record_person_arrivals` 掛 gm_narrate 的 apply_block 之後。gm_lane_system 委派 gm_system_prompt 自動受益。
- 包 7（2026-08-06）：`src-tauri/src/evaluator.rs` 新檔（白名單 tokenize＋遞迴下降 parse＋攤平迭代 eval，MAX_DEPTH 128；min/max/floor/ceil/round/if、比較與短路邏輯）；FieldRule 補 `formula`（#[serde(default)]，規範未定名故採 formula）；mechanism.rs `recompute_derived` 接 apply_block 尾端（derived 欄位每輪本地重算，樹上有葉子才算、錯誤記帳舊值不動）。Roll 不動：隨機重擲與決定性求值無共用空間，硬接非淨簡化。
- 包 2a（2026-08-06）：`src-tauri/src/refactor_ai.rs` 新檔（組卡脈絡＋四段提示詞＋標記式解析器＋13 測試）；lib.rs 註冊 `refactor_survey(world_id) -> { persons: [{uid, names[]}], interface_uids, mechanism_uids, raw }` 與 `refactor_expand(world_id, entry_uid, kind) -> { characters, rewrite, interface, mechanism, raw }`。設計定案落實：兩階段 system 同函式組出（位元組級測試鎖住）、防注入雙層聲明、solo_entry_md 程式拼接、盤點單一分類（人物＞介面＞機制）＋兩人以上合集門檻、mechanism JSON 直接反序列化成既有 FieldRule/Trigger 型別、解析失敗退 raw 雙軌保底。

## Verification
- 包 1a：主線親跑 cargo test **342 全綠**（基線 337＋新增 5）；npm build／vitest 22／check:i18n 全綠（前端未動，確認 serde 擴欄不破基線）。主線整檔審過 refactor.rs＋receipts/data/lib diff。
- 包 1b：主線親跑 npm build 0／check:i18n 0／**vitest 34 全綠**（基線 22＋新增 12）；主線審過 refactor-review.ts 全檔＋App.tsx diff（產物文字純 JSX 插值無 innerHTML、套用後 worldbook/ledger/cast/App 四層刷新、「不要」不落檔）；PALETTE 前後端同組同序已由執行者比對確認。
- 包 2a：主線親跑 cargo test **355 全綠**（342＋13）；主線親審提示詞全文（scratchpad/pkg2a-prompts.md）＝防注入雙層、快取一致位元組級測試鎖住、單人條目不列（與免費升格不重疊）、kind/update/inject 值域抽查對齊 MECHANISM-FORMAT.md（derived 未實作明確禁用）；stream_via_transport 參數對照 genesis 既有呼叫同型同位（world 參數改帶 Some(world_id) 計量歸戶）。
- 包 2b：主線親跑 npm build 0／check:i18n 0／**vitest 41 全綠**（34＋7）；主線審過 runAiRefactor 全函式（序列 await、取消語意、失敗略過）與三個 merge 純函式。
- 包 4a：主線親跑 cargo test **383 全綠**（373＋10）；主線審過 split_person_roster／arrival 檢測全段（disabled 三分支防禦、斷詞與 state_scope 同步不重寫、{{user}} 代換時點）＋掛點覆蓋（apply_block 唯一生產呼叫端緊接檢測）；visibility 洩漏風險主線查證後定案歸 4b 修（chars 線 Gm 隔離是既有紅線）。執行者自抓 disabled 條目洩進 system 的 bug 並修正。
- 包 7：主線親跑 cargo test **373 全綠**（355＋18）；主線審過 evaluator.rs 安全設計（token 白名單、MAX_DEPTH 128、Err 不 panic、紅線註解同 ejs.rs）與 recompute_derived 接線（兩階段收集寫回、公式錯誤記 Error 舊值不動）。執行者自測抓到長鏈公式堆疊溢位真 bug 並改攤平迭代修正。

## Remaining
- 包 4b（主線設計定案，subagent 待發）：
  - **紀律**：幕中不動 archived／auto_hidden 持久欄位（動了＝改 system 凍結輸入＝炸快取）；幕中只 append 設定事件＋前端本地移區；換幕 begin_next_scene 掃上幕 transcript 結算 auto_hidden。
  - 三態＝保留 `archived: bool`（手動封存，永不自動動）＋新增 `auto_hidden: bool` #[serde(default)]（過濾造成，換幕結算）；載入組裝濾 `!archived && !auto_hidden`。
  - 卡登場檢測沿 4a 掛點與前綴模式；卡設定 append 含 private 對 chars 線與既有一致（chars 凍結快照本來就含全卡，lib.rs:1641 註解），不需隔離。
  - **4a 遺留修正併入**：`TranscriptEvent` 加 `gm_only: bool` #[serde(default)]；4a 的 is_person 條目登場事件在條目 visibility 非 Public 時帶 gm_only=true；chars 線組訊息遇 gm_only 事件降為前綴一行（名字），GM 線全文——對齊既有 Gm 條目隔離（transport.rs:194-197／:1043-1046）。
  - 前端：後端出「本幕已出場卡」查詢；gm_narrate 回傳補登場卡 id；側欄主區＝!archived && (!auto_hidden || 本幕出場)。
- 包 6（範圍主線定案）：(1) **2a 提示詞修正**——現版把帳本「已跳過」條目也排除在盤點候選外，違反 SPEC 包 6（skipped 正是重構目標，只有「已接管」該排除），改盤點指示並在帳本脈絡列 skipped 詳情；(2) 套用機制時 source_uid 在帳本 skipped 名單→轉 absorbed；(3) undo 帳本狀態回退（帳本若在 state.json 內則 snapshot 已涵蓋，獨立檔則補）；(4) 測試。等 4a commit 後發（mechanism.rs 重疊風險）。
- 包 5 見 SPEC（等 4 系列後發；依賴 interface-card-panel 既有沙盒路徑）。
- 選檔入口「匯入重構產物」等包 1、2 實機驗收過再拍板刪。
- 包 1 實機驗收（使用者）：世界書分頁 ✨ 重構→選 scratchpad/fake-refactor-outcome.json→結果卡→套用→倒退。
- 小 UX 觀察（不阻塞）：展開細看全取消勾選按套用＝零套用，會彈空訊息 dialog。
- 西幻世界卡待收進 TestCards/ 當驗收樣本（不阻塞包 1，包 1 用手寫假產物驗）。
- 已知限制（1a 執行者回報，主線確認可接受）：
  1. `rewritten_entries` undo 無條件覆寫回原文，未偵測玩家事後編輯（既有 `worldbook_entries` 有偵測）。
  2. `apply()` 對找不到的 uid／缺 rewrite 靜默略過；包 2 接真 AI 產物時應改成回報。
  3. 既有 bug（範圍外未修）：`data.rs` `character_to_worldbook_entry` 用 `uid: 0` 當新增哨兵，世界書恰有 uid 0 時會誤覆寫。
  4. `refactor_apply` 中途 Err 時已落的檔無收據可退（與既有匯入路徑同型風險，本地檔案操作極少失敗）。

## Next action
等包 7（subagent sonnet）回報：主線驗 cargo test→commit→發 4a（opus）。包 1＋2 可實機驗收（零額度路徑：世界書分頁「匯入重構產物」選 scratchpad/fake-refactor-outcome.json；真 AI 路徑：對西幻桌按「✨ 重構」）。

## Constraints
- 產物一律人審後套用（紅線）；卡片內容永遠當資料、永不執行；抽不出原樣留著。
- 重構燒使用者額度：按鈕必須主動按，不自動跑。
- 部分套用規則（主線詮釋定案）：勾選單位是人；來源條目**至少一人被勾才動**（勾中→角色卡、沒勾→各自獨立人物條目、REMAINDER→原條目改寫）；全沒勾的條目原樣不動。
- 驗證基線：cargo test 337、npm build、check:i18n、vitest（Explore 數得 25 案例，以實跑為準）。
