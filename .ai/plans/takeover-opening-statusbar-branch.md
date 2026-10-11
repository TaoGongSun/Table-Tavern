# takeover-opening-statusbar-branch：原卡狀態欄被當成狀態寫進樹

同一根因的兩個症狀：原卡渲染出來的狀態區塊（開場白、GM 回覆裡的 `<Status_block>` YAML）被當成狀態回報寫進樹。
- **症狀 1（立案時看到的）**：接管桌之後才貼開場白，樹多出整棵 `状态栏`，與骨架並存。
- **症狀 2（審查時查出、已驗證，影響正常流程）**：正常順序是先貼開場白再重構。開場白先在樹上留下 `状态栏.*`，重構時 AI 照著這棵樹沿用 `状态栏.*` 路徑；套用時又保留舊值，AI 給的乾淨初始值整份被丟掉。

## 流程事實
- UI 流程是：匯入 → 開場白挑選框（modal，取消後沒有入口補貼）→ 之後才能在世界書編輯器重構。重新重構（`refactor/reset.rs` 183-211）也是先重貼開場白、再重構。
- 「重構後才貼開場白」只有兩種情況：已重構的桌再匯入第二張卡，或測試直接呼叫 `post_opening`。立案時的實測屬後者：`rt/root` 的「NH接管」桌先按「先不要」，重構完才呼叫 `post_opening`。

## 根因（兩個症狀都已驗證）
1. **套用端不分桌型**：`extract_state_block` 只要標籤名是 status 開頭就收，把 YAML 攤成欄位對。`apply_block_typed` 對這些欄位逐一 `set_tree_value`，樹上沒有的路徑就直接長出來。
   - 接管桌的狀態契約（`gm_turn_format` → InterfaceTakeover：只走 `<UpdateVariable>`、不重印狀態區塊）沒有落到套用端。
   - 開場白走 `append_opening_keyed`，GM 回覆走 gm_narrate → `apply_gm_block`。
2. **非 MVU 桌拿「解析出來的渲染輸出」當權威樹**：
   - 44db586 之後，盤點、absorb、合組、介面展開、展開段落五處都附 `table_tree_section`，並要求路徑逐字沿用（`prompt_common.rs` 195-197）。
   - `rebuild_state_fields`（`refactor/interface.rs` 236-245）遇到同名頂層鍵就保留現值。
   - 對 MVU 桌，樹是 initvar 建的真變數，這樣做是對的。對非 MVU 桌，樹只是 `<Status_block>` 解析的結果，而且解析有損：清單項剝掉 `- ` 但不編號（`response.rs` 513-516），三人塌成一人；`行动选项.选项` 清單整個消失。
3. **為何沒被折疊**：鏡像折疊（`normalize_interface_paths`）與外框（`frame.rs`）只在重構時處理產物本身。原卡 YAML 是簡體鍵、巢狀分組、一欄拼兩值，本來就跟骨架對不上別名。

### 驗證
- **零 AI（暫時性 cargo 測試，已移除）**：照正常順序匯入 `NorthHall-structure.png`、貼開場白，套用手寫的 `状态栏.*` 形狀產物（含新葉 `新葉`、三人的 `人物列表` 字串）。套用後的樹與貼開場白後的樹完全相同：
  - 新葉不存在；
  - `人物列表` 仍是只有周掌柜的分支；
  - 值帶 emoji。
- **真模型（測試通道，Sonnet 重構，`scratchpad/tosb/root`）**：照正常順序匯入、按「貼出這條」，再重構，手選「保留原卡玩法」後全部套用。
  - AI 產物（`refactor-outcome.json`）照既有樹沿用 `状态栏.*`，`state_fields` 是乾淨值：無 emoji、`人物列表` 三人字串、`行动选项.选项1..4`。
  - 套用後樹仍是開場白解析的那棵：帶 emoji、人物一人、選項不存在；11 條規則中 `选项1..4` 對不到葉子。
  - 補一則零 AI 旁白後，殼改由樹畫（截圖 `scratchpad/tosb/nh-normal-order-turn.png`）：「⏰ ⏰」「📍 📍」「🌾 🌾」重複，在場人物空白，行動選項 1-4 空白。
  - 開場那一格當下還是原卡畫面（`nh-normal-order-panel.png`），看起來正常，要等第一回合才壞。

## 改法
### 1. 接管桌套用端只認 `<UpdateVariable>`
- **判定**：重用 `data::is_interface_takeover`（`world.rs` 356）。另加一個會回錯的版本 `interface_takeover(...) -> DataResult<bool>`，`is_interface_takeover` 改成包它、讀錯當 false。
  - `chat_assembly.rs` 244 改呼叫它，不另抽第二份。
  - `GmMaterials` 載入時算一次 `takeover` 布林；`gm_turn_instruction` 與 gm_narrate 的套用都用這同一個值。
- **`apply_block_typed` 多收一個 `takeover` 參數**，所有呼叫處都要表態：
  - `append_opening_keyed` 與 `append_opening`：在提交鎖內用會回錯的版本判定，讀殼失敗就回錯（不貼）。
  - `reset.rs` 直接呼叫 `append_opening` 那條：臨時根沒有殼，判出 false，行為不變。
  - gm_narrate：用 materials 的布林，讀殼失敗當非接管，與導演指示同一判定。
  - 測試呼叫處傳 false。
- **接管桌開場白**：`fields` 與 `updates` 都不收〔模型判斷·未裁決〕。開場白的 `<UpdateVariable>` 也是照原卡結構寫的：insert 會長枝，被拒的會在第一回合誤導 GM。顯示文字照剝、`raw` 照存。
- **接管桌 GM 回覆**：`fields` 不收，`updates` 照收。
- **套用順序**：過濾放在算 `apply` 旗標（`chat.rs` 565-567）之前。接管桌 `incremental` 一律為真，旗標本來就是真，所以順序對結果沒有影響，但仍放在前面。
- **記帳與回饋分開**：
  - 開場白丟掉時：記一筆新的 `RecordKind::Ignored`。它只寫進 `mechanism-log`，`build_notes` 不收（不回饋 GM），帳本面板不計數。理由：開場白不是 GM 寫的。前端若有列 kind 的型別要一併補。
  - GM 回覆違約（接管桌、`fields` 非空）：補一筆合併的 `Rejected`，路徑記「狀態區塊」，內容「狀態區塊未套用，請以 UpdateVariable 回報變動。」，下一輪進 notes。
- **安全性**：GM 回合中途桌型不會變。`refactor_apply` 要拿整桌獨占，套用後呼叫 `world_swapped` 換世代；在途回合的 `apply_gm_block` 核對世代不符就整筆不提交。
- 未重構桌、characters 桌、沒殼的舊重構桌（含清殼後）：逐字照舊。

### 2. 非 MVU 桌的介面重構不把既有樹當權威
- **判定**：`mechanism::mvu_source(root, world_id, state) -> DataResult<bool>`。成立條件是 `incremental`（只有 `[initvar]`／`[mvu_update]` 會開），或有角色卡載入 MVU 腳本且沒被判不支援（`unsupported.is_none()`，與 `message_vars::eligible_char` 一致）。
  - 只給重構用（提示詞與套用）：讀卡失敗就回錯，不預設成非 MVU。
  - 數字欄更新策略 `resolve_numeric_update` 維持原判定（不排除不支援的卡、讀錯當非 MVU），不在本案範圍。
  - 前提「重構時桌一定是未重構狀態」靠兩件事成立：已重構桌兩個入口（AI 重構、套重構卡產物）都會先清回匯入原卡（見第 3 節）；已遊玩的桌一律擋。卡片變數模式只在 MVU 卡啟用，已涵蓋在內。
- **只有介面模式才不附樹段**：`table_tree_section(root, world_id, mode)` 在 `mode == "interface"` 且非 MVU 時回空段。
  - survey 本來就有 mode；absorb、split_group、expand_spans、expand 四個 command 多收一個 `mode`，前端傳這次 run 的 mode。
  - characters 模式維持 44db586 的行為（照附樹段）。
  - expand 本來就附開場白容器實值當初始值依據（`expand.rs` 34-40），AI 不靠樹也拿得到開場白的值。
- **套用（`refactor/apply.rs`）**：開頭讀 state 時就算出「既有樹是否權威」，之後套用會把 `incremental` 設成真，不能晚算。
  - 非權威的條件：非 MVU，而且這次真的套用介面。
  - 非權威時，`rebuild_state_fields` 以 `state_fields` 為準，不保留同名頂層鍵的現值；`filter_scope` 把套用前的樹當空，照「樹空整批不篩」走。
  - 其他情況維持 44db586 的行為。
- **undo**：沿用既有收據的快照，套用前的樹照原樣還原，不受影響。

### 3. 重構只在未遊玩的桌做〔作者裁決 2026-10-11〕
一般沒有道理玩了幾輪才重構。不分 MVU／非 MVU、重構過沒，已遊玩就擋，請玩家開新桌。網頁存檔匯入的桌天生有遊玩紀錄，一律擋；想重構就用原卡開新桌〔作者裁決 2026-10-12〕。
- **已遊玩判定**（`refactor/reset.rs` 的 `played`，改成 `pub(crate)` 共用）：符合任一項就算已遊玩。
  - `current_scene > 0`；
  - 第 0 幕有任何 `opening != true` 的事件。舊紀錄沒有 opening 欄位，照舊算已遊玩（保守）。
  - 有生成中的回合（在途回合即將成為遊玩紀錄；失敗或沒有正文就中止會解除），或已提交、還沒落檔的 GM 回覆（`has_unlanded_reply`，含中止留下的半截）。
  - 網頁存檔匯入的桌（有 `web-save.json` 旁檔）。
- **`rerun_status`**：已遊玩的檢查提到最前面，未重構的桌已遊玩也回 `Played`。
- **後端（權威）**：
  - `refactor_apply` 拿到整桌獨占後、`apply_and_record` 之前檢查。已遊玩就走 `return_assets(claimed, "refactor-played")` 立即返回，不呼叫 `world_swapped`（世代與回合紀錄不動）。
  - `reset_to_import_source` 原本就在獨占內先看 `rerun_status`，所以同樣擋得到。
- **前端（提早告知）**：
  - 兩個入口共用同一套三態閘門：`played`／`no_source` 擋下並提醒；`ready` 確認後先存草稿、`refactor_reset_to_source`，再往下走；`fresh` 直接走。每一步之後都核對請求是否已作廢。
  - 世界書編輯器的「重構」走 `runAiRefactor`：先查桌況，`ready` 就清回後再重構。
  - 套用重構卡產物走 `pickRefactorOutcome`（一處涵蓋拖入與按鈕選檔，`WorldEditor.tsx` 79／186）：在第一個 await 前固定請求世代與桌別；`played`／`no_source` 最先擋；`ready` 的確認與清回放在 `refactor_card_open` 與 `parseRefactorCardValue` 都通過之後、顯示結果卡之前（壞卡或不相符的卡不會先把桌清回）；作廢或清回沒成就釋放暫存的角色圖。
  - `applyRefactor` 收到 `refactor-played` 錯誤時，轉成 `refactorRerunPlayed` 文案。
- **文案**：沿用 `refactorRerunPlayed` 鍵，十語系改寫成「這張桌已經有遊玩紀錄，重構只能在開新桌時進行，請開新桌再重構。」提醒走既有的 `showMessage` 警告框（只有一個「知道了」），位置與現有提醒相同，不另加說明文字。

## 不做〔模型判斷·未裁決〕
- **開場白依殼反填骨架**：正常流程到不了；多卡追加時，那份 YAML 屬於另一張卡，反填反而錯。

## 測試（cargo）
- **開場白**（`append_opening` 層，開場白有收據時帶 `import_source`）：
  - 接管桌貼含 `<Status_block>`、狀態圍欄、`<details>` 狀態區塊、`<UpdateVariable>` 的開場白：樹不變、無多餘枝；事件 `text` 已剝區塊、`raw` 保留原文；`mechanism-log` 有一筆 `ignored`，notes 為空。
  - 骨架掛在 `状态栏.*` 下的接管桌：同路徑帶 emoji 的原值不覆蓋骨架。
  - 未重構桌、characters 桌、清殼後的桌：照舊收進樹（回歸）。
  - 重設重貼：維持原行為（臨時根無殼，`状态栏` 照舊長，之後重構時被重建掉）。
  - 讀殼失敗：開場白回錯，逐字稿與狀態不變。
- **GM 回覆**（經 `apply_gm_block` 的正式提交入口）：
  - 接管桌回覆混合 `<Status_block>`、狀態圍欄與 `<UpdateVariable>`：只套 patch，快照無多餘枝；notes 有一則「狀態區塊未套用」。
  - 未重構桌：照舊收。
- **判定函式**：有殼／空白殼／無殼／讀錯 × interface／characters／None。
- **非 MVU 重構**：
  - 先貼 NorthHall 式開場白（`状态栏.*` 進樹），再套 `状态栏.*` 形狀產物：樹等於 `state_fields`（新葉在、三人字串、無 emoji）。
  - 非 MVU 桌的 `table_tree_section` 為空；MVU 桌（`incremental` 或 mvu 卡）照附。
  - MVU 桌套用時，同名頂層保留現值（44db586 既有測試照過）；非 MVU 桌不篩規則。
  - undo 還原套用前的樹。
- **網頁存檔匯入桌**：`refactor_rerun_status` 回 `played`，`refactor_apply` 被拒。
- **已遊玩判定**：第 0 幕 0／1／2 則開場、只有一則一般訊息、兩則開場（未遊玩，可重構）、`current_scene > 0`、GM 已提交但未落檔、中止留下半截待落。
- **重構卡產物再套**：非 MVU 接管桌清回後再套同頂層產物，新葉與乾淨值進樹。
- **44db586 夾具**：`mechanism_filter.rs` 的 `seed_tree`、`visibility.rs` 828／916 改成真的 MVU 來源（`incremental = true`），斷言不放寬；各補一組非 MVU 介面套用的對照組。
- **mvu_source**：讀卡失敗時重構用途回錯；不支援的 MVU 卡不算。數字欄更新策略回歸：遠端載入器＋MVU 腳本、無鷹架的未重構桌仍走上游，數字 replace 照收。
- **GM 提交正式入口**：待落正文由後端代落後，事件快照乾淨、`raw` 留原文、`text` 已剝區塊。
- **已遊玩擋重構**：
  - 已遊玩桌（未重構過、重構過各一張）呼叫 `refactor_apply` 回錯，世界原封不動（state、世界書、殼、收據逐位元組相同），回合紀錄與世代不變。
  - GM 已提交但未落檔時，`refactor_apply` 與重設都被拒，回合紀錄與世代不清。
  - 未遊玩桌照常套用。
  - `rerun_status`：未重構且已遊玩回 `played`。
  - 前端：已遊玩桌按「重構」只顯示提醒、不呼叫盤點；套用重構卡產物時三態（played／no_source 擋；ready 等卡檔驗過才確認並清回；fresh 直接開卡）；壞卡不觸發清回；桌況回覆延遲期間關卡或另選檔，舊請求不開結果卡、不覆蓋；`applyRefactor` 收到 `refactor-played` 顯示提醒文案。
- `npm run verify` 12 步綠。

## 驗證計畫
1. **零 AI 重放**（測試通道，獨立 `--root`）：
   - 正常順序：匯入 `NorthHall-structure.png` → 「貼出這條」（帶收據）→ `refactor_apply` 套 `scratchpad/tosb/root` 那份真模型產物（`状态栏.*` 形狀）。
   - 核對：樹等於 `state_fields`；開場事件快照、`raw`／`text` 正確；`import-pending` 無殘留。
   - 補一則零 AI 旁白後截圖：無重複 emoji、人物三位、選項四條。
   - 另以 `TestCards/NorthHall-structure-statusbar-outcome-v2.json`（頂層形狀）再套一次對照。
2. **真模型**：同順序用 Sonnet 重構 NorthHall 一次（樹不再附給 AI），核對產物與套用後的面板；遊玩用 Haiku 跑 1 回合，看殼由樹畫是否正常。

## 已知限制
- 確認清回後、後端清回還沒完成的幾秒內再拖入一張卡，新請求可能讀到清回前的 ready，回「這桌忙」或已是 fresh 的錯誤而不開結果卡；資料不壞，再選一次即可。罕見，輕防不處理〔模型判斷·未裁決〕。
- 開場白容器標籤沒出現在條目文字時，骨架初始值取自條目範例。
- 非 MVU 桌重構時，重構前在面板手改的值、開場白 `<UpdateVariable>` 寫入的值，都以產物為準（undo 可還原）。
- MVU 桌的開場白若帶 `<Status_block>`（混合型），污染照舊。
- MVU 桌 rebuild 時同名頂層整棵保留，AI 新增的葉子會被丟（44db586 既有行為）。
- 多卡追加時，後貼的開場白狀態區塊不進骨架，GM 也看不到這些被丟掉的值。
- 接管桌 GM 若用狀態圍欄寫骨架同名欄位，也一律不收，改以回饋要求用 `<UpdateVariable>`。

## 驗證結果（2026-10-12，測試通道，claude CLI）
- `npm run verify` 12 步綠。新增測試：`mechanism/block_scope_tests.rs`、`refactor/tests/takeover_opening.rs`（開場白走 `post_opening_text`、GM 回覆走 `apply_gm_block`＋`gm_commit_apply`）、`refactor/tests/played_gate.rs`、`refactor/tests/tree_authority.rs`；44db586 夾具改成 MVU 來源並補非 MVU 對照；前端 `refactor-workflow.test.tsx` 補重構卡產物三態閘門與 `refactor-played` 提醒。
- **零 AI 重放**（`scratchpad/tosb2/root`）：
  - 正常順序：匯入 NorthHall、按「貼出這條」，再從世界書編輯器開上一輪 Sonnet 的 `状态栏.*` 產物（`scratchpad/tosb/nh-sonnet-nested-outcome.json`）並套用。樹等於產物的 `state_fields`：無 emoji、三人字串、選項 1–4 都在；開場事件快照已同步；無 `import-pending` 殘留。
  - 補一則零 AI 旁白後截圖（`tosb2/nh-fixed-turn.png`）：無重複 emoji、在場人物三位都在。
  - 同一桌已有遊玩紀錄：按「重構」與選重構卡產物，都只出「這張桌已經有遊玩紀錄…」提醒。
- **真模型**（同 root、桌 `01M4NWHC3PR9E320H83TZ6G3PT`）：
  - 正常順序 Sonnet 重構（不再附樹段）：AI 改取頂層繁體路徑，共 7 欄；套用後樹等於 `state_fields`，7 條規則都對得到。
  - Haiku 玩 1 回合：GM 只輸出 `<UpdateVariable>`，3 筆全收，0 拒收、無狀態區塊。面板截圖（`tosb2/nh-real-turn1.png`）：地點、人物動作跟著變，無重複 emoji。
