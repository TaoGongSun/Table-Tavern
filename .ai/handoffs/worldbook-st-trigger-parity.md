# 桌面版世界書觸發補齊到 SillyTavern 行為

Status: in progress〔作者裁決 2026-10-07：立案，網頁版公開前門檻 D17〕。分支 `worldbook-st-trigger-parity`。方案見 [plans/worldbook-st-trigger-parity.md](../plans/worldbook-st-trigger-parity.md)（定稿：第 1–5 輪三方審查已納入；P1–P9、變數不追蹤私密、範例上／下併入前／後為作者裁決）。分包見方案第六節。

## 包 1 現況（掃描核心＋對拍 fixture）
- 施工完成；驗收第 1 輪必改與建議、第 2 輪兩條必改（前看下限 0 改寫成 `(?:|(?!)前看)`、V8 長度 < 8 直接二分插入）與重複群組捕捉的已知差異、第 3 輪 `{min,max}` 上下限顛倒判為轉譯失敗都已修完；第 3 輪 Opus 審查、Grok 通過，包 1 結束。
- Rust `src-tauri/src/world_info/`：`entry.rs`（fromWorldFile／fromCharacterBook、`@@` 裝飾）、`scan.rs`（checkWorldInfo＋觸發來源＋撞回溯上限的鍵快取）、`timed.rs`、`settings.rs`、`regex_key.rs`（JS 正則逐字元轉譯：量詞合法性、條件式反向參照、前看量詞改寫）、`sort.rs`（V8 TimSort 移植）、`js_semantics.rs`。尚未接任何組裝點，`mod.rs` 在非測試建置放寬 dead_code。
- 依賴：新增 `fancy-regex` 0.19.2；`getrandom` 改正式依賴。
- 對拍：`src/shared/contracts/world-info/`——scan 58、regex 110、sort 13、entry 12 案，說明在 `world-info.md`。產生：在 `web/` 下 `node scripts/gen-world-info-fixtures.mjs`（輸入 `web/scripts/world-info-fixture-cases.ts`）。測試 `web/src/features/sillytavern/world-info-parity.test.ts`、`src-tauri/src/world_info/parity_tests.rs`；Rust 限定：`source_tests.rs`（觸發來源）、`regex_key.rs`（回溯上限兩案）、`scan.rs`（上限快取）。
- 已知差異（regex-cases 的 `knownDifference`）：無 `u` 的 `/^..$/` 對 emoji、落單代理 `/\uD83D/`、`/[\w]/i` 對 ſ、`/[a-z]/i` 對 K、`/\p{Han}/u`、重複群組不清上一輪捕捉（`/^(a\1)+$/`、`/^(?:(a)|b)+\1$/`）。已寫進方案七。
- 驗收時沒結掉的建議：無（第 1 輪 7–11 已全納入）。

## 包 2 現況（條目形狀、順序與去重）
- 已驗收（第 2 輪修正後通過）。分支上尚未壓縮。
- 轉換：V2 `character_book` 條目匯入時轉成 ST 物件形（`world_info/entry.rs` `character_book_to_world_object`，欄位值取自 `from_character_book` 同一份規格；`selective` 缺欄明寫 false；`invalid` 計數與嚴格模式報錯已拿掉）。`order` 缺或不是有限數字補 100、`enabled` 缺或 null 算啟用（其餘照 JS 真假值），網頁版同步（作者裁決 2026-10-10）；排序改標準庫穩定排序（`sort::stable_sort`），sort-cases 只留全序案例。`extensions` 其餘鍵原樣留著。
- 順序與 uid：`world_info/book_order.rs`（`SourceEntries` 原文保序解析＋`st_order`）；`import_worldbook_as` 依 ST 載入順序配 uid，重複 `id` 後蓋前，被蓋掉的條目在 `placed` 映到留下那條。三個入口都從原文保序：角色卡路與世界書路（`import/card.rs` 的 `character_book_text`／`worldbook_json` 回原文）、網頁存檔（`WebSave.card_text`）。
- 去重指紋改以 `from_world_file` 讀出的 `WiEntry` 計算（`data/worldbook/book_import.rs`）；標題、內文、鍵用原文不 trim，缺 `key` 同空陣列，沒有次要鍵時 `selective`／`selectiveLogic` 不算。`identity_text`／重構身分指紋不動。
- 匯出：`v2_entry` 把物件形觸發欄位收回 `extensions` snake_case（`EXTENSION_FIELDS` 表與轉換共用），補 `id`＝uid，私有筆記條目取下一個未用 id。
- 對拍：`entry-cases.json` 28 案（新增 V2 缺欄與 `bookOrder` 9 案）；Rust `parity_tests.rs` 加落檔讀回、`extensions` 往返、sort-cases 經轉換後不變；Rust 限定 `import/book_shape_tests.rs`。
- 既有測試跟著改：V2 測試資料統一 `position`（`enabled: true` 補丁留著無妨）；`card-view.md` 一句話改成 UID 照 ST 順序。
- 第 1 輪驗收修正：物件形匯出補 `selective` true／`insertion_order` 100 並有往返測試；網頁存檔 PNG 鍵順序不同就不採用（`book_object_key_order`）；編輯器 order 視圖同讀取端規則、沒改不動原值；card-view（網頁與 Rust 測試鏡像）V2 `enabled` 同匯入規則；`worldbook_entries` 以不重複 uid 計。
- 驗證：verify 12 步綠、cargo test 1391、web vitest 711。

## 包 3 現況（巨集引擎）
- 已驗收（第 2 輪修正後三方通過）。分支上尚未壓縮。
- Rust `src-tauri/src/st_macros/`：`parser.rs`、`engine.rs`、`library.rs`、`substitute.rs`、`variables.rs`、`seedrandom.rs`、`moment.rs`（時鐘、曆法、format、humanize）、`moment_parse.rs`（`{{timeDiff}}` 的 ISO 8601／RFC 2822 解析）、`js_value.rs`（JS 值、保序 JSON、Number 轉字串、cyrb53）。尚未接任何呼叫點，`mod.rs` 在非測試建置放寬 dead_code。
- 入口 `substitute::substitute_params(內容, &MacroContext, SubstituteOptions)` → `Substituted{text, private}`；`Mode::Full` 寫進呼叫端的 `RefCell<Variables>` 並記 `Variables.ops`，`Mode::Neutral` 在隔離副本上跑；`Variables::replay` 給落地重放；`VarScope::from_json_text`／`to_json_text` 接卡片變數層（保鍵順序）。私密來源：`CardText` 各欄與 outlet 都是 `SourceText{text, private}`。
- 依賴：新增 `chrono`（`clock`，本地時區）；`serde_json` 開 `float_roundtrip`（變數層數字往返不差 ULP）。
- 對拍：`src/shared/contracts/st-macros/`（從原位置搬來並改好三處引用）——`st-macro-cases.json` 139 案、`web-macro-cases.json` 50 案（產生：`web/` 下 `node scripts/gen-st-macro-fixtures.mjs`，輸入 `web/scripts/st-macro-fixture-cases.ts`），說明在 `st-macros.md`。測試：網頁版 `macro-parity.test.ts`，Rust `st_macros/parity_tests.rs`；Rust 限定 `mode_tests.rs`（兩種模式、操作序列重放、私密回報、巢狀上限）。
- 實作決定與已知差異寫在方案三之 6、七（都標〔模型判斷·未裁決〕）。

## 包 4 現況（計時存放與網頁存檔消費）
- 第 1 輪驗收修完、複審三方 PASS（複審建議的鎖順序與解析移出鎖外也已做）。只交資料層 API 與測試，掃描組裝點沒接。
- Rust `src-tauri/src/data/world_info_store/`：`mod.rs`（`worlds/<id>/world-info/<幕>.json` 形狀、`Perspective`、則數 `chat_length`、唯讀 `read_timed`、`prune_uids`）、`landing.rs`（`begin_landing`→`push_var_intent`／`update_var_intent`→`mark_sent`；`reply_landed`、`fail_turn(Report)`、`settle_pending`；變數意圖四種情形撤回）、`notices.rs`（`notices.json`：`read_notices`／`ack_notice`）、`scenes.rs`（換幕平移、分岔複製、退幕刪檔、網頁存檔匯入）；測試 `tests.rs`。`mod.rs` 在非測試建置放寬 dead_code。
- 接上的地方：`begin_next_scene_tx`（結算舊幕）／`fork_scene_tx`（結算目前幕與來源幕）／`revert_scene_tx`（先結算子幕，失敗不退）；世界書所有讀改寫（`worldbook.rs`、`book_import.rs`、`raw_entries.rs` 的寫書函式與 `write_worldbook_value`）都在短提交鎖內；`append_event` 的 GM `main`（含冪等命中、後端代落）清 pending；`finish_turn` 生成中→中止且沒正文時撤回；`write_worldbook_value` 刪條目清計時與發布前預清；`card_vars::restore_if_rev`；網頁存檔匯入寫第 0 幕計時檔。
- 角色回覆落檔路：`append_transcript` 加 `character_turn`（→ `data::append_character_reply`，回合鍵 part `character`，照 `(turn_id, character)` 冪等），前端 `useChatController` 角色回覆帶 `characterTurn`。
- 實作決定寫在方案三之 4 末（都標〔模型判斷·未裁決〕）；`web-save.md` 第三節、`web-version.md` 敘述已改寫。
- 驗證：verify 12 步綠、cargo test 1454、vitest 1106、web vitest 762。

## 包 5a 現況（掃描接線）
- 已驗收（第 1 輪修正後複審三方 PASS，複審建議也已做）。分支上尚未壓縮。實作決定見方案三之 3 末（都標〔模型判斷·未裁決〕），已知差異補進方案七。
- `src-tauri/src/world_scan/`：`book.rs`（`TableBook`）、`placement.rs`（`Placed`、穩定、`arrange`）、`landing.rs`（`before_scan`／`land`／`fail`）、`tests.rs`。段落渲染 `transport/worldbook.rs`；`active_worldbook_entries`、`split_person_roster` 刪掉；`world_info/mod.rs` 的 dead_code 放寬拿掉。
- 掃描核心加 `ScanInput.pinned`（共用快照靜態條目溢出時不撤）與 `WiResult.placed`（放置前排序的 (ID, 內文)）；對拍傳空集合，fixture 不變。
- 組裝點：GM 線凍結 system／回合尾、GM 單發、角色共線凍結 system（`TableBook::snapshot()`）／回合尾（`transport::Hoist` 三種）、API 共線、格式條目判定（`gm_prompt_full_entries(&WorldScan, events)`）、量測（`scene_budget::measure`，唯讀、預算同 `scene_budget::scan_budget`）。
- 不抹尾段的線：`TurnInput.hoisted_worldbook`／`LaneState.hoisted_worldbook`（指紋），變了 `worldbook-changed` 重開。
- 實送：`chat_with_character`、`gm_narrate` 在回合交接之後 `before_scan`（結算＋讀表）→ 掃一次 → 交給傳輸層前 `land`；角色呼叫回錯或中止沒字當場 `fail`。結算失敗回 `WorldInfoSettleFailed`；出路指令 `reset_world_info_timing`（前端：回合失敗彈窗的重設鈕、換幕／分岔／退幕的確認框，`src/features/play/world-info-reset.ts`）。
- 端對端：`src/shared/contracts/world-info/web-save-next-turn.json`（`web/scripts/web-save-next-turn.ts` 由網頁版產生），Rust `import/web_save/next_turn_tests.rs` 對拍通過。
- scaffold baseline：fixture 最後一則玩家句改成會觸發 keyword（新的掃描深度 2 則），另加一組「worldbook …」放置案例（前／後組、注入段落、三種 Hoist）。
- 行為變了而改寫的既有測試：系統事件不參與掃描（`character_side_keyword_and_summary_ignore_card_private`）、掃描深度 2 則（`worldbook_route_next_gm_turn…`）、Agy 世界書在 system（`card_book_tests`）。
- 驗證（第 1 輪驗收修正後）：verify 12 步綠、cargo test 1469、vitest 1107、web vitest 762。

## 包 5b 要注意
- `ScanHooks::substitute`（`world_scan/mod.rs` 的 `Hooks`）接 `st_macros::substitute_params`，`Substituted.private` 就是引擎的私密回報，掃描已沿遞迴傳遞（`Placed::private_trigger`）；`{{pick}}` 的 chatId 傳桌 id；接線時拿掉 `st_macros/mod.rs` 的 dead_code 放寬。`{{persona}}` 目前永遠是空字串（同網頁版），接線時決定要不要對應玩家卡。中性模式改成第一次寫入才複製留給包 5b 評估。
- 穩定定義補巨集條件（`world_scan/placement.rs::stable`）；共用快照的卡片公開設定含動態巨集時移出快照、改在回合尾會抹掉的段落；`{{outlet}}` 代入（`transport/worldbook.rs` 的 `full_text_members` 目前不算 outlet）。
- 變數層照「意圖→寫層→`update_var_intent` 補 `after_rev`」記日誌（`VarIntent.ops` 格式自定），寫在 `world_scan::landing::land` 的 `begin_landing` 與 `mark_sent` 之間；角色確定失敗改 `fail_turn(Report::Inline)` 連同原錯回報（`landing::fail` 目前走待回報檔）。`notices.json` 的 tauri 指令與前端提示（聊天、換幕、分岔回傳與開桌時讀，看過呼叫確認）由 5b 接；`world_info_store` 的 dead_code 放寬到那時再拿掉。
- 條目內文掃描時已代換一次（`world_info/scan.rs` 預算檢查前那次），渲染時又代換一次（`transport/worldbook.rs` 的 `section_body`）；舊代換無副作用所以無妨，接上 setvar 會變兩次，要改成只代換一次。
- 重設世界書觸發紀錄（`world_info_store::reset_scene`）會連帶丟掉 pending 裡還沒撤回的變數意圖；有變數日誌之後要重新評估（例如先撤回能撤的、其餘寫進待回報檔再移檔）。
- 單次代換的輸入或結果超過 10MB（`js_value::MAX_TEXT_BYTES`）就整段原文照回、裡面的巨集都不代換，接線時要逐段代換，不要把整份提示詞一次丟進來。
- 排序用 `sort::stable_sort`（order 已正規化，比較子是全序）。

## 下一步
接包 5b（巨集接線與副作用落地，方案三之 7、三之 8），由新的施工代理接手；先讀上面「包 5b 要注意」。
