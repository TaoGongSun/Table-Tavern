# Handoff: ai-card-refactor

## Current state
**2026-08-11 實測暫停：機制面跑通、產出面判定不能玩，待重設計**。orc-cave 真跑 B1–B4 一輪：重構→匯出→匯入→套用→狀態欄／帳本／殼／復原全都動作（機制面通過）；但產出品質使用者判定不合格——世界書停用墓地、規則顯示藏在世界書編輯頁（遊玩時看不到）、無介面卡硬產殼、殼把未觸發事件全列＝劇透、狀態欄位簡繁重複。七條「能玩」驗收標準已裁決，見 [refactor-output-redesign](../tasks/refactor-output-redesign.md)；重設計完成後再回來續測 C／D／E。測試水位：cargo test 428／vitest 82／npm build／check:i18n 全綠（已 commit 至 2cc2046）。

## Completed
- 玩家卡選取改為只問一位（2026-08-10 拍板）：展開細看原本每行都有「玩家卡」單選＋「不指定玩家」一列，等於任何角色都能被選成玩家——不符合多數卡預設好玩家是誰的設計。現在只有 AI 標記 `suspected_player` 的那位出現「這是我的角色」勾選（預設勾、可取消），沒人被標記就整個選項不出現（[App.tsx:2615](../../src/App.tsx#L2615)）；i18n 刪 `refactorPlayerNone`／`refactorPlayerRadioLabel`，改 `refactorPlayerCheckLabel`。結果卡標題「整理好了」→「重構完成」（十語系同步）。事後改主意的入口另立 [character-to-player-card](../tasks/character-to-player-card.md)。
- 實測第一天修正（2026-08-10）：A2 展開細看白畫面＝假產物檔停留在 person-promote 前的舊契約（單數 `source_uid`），已改寫成新契約並改成不依賴任何卡的自足測法（見待實測清單 A 段前言）；`parseRefactorOutcome` 補逐欄驗證＋`REFACTOR_IMPORT_INVALID` 訊息（[refactor-review.ts:198](../../src/refactor-review.ts#L198)），格式不對整份拒收不再炸畫面，十語系 +1 鍵；連帶修 `mergeRefactorInterfaces` 丟掉 AI 產的 HTML 渲染殼——殼從沒傳進 `refactor_apply`，B4 原本必紅（[refactor-review.ts:93](../../src/refactor-review.ts#L93)，前端 `RefactorInterface` 補 `shell?`）。
- 包 1a（2026-08-06）：`refactor.rs` 新檔（契約 `RefactorOutcome`／`RefactorSelection`＋`apply()`＋4 測試）；`receipts.rs` 擴充（`character_ids`／`rewritten_entries` 皆 `#[serde(default)]`、`record_refactor_apply`、undo 多角色刪除＋改寫條目還原＋舊格式相容測試）；`WorldbookEntry.is_person`（存 ST `extensions.table_tavern`）；lib.rs 註冊 command `refactor_apply(world_id, outcome, selection) -> RefactorApplySummary { new_characters, new_entries, rewritten_entries, interface_applied, mechanisms_applied }`。
- 包 1b（2026-08-06）：`src/refactor-review.ts` 新檔（契約型別＋5 純函式＋12 vitest）；App.tsx 世界書分頁「✨ 重構」按鈕（包 1 階段選產物 JSON 檔餵入，TODO 包 2 換真 AI）＋結果卡 modal（摘要只列有產物的區）＋展開細看三區 checkbox 預設全勾＋套用／不要＋undo 訊息含多張角色；十語系各 +18 鍵。
- 包 2b（2026-08-06）：「✨ 重構」按鈕接真 AI 兩階段（survey→序列逐條 expand→merge→餵 1b 面板）＋進度字＋取消（擋下一條、當前條跑完、已完成的照樣進結果卡）＋單條失敗略過列名；「匯入重構卡」選檔鈕保留為零額度正式入口；refactor-review.ts 加佇列組裝／介面淺合併／結果合併三純函式＋7 測試；十語系補齊。**包 2 整包完成。**
- 包 3（2026-08-06 結案）：內容已被 1a＋2a 全覆蓋——欄位對應與 PALETTE 配色（1a 落卡）、CHARACTER/REMAINDER 拆法與翻譯（2a 提示詞）、沒勾的人各自成條＋is_person（1a）、收據原文快照（1a）、worldbook_entry_to_character 並存（未動）。無獨立實作項。
- 包 5b（2026-08-07）：`src/refactor-shell.ts` 新檔（fillShellPlaceholders：`{{路徑}}` 逐層查樹、值五實體 HTML escape、查不到留空）＋7 vitest；App.tsx 切桌／套用後／undo 後三處刷新殼、cardInterfaceShell 重構殼優先路（既有 event.raw 找殼路逐字未動）、覆蓋層工具列 ⓘ 說明鈕；十語系。**包 5 全部完成＝七包齊。**
- 包 5a（2026-08-06）：RefactorInterface 加 `shell: Option<String>`；INTERFACE_BODY 補 `## SHELL` 段（自包含單檔 HTML、`{{狀態樹路徑}}` 佔位、值必 HTML escape 純文字、互動走 `window.triggerSlash`、沒把握就純展示）；殼檔 `worlds/<id>/interface-shell.html`＋command `refactor_interface_shell(world_id) -> Option<String>`；收據 `interface_shell_created` undo 刪檔（沿 world_card_created 模式，二次套用覆寫不回上一版——與既有慣例一致）。
- 包 4b2（2026-08-06）：`src/character-visibility.ts` 新檔（isCharacterHidden 判準：archived 一律隱藏；auto_hidden 卡本幕出場即回主區）＋5 vitest；App.tsx 側欄分流、enterTable 初始化 scene_appearances、narrateOnce 併入 arrived_characters、隱藏區「自動隱藏」標籤＋拉回走 set_character_auto_hidden；reorderCast 與預設發言對象統一同判準（避免誤選隱藏卡）；十語系 autoHiddenBadge。**包 4 全部完成。**
- 包 6（2026-08-06）：SURVEY_BODY 與組卡脈絡帳本段修正（absorbed＝不必再拆；skipped＝重構目標、機制候選）；apply() 機制套用即記帳本 Absorbed 一筆（skipped 靠「同標題最新一筆」語意自動蓋掉）；帳本是獨立 append-only 檔 `mechanism-log.jsonl`，收據新增 `added_ledger_lines` 快照、undo 第 5 步整段挖除（不整檔覆寫——期間新產生的遊玩紀錄不動）。既有缺口記錄：character/worldbook 匯入路徑的帳本寫入原本就不回退，非本包新增，未動。
- 包 4b1（2026-08-06）：CharacterMeta.auto_hidden 三態（archived＝手動永不自動動）＋卡登場檢測（前綴「（角色回歸）〈名字〉」，掛 4a 同點，幕中只 append 不動欄位）＋begin_next_scene 換幕結算（出現過＝回歸事件∪換幕當下 present；失敗吞掉不擋換幕）＋`scene_appearances(world_id) -> { character_ids, person_titles }`＋gm_narrate 回傳補 `arrived_characters`／`arrived_persons`＋TranscriptEvent.gm_only 洩漏修正（chars 線 System 事件遮成前綴行、GM 線全文）；主線補 command `set_character_auto_hidden`（隱藏區手動拉回用）。
- 包 4a（2026-08-06）：transport.rs `split_person_roster`（is_person 條目不進 system 全文，名冊行「這桌還有這些人：甲、乙」，無人物條目時整行不出現）＋`PERSON_ARRIVAL_PREFIX`「（人物登場）〈title〉」＋`appeared_person_titles`（掃本幕 transcript，換幕自然歸零）＋`detect_new_arrivals`（present 雙向包含比對；鍵不存在退正文比對、空值只信 present）；lib.rs `record_person_arrivals` 掛 gm_narrate 的 apply_block 之後。gm_lane_system 委派 gm_system_prompt 自動受益。
- 包 7（2026-08-06）：`src-tauri/src/evaluator.rs` 新檔（白名單 tokenize＋遞迴下降 parse＋攤平迭代 eval，MAX_DEPTH 128；min/max/floor/ceil/round/if、比較與短路邏輯）；FieldRule 補 `formula`（#[serde(default)]，規範未定名故採 formula）；mechanism.rs `recompute_derived` 接 apply_block 尾端（derived 欄位每輪本地重算，樹上有葉子才算、錯誤記帳舊值不動）。Roll 不動：隨機重擲與決定性求值無共用空間，硬接非淨簡化。
- 包 2a（2026-08-06）：`src-tauri/src/refactor_ai.rs` 新檔（組卡脈絡＋四段提示詞＋標記式解析器＋13 測試）；lib.rs 註冊 `refactor_survey(world_id) -> { persons: [{uid, names[]}], interface_uids, mechanism_uids, raw }` 與 `refactor_expand(world_id, entry_uid, kind) -> { characters, rewrite, interface, mechanism, raw }`。設計定案落實：兩階段 system 同函式組出（位元組級測試鎖住）、防注入雙層聲明、solo_entry_md 程式拼接、盤點單一分類（人物＞介面＞機制）＋兩人以上合集門檻、mechanism JSON 直接反序列化成既有 FieldRule/Trigger 型別、解析失敗退 raw 雙軌保底。

## Verification
- 實測第一天修正（2026-08-10）：主線親跑 **vitest 82 全綠**（基線 71＋新增 11：匯入拒收十例＋殼合併一例）／npm build／check:i18n 十語系 OK；假檔以新契約結構驗過（4 角色、介面 uid 3、機制 uid 4、可刪共用 uid 1）。Rust 未動故不重跑 cargo。
- 包 1a：主線親跑 cargo test **342 全綠**（基線 337＋新增 5）；npm build／vitest 22／check:i18n 全綠（前端未動，確認 serde 擴欄不破基線）。主線整檔審過 refactor.rs＋receipts/data/lib diff。
- 包 1b：主線親跑 npm build 0／check:i18n 0／**vitest 34 全綠**（基線 22＋新增 12）；主線審過 refactor-review.ts 全檔＋App.tsx diff（產物文字純 JSX 插值無 innerHTML、套用後 worldbook/ledger/cast/App 四層刷新、「不要」不落檔）；PALETTE 前後端同組同序已由執行者比對確認。
- 包 2a：主線親跑 cargo test **355 全綠**（342＋13）；主線親審提示詞全文（scratchpad/pkg2a-prompts.md）＝防注入雙層、快取一致位元組級測試鎖住、單人條目不列（與免費升格不重疊）、kind/update/inject 值域抽查對齊 MECHANISM-FORMAT.md（derived 未實作明確禁用）；stream_via_transport 參數對照 genesis 既有呼叫同型同位（world 參數改帶 Some(world_id) 計量歸戶）。
- 包 2b：主線親跑 npm build 0／check:i18n 0／**vitest 41 全綠**（34＋7）；主線審過 runAiRefactor 全函式（序列 await、取消語意、失敗略過）與三個 merge 純函式。
- 包 5b：主線親跑 npm build 0／check:i18n 0／**vitest 53 全綠**（46＋7，含主線補 undo 後刷新殼再驗）；主線親讀 refactor-shell.ts 全檔（escape 順序 & 先換、五實體齊、佔位語法保守）。
- 包 5a：主線親跑 cargo test **406 全綠**（398＋8）；主線審過 parse_interface_expand（STATE 必要／SHELL 選配／空殼 None）與殼規格關鍵句（escape 語意、triggerSlash 為墊片真名）。
- 包 4b2：主線親跑 npm build 0／check:i18n 0／**vitest 46 全綠**（41＋5）；主線審過 character-visibility.ts 判準全檔。
- 包 6：主線親跑 cargo test **398 全綠**（395＋3）；主線審過 undo 挖除段（rfind 對應最近一筆收據）與提示詞修正字面。
- 包 4b1：主線親跑 cargo test **395 全綠**（383＋12，含主線補 command 後重跑）；主線審過 settle_card_visibility（archived 保護、(a)∪(b) 判定、失敗吞掉）與 system_event_text／lane_event_line 遮蔽段。已知限制記 Remaining：邊緣誤隱藏與 revert/fork 不復原結算。
- 包 4a：主線親跑 cargo test **383 全綠**（373＋10）；主線審過 split_person_roster／arrival 檢測全段（disabled 三分支防禦、斷詞與 state_scope 同步不重寫、{{user}} 代換時點）＋掛點覆蓋（apply_block 唯一生產呼叫端緊接檢測）；visibility 洩漏風險主線查證後定案歸 4b 修（chars 線 Gm 隔離是既有紅線）。執行者自抓 disabled 條目洩進 system 的 bug 並修正。
- 包 7：主線親跑 cargo test **373 全綠**（355＋18）；主線審過 evaluator.rs 安全設計（token 白名單、MAX_DEPTH 128、Err 不 panic、紅線註解同 ejs.rs）與 recompute_derived 接線（兩階段收集寫回、公式錯誤記 Error 舊值不動）。執行者自測抓到長鏈公式堆疊溢位真 bug 並改攤平迭代修正。

## 待實測清單（新對話照此逐項勾，全過即結案）

### A. 零額度：匯入既有產物（**待 [refactor-outcome-export](../tasks/refactor-outcome-export.md) 完成**）
2026-08-10 拍板：不再用手工假產物測——手捏一份逼真的產物等於用人力重造 app 按一顆鈕就會產的東西，成本高又測不出真實情況。改成先做匯出功能，B 段真跑一次把產物存起來，A 段拿那份檔案重放。
- [ ] A1 匯入 B 段存下的產物檔→結果卡摘要與當初一致。
- [ ] A2 展開細看：三區預設全勾；角色行 emoji＋名字＋灰字出處條目名；只有 AI 認定是 `{{user}}` 的那位有「這是我的角色」勾選（預設勾、可取消）。
- [ ] A3 全部套用→結果與 B3 一致（換一張同卡新桌驗，確認產物可重放）。
- [ ] A4 側欄「復原上次匯入」→全部回原樣（角色消失、被刪條目回來、停用狀態復原、玩家卡取消、狀態欄欄位消失、帳本紀錄消失）；`worlds/<id>/refactor-outcome.json` 仍在（undo 不刪檔，2026-08-10 拍板）。
- [ ] A5 重選檔案→只勾共用合集裡的部分人→套用→合集條目因有人沒勾而整條留著、沒勾的人各自成獨立 is_person 條目→復原回原樣。
- [ ] A6 隨便選一個非產物 JSON（例如 TestCards/WestFantsy.json）→顯示「這個檔案不是重構產物」，畫面不變白（防禦已於 2026-08-10 補上）。
- [ ] A7 套用後世界書分頁「匯出重構卡」→存檔→再匯入讀回，摘要與 A1 一致；沒重構過的新桌按同鈕→「這桌還沒有重構卡」。

### B. 真 AI：orc-cave 重構（燒 GM 檔位額度：盤點 1＋展開約 8 次）
測試卡改用 **TestCards/orc-cave-copy.png**（2026-08-10 拍板，結構單純且三類產物齊備）：18 條條目，人物 6 位（利格魯德跨中英兩條＋行為模式共三條、巴古克／古茲卡各一條＋兩人共用一條劇情線、格洛克、伯恩、`{{USER}}` 玩家設定），uid 12 同時是介面來源（可收合狀態欄模板：淪陷天數／當前環境／駐留角色／劇情階段）與機制來源（天數計數＋第 7／14／21／30 天里程碑），其餘為種族與世界觀設定不該被拆。西幻卡（規模大、17 次呼叫）留作壓力測試備用。
- [ ] B1 orc-cave 桌按「✨ 重構」→「盤點中…」→「整理『X』i/n」逐條進度→結果卡（預期：6 人、1 介面、機制若干；利格魯德三條併成一張、`{{USER}}` 標成疑似玩家）。
- [ ] B2 重跑一次並中途按取消→已完成的部分照樣進結果卡。
- [ ] B3 套用→升格者的專屬條目整條消失、共用條目全勾才刪、沒勾的人各自成獨立條目、狀態欄出現淪陷天數等欄位、帳本機制條「已接管」、介面來源條目停用。
- [ ] B4 介面覆蓋層開啟→重構殼渲染（AI 有產 SHELL 時）；工具列 ⓘ 鈕點開說明；殼壞時關覆蓋層看側欄狀態欄＝保底。
- [ ] B5 GM 跑一輪→狀態樹變→介面數值跟著換新。

### C. 在場過濾與自動上下場（用 B 套用後的桌）
- [ ] C1 GM 回覆 present 含某沒升格人物名→聊天出現「（人物登場）〈名〉」系統事件（含設定全文）；同幕同人不重複。
- [ ] C2 換幕→本幕沒出場的角色卡進隱藏區（標「自動隱藏」）、出場過的留主區；手動封存卡不受自動結算影響。
- [ ] C3 隱藏區拉回自動隱藏卡→立即回主區。
- [ ] C4（開發者）看 GM 線 system／lanes 快照：is_person 條目只剩一行名冊「這桌還有這些人：…」。

### D. 機制執行期（state-values-mvu 合併驗收，2026-08-05 拍板延至本案後一起驗）
- [ ] D1 重構產的欄位規則生效：GM 報值超界被夾回、拒收欄回饋。
- [ ] D2 觸發表：數值進區間→對應文本注入。
- [ ] D3 derived 公式欄（若有產出）本地重算，模型不用報。
- [ ] D4 增量路徑煙霧：勇者卡桌便宜檔位跑幾輪（SPEC「合併實測」項）。

### E. 回歸與快取
- [ ] E1 無 is_person 條目的舊桌跑一輪：行為與從前一致（名冊行不出現）。
- [ ] E2 重構桌連跑幾輪，額度分頁快取命中維持既有水位（85–88%）。
- [ ] E3 多角色桌：GM 限定人物的登場事件在角色線只見前綴一行，不洩全文。

### 實測後拍板事項
- revert_scene／fork_scene 不復原 auto_hidden 結算：要不要補。
- 小 UX：展開細看全取消勾選按套用會彈空訊息 dialog；重構進度／失敗訊息寫 worldbookMessage，結果卡開著時看不到。

## 已知限制（主線確認可接受，非阻塞）
1. `rewritten_entries` undo 無條件覆寫回原文，未偵測玩家事後編輯（既有 `worldbook_entries` 有偵測）。
2. `apply()` 對找不到的 uid／缺 rewrite 靜默略過。
3. 既有 bug（範圍外未修）：`character_to_worldbook_entry` 用 `uid: 0` 當新增哨兵，世界書恰有 uid 0 時會誤覆寫。
4. `refactor_apply` 中途 Err 時已落的檔無收據可退（與既有匯入路徑同型）。
5. 全程在場但最後一輪 present 漏列且從未隱藏過的卡，換幕會誤隱藏——手動或劇情拉回即復原。
6. 殼檔二次套用覆寫不回上一版（與 world_card_created 既有慣例一致）；殼快取滯留已補（undo 後刷新）。
7. 單發 assemble_messages 路徑（非 lane 單角色）對 Public 的 is_person 條目仍送全文——重構產的條目都是 Gm 限定，實務不觸發。

## Next action
1. 先做 [refactor-outcome-export](../tasks/refactor-outcome-export.md)——2026-08-10 六項拍板完成（兩入口都做、undo 不刪檔、含殼、uid v1 不處理），實作發包中，規格見該案交接檔。
2. 回到本案從 **B 段**開跑（orc-cave 卡），套用前先把產物匯出存檔。
3. 拿存下的產物跑 A 段（零額度重放），再依序 C、D、E。

全過→兩個任務一起結案（狀態改 completed、TASKS.md 行搬 DONE.md、已驗收段落搬 archive/）；有紅→帶著現象回來修。

## Constraints
- 產物一律人審後套用（紅線）；卡片內容永遠當資料、永不執行；抽不出原樣留著。
- 重構燒使用者額度：按鈕必須主動按，不自動跑。
- 部分套用規則（主線詮釋定案）：勾選單位是人；來源條目**至少一人被勾才動**（勾中→角色卡、沒勾→各自獨立人物條目、REMAINDER→原條目改寫）；全沒勾的條目原樣不動。
- 驗證基線（實作完成後）：cargo test 406、vitest 53、npm build、check:i18n 全綠。
