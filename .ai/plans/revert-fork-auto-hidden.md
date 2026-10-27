# revert-fork-auto-hidden — 做法選項（待作者拍板）

## 問題一句話
換幕結算把這幕沒出場的角色卡收進隱藏區（`auto_hidden=true`）。退回前幕（revert）或從前幕分岔（fork）之後，隱藏區還是「換幕後」的樣子，不是那個時間點的樣子：GM 上下文會少掉那些卡，玩家只能自己一張張拉回。

## 重現（2026-10-27，Rust 整合測試，測完已刪）
- revert：甲、乙兩卡，幕 0 最後 present＝「甲」→ 換幕 → 乙被收（`乙=true`）→ 退回幕 0 → **乙仍是 true**（應為 false）。
- fork：幕 0 present＝「甲、乙」→ 換幕到幕 1 → 幕 1 present＝「甲」→ 換幕到幕 2（乙被收）→ 從幕 0 分岔成幕 3 → **乙仍是 true**（幕 0 當時是 false）。
- 施工時把這兩段直接寫成 `data/scene/presence.rs` 或 `lifecycle.rs` 的正式測試。

## 現況流程
- 隱藏旗標存在卡檔 frontmatter（`data/character.rs:183-197` serialize、`:389` `set_character_auto_hidden`），不屬於任何一幕、不在逐字稿裡。
- 換幕：`data/scene/lifecycle.rs:137` `begin_next_scene_tx` → `write_state` 後呼叫 `settle_card_visibility`（`data/scene/presence.rs:145`）：出現過（本幕回歸事件 ∪ 換幕當下 present 命中／歧義）→ false，其餘 → true；archived 跳過；失敗吞掉。present 缺席且這桌不要求報 present 時直接不結算（`presence.rs:152`）。
- 退回：`lifecycle.rs:219` `revert_scene_tx` 只還 `current_scene`、幕名、`state.state`（從父幕逐字稿最後一則事件的 state 取回），**沒碰卡檔**。
- 分岔：`lifecycle.rs:49` `fork_scene_tx` 複製來源幕逐字稿、改標籤、還 `state.state`，**沒碰卡檔**；離開的那一幕也不結算。
- 手動：隱藏區的「拉回」走同一支 setter 寫 false（`commands/character.rs:68`、前端 `useCharacterController.ts:285`）。儲存層分不出是結算寫的還是玩家拉的。手動「封存」是另一個欄位 `archived`，結算與本案復原都不碰。
- 誰在讀旗標：GM 上下文 `chat_assembly.rs:33/72`、回歸偵測 `commands/chat.rs:303`、介面讀取 `import/interface.rs:94`、前端側欄 `character-visibility.ts:21`。
- 前端：`useSceneActions.ts:117/133` 退回、分岔後都 `enterTable` 重讀卡片與 `scene_appearances`（掃目前幕逐字稿的回歸事件），所以後端只要把旗標還對，前端不用改；分岔複製來的回歸事件也會自動讓那些卡算「本幕已出場」。

### 既有快照機制能不能直接用
不能。`pop_transcript`（`data/scene/transcript.rs:548`）與 revert／fork 還的是事件上的 `state`（模型逐回合回報的狀態表），隱藏旗標在卡檔，兩者沒有交集。把隱藏集合塞進每則事件的 state 要動 `transcript.rs`（initvar-nonzod-overwrite 施工中）與 GM 狀態合併，而旗標本來就只在換幕時變，幕級存一份就夠。可重用的是 `WorldState` 既有「幕號字串當 key」的表（`scene_titles`／`scene_labels`，`data/state.rs:341-344`），revert 也已經在清這類 key。

## 選項

### A. 離開一幕時拍快照，回到那一幕就照快照還原（推薦）
- **做什麼**：`WorldState` 加一張表「幕號 → 卡 id → 是否自動隱藏」（false 也寫）。換幕在結算**之前**記下剛結束那幕每張卡的值，跟 `current_scene` 同一次 `write_state` 落地；分岔時記下被離開的那一幕（父幕）。不結算的桌、全員可見的幕也照樣寫 key。退回到幕 N、或從幕 N 分岔，就照表 N 把卡寫回。離開一幕只有換幕與分岔兩條路，所以每個過去的幕都會有快照。
- **玩家會看到**：退回／分岔後，側欄主區與隱藏區跟當時一模一樣；GM 上下文也跟著回來，不用手動拉。
- **手動操作**：手動拉回照實記進快照——在幕 N 裡拉回的卡，回到 N 時照樣在主區；只在被退掉那一幕（N+1）裡拉回的，會被 N 的快照蓋回去（退回只允許 N+1 只有那則前情提要，這段時間很短）。封存（archived）完全不受影響。
- **邊角**：見下方「施工要點」寫回規則。舊桌沒有快照 → 不還原、跟現在一樣（發佈前不顧舊桌）。
- **改動範圍**（實作後）：`data/state.rs` +7、`data/scene/lifecycle.rs` +15、新檔 `data/scene/visibility.rs` 60 行＋`visibility/tests.rs` 479 行、`data/character.rs`（原子寫＋註解）、`data/world.rs` +2 與 `mechanism/test_support.rs` +2（`WorldState` 字面值補新欄位；`data/world.rs` 是 gm-no-state-fence-worldbook-table 施工中的檔，合併時注意）；前端 0。
- **風險**：低。state.json 每幕多一張小表。

### B. 不存資料，回到一幕時重算當初那次結算
- **做什麼**：回到幕 N 時，拿 N 的「上一幕」逐字稿（回歸事件＋最後 present）重跑一次結算，等於重演「進入 N 那一刻」的結果；幕 0 一律全部顯示。
- **玩家會看到**：大多數情況跟 A 一樣。
- **手動操作**：幕 N 裡手動拉回的全部遺失（重算只得到 N 開頭的狀態），玩家要重拉。
- **算不對的情況**：①不要求報 present 的桌（介面接管桌、沒重構的顯示腳本桌）當初根本沒結算，重算無從得出當時值；②比對靠現在的卡片清單與世界書人物條目，中途刪卡／改條目就會算出不一樣的結果；③分岔幕的「上一幕」是分岔當下所在的幕，不是來源幕，從分岔幕再分岔或退回時要另外追來源，容易錯。
- **改動範圍**：`lifecycle.rs` 約 +15、`presence.rs` 約 +10、測試約 +60；不碰施工中檔案、不改存檔格式。
- **風險**：中，上述三種情況會悄悄還原成錯的樣子。

### C. 拿掉持久旗標，隱藏改成每次讀取時從逐字稿推導
- **做什麼**：不再在卡檔存 `auto_hidden`，「這張卡現在該不該藏」每次從目前幕的來源鏈推導；手動拉回改記在幕層。
- **玩家會看到**：跟 A 一樣，且以後任何回溯操作天生正確。
- **手動操作**：要另設幕層的手動紀錄，規格要重新拍。
- **改動範圍**：大。要改所有讀旗標的地方：`chat_assembly.rs:33/72`、`import/interface.rs:94`（兩支都在 gm-no-state-fence-worldbook-table 施工中）、`commands/chat.rs:303`、`commands/character.rs` 凍結快照、前端側欄判準與拉回，另加推導本體。
- **風險**：高，跟施工中的案子直接撞檔，回歸面大。

## 推薦
A〔模型判斷·未裁決〕：唯一在所有桌型（含不結算的介面接管桌）都還得回「當時的值」的做法，也保住玩家在那一幕裡做過的手動拉回；改動集中在換幕／退回／分岔三個函式和一張新表，不碰施工中的檔案。B 的三種失準都是悄悄還原錯、玩家很難察覺；C 的範圍與衝突不成比例。

## 施工要點（A，第 1–3 輪審查後主線裁決，三方共識，2026-10-27）
- **形狀**：`scene_card_visibility: BTreeMap<幕號字串, BTreeMap<卡 id, bool>>`。空表也保留（空快照≠沒有快照）：外層可以 `skip_serializing_if = "BTreeMap::is_empty"`，內層不加，零張卡的幕照樣有 key。另加 `pending_leave: Option<u64>`（見下「分岔兩段提交」），帶 `#[serde(default)]`，舊存檔缺欄位＝沒有。
- **何時寫**：換幕在結算前、與 `current_scene` 同一次 `write_state`；分岔記父幕（被離開那幕，見下兩段提交）；退回清掉被刪子幕的 key；退回後再離開父幕會覆寫父幕快照；幕號重用時覆寫舊 key。
- **拍快照讀卡失敗就中止**：`list_characters` 的 I/O 錯誤（列目錄、讀檔）一律往上回錯、整個操作不做，不記成空表；`read_state` 失敗同樣中止。解析不了的卡檔 `list_characters` 本來就略過（不是 I/O 錯），不進快照，寫回時算表外跳過。
- **分岔兩段提交**（第 2 輪 Sol：重試不能重拍被離開那幕）：
  1. 第一段 `write_state`：寫 `scene_card_visibility[M]`（M＝目前幕）與 `pending_leave = Some(M)`，`current_scene` 不動。若進來時已經 `pending_leave == Some(M)`，表示上次在 M 的離開沒做完、卡可能已被改過，**沿用既有的 `[M]`、不重拍**。
  2. 照來源幕 N 的快照寫卡（失敗回錯，`current_scene` 仍是 M、`pending_leave` 留著）。
  3. 其餘既有步驟（世界書計時、逐字稿、種子），最後一次 `write_state` 推進 `current_scene` 並清掉 `pending_leave`。
  - 規則不綁「同一個轉移」：只要 `pending_leave == 目前幕`，任何離開這一幕的操作（重試同一個分岔、改從別幕分岔、改成換幕）都沿用那份 `[M]`，因為卡已經不是 M 的原值。代價是失敗後到下一次離開之間玩家在 M 做的手動拉回不會進快照（罕見，列已知限制）。
- **沿用快照的條件**：`pending_leave == 目前所在幕` **且** 該幕的快照 key 還在，才沿用；key 不在就重拍。`pending_leave` 不等於目前所在幕（殘留自別幕）就清掉，照常重拍。
- **退回一律清 `pending_leave`**：退回的那次 `write_state` 不論原值都清掉。
- **一般換幕沒有同類問題**：快照與 `current_scene` 同一次 `write_state`，那次失敗就什麼卡都沒動、重試重拍得到原值；成功後才結算改卡。唯一要處理的是上面那種殘留：進來時 `pending_leave == 目前幕` 就沿用既有 `[M]`、不重拍，並在同一次 `write_state` 清掉 `pending_leave`。
- **退回沒有同類問題**：退回不拍快照（子幕會被刪），只讀父幕的 `[P]` 寫卡，`[P]` 不會被這個操作改到，重試讀到的還是同一份，冪等。
- **寫回規則**：沒有該幕 key → 不動任何卡。有 key → 表內的卡照布林寫回；表外的卡（快照後才新增）、卡檔已不存在、現在已封存的卡都跳過。
- **失敗處理**：不吞錯。在同一把提交鎖裡先寫完所有卡檔，全部成功才推進 `current_scene` 的那次 `write_state`；任何一張卡 I/O 錯誤就回錯，`current_scene` 不動，重試可補完（寫卡冪等、被離開那幕的快照靠上面兩段提交保住）。卡檔已不存在算跳過、不算失敗。
- **原子寫**：`set_character_auto_hidden`（`data/character.rs:398`）由 `commit_world_write` 改用既有的 `commit_world_write_atomic`，寫到一半失敗不截壞整張卡。
- **已知限制**（不做逐卡持久重試〔模型判斷·未裁決〕）：退回或分岔回錯後，玩家不重試、直接在原幕繼續玩，原幕的卡會停在「一部分已改成目標幕的值」的混合狀態，GM 上下文也照這個混合狀態送。
  - 分岔失敗：`pending_leave` 保住了原幕快照，之後離開原幕時沿用乾淨的那份，日後回到原幕能還原正確值；但玩家在原幕這段期間看到的是混合狀態，期間的手動拉回不進快照。
  - 退回失敗：子幕沒有 `pending_leave` 保護。之後從子幕換幕：要求報在場名單或有在場名單的桌，結算會重算覆蓋；不要求報、又沒有在場名單的桌會略過結算，混合旗標原樣留下。之後從子幕離開（換幕或分岔）時，混合旗標會被拍進子幕的新快照，日後回到子幕還原的就是混合值。
- **註解同步**：「auto_hidden 只在換幕結算改」改成「換幕結算與退回／分岔復原」——`transport/arrivals.rs:73`、`commands/chat.rs:699/942`、`data/scene/presence.rs` 結算函式說明、`data/character.rs:24/358/387`。
- **測試清單**：
  1. 退回、分岔雙向還原（含重現的兩段）。
  2. 幕內拉回保留；只在子幕拉回的被父幕快照蓋掉。
  3. 封存卡不動。
  4. 快照後才建的卡，從早幕分岔後維持原值（仍收起）。
  5. 已刪卡跳過。
  6. 沒有 key 不還原；空快照保留並照規則處理。
  7. 全員可見的幕，仍能把後來被收的卡寫回 false。
  8. 不結算的桌（不要求報在場名單）也有快照。
  9. 分岔再分岔。
  10. 分岔後立刻退回，恢復的是父幕（分岔當下所在幕）不是來源幕。
  11. 退回後修改再離開，覆寫父幕快照。
  12. 幕號重用。
  13. 注入快照讀取失敗、第二張卡寫入失敗、`write_state` 失敗：場景不前進、卡檔完整、重試能補完。
  14. 分岔時卡寫完、推進場景的 `write_state` 失敗 → 重試成功 → 退回 M：每張卡恢復成第一次操作之前的完整值（含 Sol 的例子：M 的甲 true、N 的甲 false）。
  15. 拍快照時讀卡失敗（列目錄或讀卡檔注入 I/O 錯）：整個換幕／分岔中止、不寫空表、場景不前進。
  16. 分岔失敗留下 `pending_leave` 後改走一般換幕：沿用原快照、`pending_leave` 被清掉。另一條：`pending_leave` 殘留時先退回、再重用那個幕號、然後離開，快照必須是這一次新拍的。

## 業界做法與差距
- SillyTavern：分支（`bookmarks.js` `createBranch`／`getBranchChatSnapshot`）只複製到該則為止的訊息，chat metadata 重新產生，不還原任何歷史狀態；群組成員啟停（`disabled_members`）存在群組物件上，刪訊息、swipe、分支都不還原（`group-chats.js`，2026-10-27 讀 release 分支原始碼）。要撐過回溯的狀態，社群做法是存在訊息上（MVU／MagVarUpdate 把變數存在每則訊息，回溯時取最後一則的值）——TT 的 `TranscriptEvent.state` 就是這套。
- 一般遊戲的做法是存檔點快照（checkpoint）：回到哪個點就載入那個點的快照；或事件溯源（狀態全由事件推導）。
- 差距：TT 現況跟 ST 的成員啟停同一類限制（幕級狀態不跟著回溯）。A＝checkpoint 做法，把幕級狀態補進「每幕一份」的可還原位置；C＝事件溯源，最乾淨但本案範圍不划算。
