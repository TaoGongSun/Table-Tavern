# vn-mode 包 5 方案：章、結局、完結

總綱 [vn-mode.md](vn-mode.md) §3（`[chapter]`／`[ending]` 標記與終止優先序）、§6（換章字卡）、§8（章、幕與故事完結，定案）、§11 包 5 列；前置 [包 1](vn-mode-pkg1.md)（終止優先序、`pkg1_accepted`、`VnStop`、`gm_stop`）、[包 2](vn-mode-pkg2.md)（播放器結尾狀態）、[包 4](vn-mode-pkg4.md)（聖經、`chapter` 受保護根、結局 `Condition`、凍結注入）、[包 9](vn-mode-pkg9.md)（`chapter_lock`、`branching: none` 章門檻、觀察「GM 不寫 `[chapter]`」）。

**2026-11-02 完成**：方案審 2 輪（必改 2＋0，串 01a19806）；5a 驗收 2 輪（必改 1＋0，串 01a19864／01a19871）；5b 驗收 2 輪（必改 1＋0，串 01a198ab／01a198b4）。

立案估 250（中案），主線定案（子代理盤點與草案、主線裁定 §4 異議）逐項重估核心約 **1060**（§6），建議拆 5a／5b。章結束流程碰換幕的提交鎖與 `refuse_during_turn`，照流程上限做方案送審。本檔除標〔作者裁決〕者皆〔模型判斷·未裁決〕。

## 0 業界做法與我們的對應

| 業界 | 我們 | 差距 |
|---|---|---|
| Ren'Py／ink／Twine：結局是劇本裡寫死的 `jump ending_x`，條件由作者寫成 `if` 判斷式；結局一覽靠 `persistent` 記錄（跨周目） | 結局表＋`Condition` 由引擎求值，GM 只能「申請」結局，引擎驗條件才收 | 我們的條件是聖經生成時模型寫的；低階模型不回報旗標時只剩 `default`（§5） |
| 章節制 VN（Steins;Gate、Danganronpa 章卡）：章末全屏字卡＋章標題卡，存檔／章節選擇從章頭開始 | 章末字卡、換幕＋`chapter`＋1、「從某一章重玩」＝從那章的幕首分岔 | 同；我們每章至少一幕，章頭＝幕首摘要事件 |
| AI 互動小說（AI Dungeon、NovelAI）：沒有結構化結局，靠玩家自己停 | 章數上限 M、M−1 收束、第 M 章強制收尾 | 我們多一層「必定完結」保底，代價是可能收得突兀 |
| 跨周目：Ren'Py `persistent`、多周目解鎖 | 新周目＝複製設定開新桌；跨周目記錄 v2（總綱 §8） | 一致於總綱 |

## 1 現況查證（2026-11-02，分支 vn-mode）

### 1.1 `[chapter]`／`[ending]` 解析與停點

- 解析：`chat_turn/vn_script.rs:202-203` 認 `[chapter]`、`[ending: id]`（關鍵字不分大小寫、id 非空），:348-349 推成 `VnCell::Chapter`／`VnCell::Ending{id}`（型別 :54-57）。角色線 `filter_character_reply`（:545 起）整行剝掉。
- 優先序：`terminal_rank`（:378-385）`ending 3 > chapter 2 > choices 1`；`apply_terminal_priority`（:393-410）只讓「取得壓制資格」的最高者剝低者。資格函式 `pkg1_accepted`（:388-390）只給 `Choices`，所以 `Chapter`／`Ending` 永遠不壓制、也不被驗證。
- 落檔版 `parse_event`（:484-511）在**讀取時**套優先序（`read_scene` 每次重算，:424-482），所以格只由事件文字決定，沒有存「是否接受」。
- 停點：`gm_stop`（:531-541）→ `stop_reason`（:514-528）三種任一出現就 `next = None`、`vn_stop = Some(..)`；呼叫點 `chat_turn/gm.rs:382-394`，在 `turn.commit` **之後**、只拿 `display`。
- 前端**不讀 `vn_stop`**：`useChatController.ts:653-666` `advanceFrom` 只看 `next === null` 停迴圈。播放器 `vn-cells.ts:66-75` `isPlayable` 註明「章、結局不停」，`useVnPlayback.ts:23` `EndState` 只有 `waiting|choices|continue`；所以停在 `[chapter]`／`[ending]` 的事件現在播完顯示「繼續」，按下走 `gmAdvance` 在**同一幕**繼續寫（9a 實測「GM 不寫 `[chapter]` 直接演第 2 章」，寫了也一樣沒有作用）。

### 1.2 換幕入口與鎖

| 入口 | 現在的路 | 鎖 |
|---|---|---|
| 工具列「換幕」 | **只在聊天視圖**：`views/AppWorkspace.tsx:284` → `features/play/scene/useSceneActions.ts:81-102` `advanceScene`（`runTableOp` 進出桌互斥）→ `invoke("advance_scene")` → `commands/scene.rs:383-411` → `scene_budget/summarize.rs:314-323` `advance_locked`（`world_write_permit_async`）→ `advance_with`（:325-345，摘要呼叫在鎖外的許可內、全部成功才寫）→ `data/scene/lifecycle.rs:130` `begin_next_scene` → `with_commit` 短提交鎖內 `begin_next_scene_tx`（:151 `refuse_during_turn`、:164 先落摘要事件、:204 再 `write_state` 寫 `current_scene`）→ 前端 `enterTable` 重進本桌 | 桌寫入許可＋短提交鎖；回合在途（Generating／AwaitingAppend）一律拒 |
| 容量關卡 | `chat_turn/gm.rs:121-127` `check_capacity` → `scene_budget/gate.rs:56-91` `admit`：本幕摘要會溢出就拒，回 `scene_capacity_full`。前端：聊天視圖 `AppWorkspace.tsx:224` 的 `ErrorNote` 附換幕鈕（`shared/ui/atoms.tsx:45` 認錯誤碼）、`App.tsx:1129` 回合失敗框的換幕鈕；兩者都走同一個 `advanceScene` | 同上 |
| GM `[chapter]` | **沒有**：見 1.1，只是停點 | — |

**VN 視圖現在零個換幕入口**：`features/vn/VnPlayer.tsx` 工具列（:182-262）沒有換幕鈕；`App.tsx:1033` 傳給它的 `ErrorNote` 沒帶 `onAdvanceScene`，容量滿時 VN 視圖只有錯誤字、沒有鈕，玩家要切聊天視圖才換得了幕。回合失敗框（App 層）有鈕。

快照順序陷阱：`begin_next_scene_tx` 先 `append_transcript_tx` 再 `write_state`；`data/scene/transcript.rs:142-145` 補快照是讀**磁碟**上的 `state.json`，所以摘要事件的快照是換幕前的狀態。`fork_scene_tx`（lifecycle.rs:120）與 `revert_scene_tx`（:257）都以「該幕最後一則有快照的事件」還原狀態樹。

### 1.3 `chapter` 狀態樹欄位誰寫

- 種子：`data/vn.rs:516-530` `Bible::seeds()` 給 `chapter=1`；`fill_missing`（:573）每個 GM 回合在提交閉包內補種（`gm.rs:288`）。規則 `read_only`＋`UpdateMode::Reject`、`inject: turn`（vn.rs:557-566）；受保護根 `LOCKED_ROOTS = ["flags","revealed","chapter"]`（vn.rs:20），`mechanism/write_guard.rs` 擋模型、圍欄、面板手改、once 旗標。
- 讀：`vn_bible.rs:251` `chapter_of`；`turn_inputs`（:268-303）在 `frozen.scene != current_scene` 時把樹上的章號抄進 `frozen.chapter`，當章包依它取（`bible_chapter`）；`vn_bible.rs:434-470` 揭露清單的章號。
- **沒有任何程式把它加一**。沒有聖經的 VN 桌（`bible: null`）沒有種子、沒有規則，樹上沒有 `chapter`。

### 1.4 GM 規範與結局 id

- `transport/vn_format.rs:14-62` `vn_format_rules`（GM system 穩定前綴，`chat_assembly.rs:530`）只教 `[bg]`、`[choices]`，**沒有** `[chapter]`／`[ending]`；`vn_character_rules`（:151-165）禁寫清單只列 `[bg]`／`[choices]`（過濾器已剝其餘）。
- 結局 id 清單**已在** GM 常駐段（`transport/vn_bible_prompts.rs:213-221`：`- id 標題: 條件白話`），但 `default` 顯示成「無條件／always」，等於邀請 GM 隨時寫 `[ending: default]`。章數 M 也已在常駐段（:149「共 N 章」）。
- 回合尾導演指示 `vn_narrate_instruction`（vn_format.rs:195-255）有 `branching: none`、人稱、主角句，沒有章或結局相關句。`chapter` 現值靠 `inject: turn` 進回合尾狀態塊。

### 1.5 結局條件求值

`mechanism/triggers.rs:139` `holds(tree, mechanism, condition)` 是私有函式（處理 collections 需要 `&Mechanism`），`condition_holds`（:134-137）只在 `#[cfg(test)]`。重用＝把 `holds` 開成 `pub(crate)`，不另寫求值器。結局表驗證正規化已在 `data/vn.rs:340-380`（只認 `chapter` 與表內 `flags.*` 路徑、保證有 `default`、`branching: none` 補章門檻）。

### 1.6 `ended`、回饋通道、其他

- `ended` **不存在**（整個 Rust 與 `features/vn` 都沒有）。
- 拒收回饋的既有通道：`mechanism/ledger.rs:422-423` 把 `build_notes` 結果寫進 `world.state.notes`，`transport/state_view.rs:97-105` 下一輪放進回合尾。`gm_commit_apply` 每輪覆寫 `notes`，所以要在它之後追加。
- `fork_scene_tx`（lifecycle.rs:52-55）只收 `from_scene < current_scene`、複製**整幕**並以該幕最後快照續玩（＝從那幕結尾續，不是從章頭重玩）；結局所在的最後一幕不能分岔。
- 沒有任何「複製一張桌」的函式（`refactor/reset.rs` 是照匯入收據重放，不是複製）。
- 標題畫面在包 6（總綱 §9、§11），本包還沒有。

## 2 定案設計

### 2.1 狀態樹：`ended` 與 `finale`

- 新增兩個引擎專用頂層葉子，加進 `LOCKED_ROOTS`（改成 5 個），規則同 `chapter`（`read_only`、`Reject`、`inject: turn`）：
  - `ended`：字串，`""`＝未完結，否則＝接受的結局 id。
  - `finale`：`"true"`／`"false"`，第 M 章的章結束入口觸發後設真＝「下一回合強制收尾」。不能只用 `chapter == M` 推導（整個第 M 章每回合都會被逼結局）；要跟快照走（重開 app、收回、分岔都對）。
- 種子：`Bible::seeds()` 加 `ended=""`、`finale="false"`；**沒有聖經的 VN 桌**也要有 `chapter`／`ended`／`finale` 三個種子與規則：新增 `vn::engine_seeds()`／`engine_rules()`，`turn_inputs` 在 `bible: null` 時回這組（`fill_missing` 照常補），規則在第一個 GM 回合補種時一併寫入（施工時照包 4 `install` 的規則寫法放進提交閉包，不另開入口）。
- 這些都在 `state.state.tree`，事件快照、收回、分岔、退幕自動帶著走（總綱 §8「跟快照走」）。

### 2.2 標記驗證：取代 `pkg1_accepted`

驗證只在**提交時**做一次，結果寫回事件文字與狀態樹；之後讀取一律信任存下的文字。

- 新檔 `chat_turn/vn_ending.rs`：`judge(cells, ctx) -> Verdict`，`ctx`＝{聖經結局表（沒聖經＝只有 `default`）、`chapter`、M、`finale`、套完本拍更新後的樹與機制}；`Verdict`＝{要剝掉的標記行、要補的 `[ending: default]`、`ended` 新值、`finale` 新值、回饋句}。
- 規則（照總綱 §8）：
  - `[ending: id]`，一般回合：id 在表內、**不是 `default`**、`when` 全部成立（`triggers::holds` 對**套完本拍 `<UpdateVariable>` 後**的樹求值——本拍改的旗標才算數）＝接受；否則剝掉該行＋回饋句「結局 `<id>` 不成立（條件：…）／不在結局表；故事繼續」，劇情照播，不轉 `default`。
  - `[ending: id]`，強制收尾回合（`finale` 真）：id 在表內且條件成立（`default` 恆成立）＝接受；GM 沒寫或不成立＝引擎在文字末尾補一行 `[ending: default]` 並接受。強制收尾回合**必定完結**。
  - `[chapter]`：`chapter < M`＝接受（留在文字，作為章結束流程的觸發，§2.3）；`chapter >= M`＝剝掉並設 `finale = true`（「第 M 章裡的章結束入口不再換幕、改走強制收尾」）。`finale` 已真時一律剝掉。
  - 同一拍有被接受的 `[ending]` 時，其餘終止標記照優先序剝（含 `[choices]`）。
- 剝掉＝從事件文字（`text` 與 `raw` 都改）刪掉那一行，所以落檔文字裡留下的 `[chapter]`／`[ending]` 就是被接受的。`parse_event` 的資格函式改成「GM 事件裡的終止格恆有資格」，刪 `pkg1_accepted`；串流入口照舊不套刪格規則（包 1 契約不變）。
- **接線（`gm.rs`）**：解析與判定要搬進 `turn.commit` 的 apply 閉包（在 `gm_commit_apply` 之後），閉包內同時寫 `ended`／`finale` 與回饋句（追加到 `state.state.notes`），讓主事件快照帶著結果。現在 `PendingMain` 在呼叫 `apply_gm_block` 前就定死，所以 `data/message_vars/write.rs` 的 `apply_gm_block`／`submit`（:451、:518）把 `main` 改成「apply 之後產生」（`impl FnOnce(&R) -> PendingMain`；其他呼叫端傳 `move |_| main`）；`GmNarration.text` 用同一份最終文字。`gm_stop` 改吃判定後的文字。
- `vn_stop` 加一個 `Finale`（`[chapter]` 在第 M 章被轉成收尾）：前端照「沒有選項」處理，顯示「繼續」，按下走強制收尾回合。
- GM 線回聲對帳比的是 `event.text`（`lanes/plan.rs:199–207`，Sol 查證），剝行後必然判成 ReplyDiverged，下一拍重開 session（斷一次快取）。接受此代價：只有不成立的 `[ending]`、第 M 章的 `[chapter]` 會剝行，屬少數回合；不另改 expected reply。
- zod 引擎桌（VN 卡不會是，但理論上可能）走 `commit_engine`，不做結局驗證：`[ending]` 一律剝、只有強制收尾能完結，列已知限制。

### 2.3 章結束流程：一個 handler、一個後端落點

**後端**：章號加一與 M 上限判斷放進 `begin_next_scene_tx` 那把 `with_commit`，與 `current_scene` 同一次 `write_state`。理由：`[chapter]` 被接受時 GM 回合還在 AwaitingAppend，`refuse_during_turn` 會擋，所以換幕不能在 `gm_narrate` 裡做，只能等事件落檔後由前端呼叫；放在換幕交易裡，失敗（摘要失敗、停止、寫檔失敗）時章號與幕號一起不動。

- **只有章結束入口加章**（§4 異議 1 採納〔模型判斷·未裁決〕）：`advance_scene` 加參數 `chapter_end: bool`；GM `[chapter]` 被接受與 VN 工具列「下一章」傳 true；容量關卡與聊天視圖的換幕鈕傳 false＝只換幕（存檔單位），章號、當章包、`finale` 都不動。理由：章是劇情單位，容量是技術限制，小上下文模型會把章切碎、提早撞 M。
- `begin_next_scene_tx` 收到 `chapter_end` 且讀 `table_kind == vn` 時（鎖內讀 `vn.json` 的 `chapter_max`，沒聖經＝8；讀樹上 `chapter`，缺＝1）：
  - `ended` 非空 → 拒（`VN_ENDED`）。
  - `chapter < M` → 樹上 `chapter = chapter + 1`。
  - `chapter >= M` → 不加章，`finale = true`（工具列「下一章」在第 M 章＝要求收尾）。
  - `ended` 非空時，非章結束的換幕也拒（完結桌不再換幕）。
  - **摘要事件明傳 `state: Some(新樹)`**，不靠 `append_line_tx` 讀磁碟補快照（1.2 的順序陷阱，否則分岔／退幕還原到舊章號）。
  - **變數模式的權威表一併同步**（Sol 方案審第 1 輪必改 1）：有效 `message_vars.stat_data` 時 `read_state` 以表覆蓋樹（`data/state.rs:516–524`），`scene_seed_for_next` 沿用舊幕表（`data/message_vars/write.rs:663–683`）。所以同一換幕交易內，把新 `chapter`／`finale` 合入新幕種子表與摘要事件攜帶的表，再發布種子；樹、表、快照三處同值。測試：變數模式 VN 桌換章後 `read_state`、新幕首個 GM 回合讀到的章號、退幕與分岔還原，皆為新章號／`finale`。
- 總綱 §8「已是第 M 章就不換、轉結局路徑」的調整：`[chapter]` 在第 M 章由 §2.2 就地轉 `finale`、**不換幕**；但工具列與容量兩個入口走的是 `advance_scene`，容量滿時若不換幕，強制收尾那一回合會被 `check_capacity` 擋下（`gate.rs:56-91` 溢出即拒）而永遠完結不了。所以第 M 章的換幕照換（存檔單位），只是不加章、改設 `finale`。見 §4 異議 2。
- 命令不加新的：`advance_scene` 對 VN 桌自動走這段；回傳照舊幕號。聊天視圖的換幕鈕、`ErrorNote`、回合失敗框也就自動都是章結束流程（總綱「三個入口都走它」）。
- 新章的當章包：`turn_inputs` 以幕為鍵重抄 `frozen`（vn_bible.rs:281-292），換幕後第一輪就取新章號的當章包，不必另接。

**前端**：VN 視圖一個 handler `endChapter()`（`VnPlayer` 收 App 傳入的 `onAdvanceScene`＝`sceneActions.advanceScene`）：

1. 顯示章末字卡「第 N 章 完」（N＝游標所在事件快照的章號，`read_scene` 的 `VnEventCells` 多帶 `chapter`）。
2. 換幕分三個階段，各自有結果（Sol 方案審第 1 輪必改 2；現行 `advanceScene(): Promise<boolean>` 吞例外、後端成功但 `enterTable` 失敗也回 false，`useSceneActions.ts:81–102`）：
   - **換幕**（後端 `advance_scene`）：回 `{ committed: 新幕號 } | { failed, code }`。未提交（摘要失敗、停止、忙碌、`VN_ENDED`）＝字卡顯示原因＋「重試」，重試才再呼叫換幕；章號與幕都沒動。
   - **重進**（`enterTable`）：handler 記住已提交的幕號；重進失敗只重試重進，絕不再呼叫換幕（否則多換一章）。
   - **首拍**（`chat.gmAdvance()`）：重進成功後才開；失敗照一般回合失敗處理（字卡收起、播放器顯示既有錯誤與重試），不回頭動換幕。取消或忙碌一律不自動續寫。
   `sceneActions` 擴一個回傳分階段結果的版本給 VN handler 用，聊天視圖既有呼叫端照舊。
3. 重進成功後字卡換成「第 N+1 章：〈章標題〉」（`finale` 時不換標題），自動開首拍，第一格出來字卡淡出。
4. 觸發點：
   - 游標推進到「最後一則正式事件的 `Chapter` 格」（`isPlayable` 讓它成為停點，條件比照現在的選項格：`kind !== "pending"` 且屬於本幕最後一則正式事件——串流中寫了會被剝掉的標記不能當停點；`EndState` 加 `chapter`）。auto 推上章末格會觸發（總綱 §6 本來就是自動流程）；skip 推到章末格停住，要玩家按一下才開下一章（skip 是快轉已讀，不該替玩家花摘要與下一章的額度）。重開 app 從這裡進桌也一樣：玩家推進到章末格才觸發，不會一進桌就花額度。
   - VN 工具列新增「下一章」鈕（取代總綱說的「換幕」字樣，VN 玩家不認識「幕」〔模型判斷·未裁決〕），確認框一句「結束本章並整理摘要」。
   - VN 視圖的 `ErrorNote`（容量滿）換幕走 `chapter_end: false`：同樣顯示「整理中」字卡，但不播「第 N 章 完」、不換章標題。
- 總綱 §6 寫字卡與摘要「同時跑」：實際上摘要要等玩家讀到章末格才開始，因為換幕會 `enterTable` 換掉播放器的格（還沒讀完的格會消失）；等待由字卡吸收。施工確認 `enterTable` 不會重掛 `VnPlayer`（`App.tsx:1027` 沒 key），若會，字卡狀態提到 App 層。

### 2.4 收束模式與強制收尾（回合尾導演指示）

全部放 `vn_narrate_instruction` 回合尾（每輪重組、不進 system，不碰快取紅線）。`VnDirection` 多帶 `chapter`、`chapter_max`、`finale`、本章 `end_condition`、目前成立的結局 id 清單（以本回合開始時的樹求值，只是提示）。

- 每輪（有聖經）：「目前第 k／M 章。本章結束條件：〈end_condition〉。達成時在本拍最後另起一行寫 `[chapter]`，同一拍不要再出選項。」——對準 9a 觀察「GM 不寫 `[chapter]`」；結束條件文字本來就在當章包，回合尾再點一次是遵守度補強。
- `k == M−1`（M≥3 必有）：加「收束模式：本章內把劇情推向最接近的結局，開始收伏筆。」
- `k == M`：改成「這是最後一章。本章結束時不要寫 `[chapter]`，改寫 `[ending: 結局id]`；目前條件成立的結局：a、b（都不成立時照劇情收尾，引擎會處理）。」
- `finale` 真：「（強制收尾）這一拍是故事最後一拍：寫完結局場面，最後一行寫 `[ending: 結局id]`；目前成立的結局：…；都不成立就寫 `[ending: default]`。不要出選項。」
- 沒聖經的桌：只在 `k == M` 與 `finale` 時加對應句，結局只有 `default`。

### 2.5 GM 規範補寫法（system 穩定前綴）

- `vn_format_rules` 加兩行（zh／en）：
  - `[chapter]`：本章結束條件達成時，在本拍正文最後另起一行寫，與 `[choices]` 不同拍。
  - `[ending: 結局id]`：只在故事真正收尾、且該結局條件成立時寫；id 照「結局」表原字；`default` 只在導演指示要求時用。
- `vn_bible_prompts.rs` 結局段：`default` 那行從「無條件」改成「保底結局（只在導演指示要求時用）」。
- `vn_character_rules` 禁寫清單補 `[chapter]`／`[ending]`（過濾器早已剝，只是少一個誘因）。
- 結局 id 清單不另給：常駐段已有（GM-only，角色線看不到，劇透鎖是 UI 層與它無關）。規範改一次＝GM 線 system 前綴改一次，換版後第一輪斷一次快取，之後穩定；回合尾的章與結局句不影響快取。

### 2.6 `ended` 鎖住續寫

- 後端統一檢查 `vn_ended(state) -> bool`（VN 桌且樹上 `ended` 非空），錯誤碼 `VN_ENDED`：`gm_narrate`（`check_capacity` 之前）、`chat_with_character`、`append_player_event`、`vn_reroll_choices`、`begin_next_scene_tx`（§2.3）。
- **不擋**：`append_transcript`（GM 事件落檔）與 `state_update` 附屬事件。結局那一拍提交時 `ended` 已寫進 `state.json`，前端才落主事件；擋了結局事件自己就落不了檔。
- 收回（undo）與分岔照常可用：狀態跟快照退回，`ended` 自然清掉（總綱「只能從分岔點續」；收回等於回到結局前一拍，不另擋）。
- 聊天視圖：送出時吃到 `VN_ENDED` 錯誤字，不另做輸入框停用〔模型判斷·未裁決〕。

### 2.7 完結畫面（5a 最小版、5b 補齊）

- 游標推進到最後一則正式事件的 `Ending` 格＝停點（同 §2.3 的正式格條件；`EndState` 加 `ended`），蓋上 `EndingScreen`：「完」、結局標題（`default` 沒標題時顯示「完」）、已達成 x／y、鈕。
- 5a 鈕：「回大廳」（標題畫面在包 6，先回大廳）、「開新周目」（5b 才有，5a 不出）。
- 5b 鈕：「從某一章重玩」（§2.9）、「開新周目」（§2.10）、「結局一覽」。
- 已達成 x／y：本桌所有幕（含分岔留下的分支）逐字稿裡出現過的 `ended` 值集合（掃各幕事件快照）／結局表條數（含 `default`）；新命令 `vn_endings(world_id) -> { endings: [{id, title, reached}], current }`，未達成的標題回空，前端顯示「???」。

### 2.8 結局一覽（5b）

總綱放標題畫面，標題畫面在包 6；本包先放 VN 工具列 ⋯「結局一覽」對話框＋完結畫面鈕，包 6 標題畫面直接重用元件。內容＝`vn_endings`，已達成顯示標題，未達成「???」；劇透鎖住時也照這個規則（已達成的不算劇透）。

### 2.9 從某一章重玩（5b）

- `fork_scene` 現成不能用（1.6：整幕複製＝從那幕結尾續；最後一幕不能分岔），總綱 §8 的「`fork_scene` 現成」在此改寫。
- 新增「只保留幕首事件的分岔」：`fork_scene_tx` 加 `keep: Option<usize>`（截前 N 則，章頭用 1），命令 `vn_replay_chapter(world_id, chapter)`。章頭幕＝沿目前幕的**劇情祖先鏈**往回找，第一個幕首快照 `chapter == k` 且最早的那幕；第 1 章＝鏈上最早的第 0 幕（開場事件）。`scene_labels.parent` 是「分岔前玩家所在的幕」不是內容來源（`data/scene/lifecycle.rs:109–117`，Sol 建議），不能直接當祖先鏈：`SceneLabel` 加 `source: Option<u64>`（分岔時記被複製的來源幕），祖先鏈＝分岔幕走 `source`、換幕來的幕走 `parent`（沒有 `source` 的舊分岔幕退 `parent`，發佈前不顧舊桌）。這樣同章多幕（容量換幕）時取該章最早一幕，重玩後再選章也只走自己這條分支的祖先，不會跳到已離開的分支。測試補：同章兩幕取前者、重玩第 2 章後再重玩第 1 章、從舊分支切回後選章。新幕狀態＝那則事件的快照（§2.3 明傳快照才正確），`ended`／`finale` 自然是空。
- 限制：截到幕首時世界書計時檔沿用來源幕整幕結束時的檔（`copy_for_fork`），定時條目可能差幾輪〔列已知限制，包 6 事件級分岔一併處理〕。
- `fork_scene_tx` 的 `from_scene < current_scene` 限制對截幕首的分岔放寬為 `<=`（重玩目前這章）。
- 包 6 的「從某一則事件分岔」直接擴充同一個 `keep` 參數，不另寫。

### 2.10 新周目（5b）

- 命令 `vn_new_playthrough(world_id) -> new_world_id`：照 VN 卡匯入的模式 `create_world_exclusive(vn)` 建新桌，一次建完、任一步失敗刪整桌目錄。
- **角色 id 一律沿用原 id**：`vn/sprites/<角色id>/`、聖經 `holder`、世界書可見性、`player_card_id` 都以 id 為鍵；不走 VN 卡匯入那條 `new_id()` 重對應的路。
- **複製白名單**：`world.md`、角色卡（含 `player_card_id`、遊玩中改動的可見性欄照第 0 幕快照還原）、世界書、`vn/vn.json`（`frozen` 清掉、`spoiler_lock` 照舊）、`vn/sprites/`、`vn/backgrounds/`、`vn/cover.png`、`gm.png`、模型指定、機制規則與 `mechanism.incremental`。
- **重建**：狀態樹＝聖經 `seeds()`＋引擎種子（覆蓋，不沿用遊玩值）；第 0 幕只留第 0 幕的 `opening: true` 事件（文字原樣，快照重蓋）；桌名「〈原名〉（第 N 周目）」，N 從原名的周目後綴加一。
- **不帶**：其他逐字稿、`lanes.json`、`world-info/` 計時、`mechanism-log.jsonl`、回合紀錄與變數模式控制檔、匯入收據、網頁存檔檔。
- 前端：完結畫面「開新周目」→ 確認框 → 建好直接進新桌（VN 視圖，第一拍是開場事件，「繼續」開演）。

## 3 測試

- 單元（後端）：
  - `judge` 表驅動：一般回合合法結局、條件不成立、id 不在表、一般回合寫 `default`、強制收尾 GM 給合法 id、強制收尾沒寫或不成立補 `default`、本拍 `<UpdateVariable>` 剛把旗標設真同拍寫結局（要接受）、`[chapter]` 在 k<M 留、k=M 剝並設 `finale`、`[ending]`＋`[choices]` 剝選項、無聖經桌。
  - 換幕：VN 桌 `chapter` 加一並與 `current_scene` 同次寫入、摘要事件快照帶新章號（分岔與退幕還原後章號正確）、第 M 章換幕不加章設 `finale`、`ended` 拒換幕、摘要失敗章號不動、聊天桌不受影響。
  - `ended` 四個入口拒收；收回後解除。
  - 規則與種子：`ended`／`finale` 唯讀、模型寫入走既有拒收；無聖經桌補種三個鍵。
  - 導演指示四種分支（一般、M−1、第 M 章、強制收尾）與規範字串。
  - 5b：`vn_endings` 跨分支掃描、截幕首分岔（章頭幕找法、第 1 章＝第 0 幕、快照）、新周目白名單（不帶逐字稿與計時檔、失敗不留半桌）。
- 前端：`isPlayable` 與 `EndState` 新分支、`endChapter` 失敗重試、字卡時序（假 invoke 要照真後端對 `VN_ENDED` 報錯，見 process-log 包 0 觀察）。
- 測試通道真模型（luna；另以 Haiku 跑一輪看遵守度，§5）：
  1. 設 M=3 的生成桌玩到 GM 自己寫 `[chapter]` → 字卡 → 摘要 → 第 2 章第一拍自動開演、劇本分頁章號 2。
  2. 工具列「下一章」與容量入口（harness 注入超量事件或調低上限）各走一次。
  3. 第 2 章看收束句（AI log）；第 3 章 GM 寫合法結局 → 完結畫面。
  4. 第 3 章按「下一章」→ 強制收尾回合 → 完結。
  5. 故意寫不成立的結局（harness 改回覆）→ 剝掉、回饋句下一輪出現、劇情照播。
  6. 完結後送出被擋；收回後可續。
  7. 5b：結局一覽、從第 2 章重玩（新分支從第 2 章頭開始、x／y 不變）、開新周目（新桌乾淨、立繪背景沿用、不重生圖）。
  截圖主線親看。

**5a 實測結果**（2026-11-02，測試通道 release 包；第 7 項屬 5b 未做）：

- 桌：luna 生成桌「三夜熄燈」（M=3）、Haiku 生成桌「雪夜白樺屋」（M=3）、MVU 變數模式桌「灰港編年史」（聖經 M=8，**注入**改成 3）。
- 1 過：luna 第 1、2 章都自己寫 `[chapter]` → 字卡 → 摘要 → 下一章第一拍；修正前第一拍落在舊幕被丟（下列修正 1），修後第 3 章第一拍自動開演、劇本分頁章號正確。變數模式桌摘要與第一拍的 message_vars、讀狀態章號皆為新章。
- 2 過：工具列「下一章」第 2→3 章；聊天視圖換幕不加章；容量入口（**注入** context_window 12000）錯誤條「換幕」→ 幕 2→3、章號維持 3。
- 3 過：AI log 看到第 1 章、M−1 收束、最後一章三種提示；luna 從分岔幕自己寫合法結局 → 完結畫面 2／3。
- 4 過：第 3 章按「下一章」→ 尾聲字卡 → 強制收尾 → `default` 完結 1／3；Haiku 強制收尾給合法結局 id（另試圖用 `<UpdateVariable>` 寫 `ended` 被拒）。
- 5 過：**注入**（聊天視圖以玩家話要求 GM 寫不成立的結局 id）→ 標記剝掉、下一輪提示出現回饋句、劇情照播。
- 6 過：完結後送出被擋；收回後 `ended` 回 `none`、可續。
- 遵守度（§5）：Haiku 第 1 章 14 拍寫 1 次 `[chapter]`；第 3 章 15 拍沒寫結局，只能靠強制收尾。
- 呼叫：共 109 次（luna GM 28、角色 11、單發 7、生圖 13；Haiku GM 31、角色 9、單發 10）。
- 修正：(1) 下一章第一拍等播放器換到新幕才開演（舊閉包落到舊幕被拒）；(2) VN 摘要快照清掉 `changes`（旗標提示在摘要重播）；(3) 摘要事件不產生終局格（摘要抄到 `[ending]` 會被當結局）；(4) 下一章確認框取消鈕在地化；(5) 完結畫面不再蓋住工具列；(6) 章提示結束條件尾句號重複；(7) 換幕摘要（`SceneSummary` 事件）不再當旁白播、只進紀錄，容量換幕、章結束換幕、重開進桌都從「繼續」／首拍開始；(8) 對話框限高、長段落在框內捲動並跟著打字機捲到最新行。
- Sol 驗收第 1 輪必改：換幕先備妥新幕種子（變數模式套新章號）與摘要（不回寫目前狀態），最後一次 `write_state` 才發布 `current_scene` 與章號；發布失敗章／幕維持原值、重試只前進一次（兩個故障注入測試：樹模式、變數模式）。

**5b 實測結果**（2026-11-02，測試通道 release 包）：

- 桌：luna「三夜熄燈」（已完結 guided_home）、MVU 變數模式桌「灰港編年史」（5a 注入 M）。
- 完結畫面四顆鈕（從某一章重玩／開新周目／結局一覽／回大廳）過；結局一覽已達成顯示標題、未達成「???」、2／3 過。
- 從第 2 章重玩過：新分支幕 5（`source` 1＝第 2 章頭）、章號 2、`ended` 回 none、x／y 仍 2／3、原分支逐字稿原封；用分岔切回已完結分支照樣顯示完結畫面。再選第 1 章（⋯ 選單）過：`source` 0，從開場第一句播。
- 開新周目過：桌名「三夜熄燈（第 2 周目）」、只有白名單檔、狀態樹＝聖經種子、第 0 幕只留開場、背景沿用不重生圖；「繼續」開演跑 GM，下一章流程照常（第 2 章、首拍從第一句播）。
- MVU 桌開新周目：施工版不帶控制檔、會丟掉整張卡的變數表（樹模式）——已修（下列修正 1）；修後新桌仍是變數模式、第 0 幕新 epoch、狀態欄照常，GM 回報 HP／Injury 被收（`changes` 有 `Player.HP -1`），寫唯讀 `/chapter` 被拒並回饋。（**注入**：用 harness invoke 直接開新周目（這桌未完結、沒有完結畫面）；最後兩輪把 GM 暫切 luna 並在玩家句裡要求更新狀態——Haiku 兩輪都沒寫 `<UpdateVariable>`。）
- 呼叫：15 次（Haiku GM 7、角色 4、單發 1；luna GM 2、生圖 1＝新周目第一拍的新時段背景）。
- 修正：(1) 變數模式桌開新周目照原桌模式與代換值重建控制檔，第 0 幕種子＝開場那則的表，開場事件帶表；白名單補世界層原卡 `source-card.*`（卡片自帶介面）；(2) 換幕／重玩／分岔／換桌後播放器游標照新幕進桌位置重新定位（原本沿用舊游標、找不到就跳到最後一格，重玩第 1 章會停在開場最後一句）；(3) 結局一覽／章節選擇開著時收起完結畫面（原本兩層字疊在一起）、清單置中。

## 4 異議

1. **容量入口逼加章**（主線採納，§2.3）（總綱 §8 把容量列為章結束入口）：小上下文模型（本地模型、短窗 API）會頻繁觸發容量，章被切碎、提早撞到 M 而強制收尾，故事被「長度」而不是「劇情」結束。容量與聊天視圖的換幕只換幕不加章（`advance_scene` 加 `chapter_end: bool`），章號與當章包沿用；多約 15 行。
2. **第 M 章仍要能換幕**（主線採納）：總綱寫「第 M 章裡任何章結束入口都不再換幕」，但容量滿時不換幕，強制收尾回合會被容量關卡擋死、桌子永遠完結不了。改為：`[chapter]` 在第 M 章不換幕（就地 `finale`）；工具列「下一章」在第 M 章換幕並設 `finale`；容量照換幕、不動章。
3–6 主線全採納。
3. **「從某一章重玩（`fork_scene` 現成）」不成立**（§2.9）：需要新增截幕首分岔；本包補最小後端，包 6 擴成事件級。
4. **結局一覽與「回標題」依賴包 6 標題畫面**：本包先放工具列 ⋯ 與「回大廳」。
5. **「字卡與摘要同時跑」做不到**（§2.3）：換幕會換掉播放器的格，摘要要等玩家讀到章末格才開始。
6. `default` 在常駐段顯示「無條件」會誘導 GM 隨時寫保底結局；§2.5 改字，§2.2 一般回合拒收 `default`（總綱已定「只有引擎強制收尾才走 `default`」，這裡只是補上 GM 可見文字的對應）。

## 5 遵守度風險（包 4／9a 觀察）

- Haiku 不回報旗標 → 結局條件幾乎都不成立 → 全部走強制收尾的 `default`。可玩、能完結，但結局分歧消失。本包不做引擎推斷旗標（總綱 §7 明令不從正文推斷）；通道用 Haiku 跑一輪記結果，若 `default` 佔絕大多數，另立案評估「低階模型提示選高階 GM」之類的玩家提示，不在本包。
- GM 不寫 `[chapter]`：靠 §2.4 每輪點一次結束條件＋工具列「下一章」＋容量入口三重保底。

## 6 預算（逐項估核心）

| 項目 | 5a | 5b |
|---|---|---|
| 狀態樹 `ended`／`finale`、無聖經引擎種子與規則（data/vn.rs、vn_bible.rs） | 45 | |
| `vn_ending.rs` 判定＋剝行補行、`triggers::holds` 開放 | 110 | |
| `gm.rs` 接線、`apply_gm_block` main 改 apply 後產生、`gm_stop`／`VnStop::Finale`、回饋句 | 60 | |
| `begin_next_scene_tx` VN 分支（加章、M、`finale`、明傳快照、`ended` 拒） | 45 | |
| `ended` 四入口檢查＋錯誤碼 | 25 | |
| 提示詞：規範兩行＋角色禁寫＋結局段改字（zh／en 字串） | 25 | |
| 回合尾導演指示四種分支（zh／en 字串）＋`VnDirection` 接線（chat_assembly） | 80 | |
| `read_scene` 帶 `chapter` | 10 | |
| 前端：`isPlayable`／`EndState`、章末字卡元件＋`endChapter` 流程、工具列「下一章」、ErrorNote 接線、App 傳 prop | 140 | |
| 前端：最小完結畫面＋`vn_endings` 的 x／y | 50 | |
| `vn_endings` 命令（跨幕掃描） | 40 | |
| CSS：字卡、完結畫面 | 60 | |
| 截幕首分岔（`keep` 參數、章頭幕查找、命令） | | 70 |
| 新周目命令（白名單複製、重建、失敗刪桌） | | 150 |
| 前端：結局一覽對話框、章節選擇、新周目流程、完結畫面補鈕 | | 120 |
| CSS：一覽、章節選擇 | | 30 |
| **合計** | **約 690** | **約 370** |

- 合計約 **1060**（立案 250 的 4.2 倍）。依 process-log 經驗（同類包實作再超 1.4–2 倍，超支主因提示詞字串、CSS、接線點數），已把 zh／en 字串、CSS、每個入口的接線分列。
- **建議拆包**：5a＝章與結局引擎（標記驗證、章結束流程與鎖、M 上限與收束、強制收尾、`ended` 鎖、規範、字卡、最小完結畫面）；5b＝結局一覽、從某一章重玩、新周目。本檔同時是 5a 與 5b 的方案：Sol 審本檔一次＝兩包的方案審（5a 碰提交鎖與換幕交易必送；5b 估 370 屬大案，也在這次一併審），施工後兩包各驗收。
- 熱點：`gm.rs` 410、`commands/scene.rs` 712、`App.tsx` 1174、`useChatController.ts` 922——判定邏輯放新檔 `vn_ending.rs`，熱點只加接點；`App.tsx` 只傳 prop。

## 7 已知限制（預計）

- 結局條件依賴模型回報旗標；低階模型多半只得到 `default`（§5）。
- 容量換幕不加章：GM 一直不寫 `[chapter]`、玩家也不按「下一章」時，一章可以跨很多幕（每輪導演指示有提醒）。
- 截幕首分岔沿用來源幕結束時的世界書計時檔。
- 沒有聖經的桌，`chapter`／`ended`／`finale` 的唯讀規則在第一個 GM 回合補種時才寫入，之前的那一輪不擋模型寫這三個鍵。
- zod 引擎桌不驗結局，只能強制收尾完結。
- 跨周目的結局一覽不共用（總綱：v2）。
- 聊天視圖看 VN 桌：完結後輸入框不停用，送出才吃錯誤。
- 包 5 前建的桌，舊事件裡沒驗過的 `[chapter]`／`[ending]` 會被當成接受（發佈前不顧舊桌）。
- 5a 施工：`ended` 的未完結值是保留字 `none`（狀態樹寫空字串＝刪葉子），結局 id 不得叫 `none`（正規化丟掉）。
- 5a 施工：下一章字卡只顯示章號（「第 N 章」），不顯示章標題（播放器沒有聖經；劇透鎖下也不必另開讀取）；第 M 章轉收尾時顯示「尾聲」。
- 5a 施工：完結錯誤走 `UiMsg::VnEnded`（十語系），不是裸字串碼。
- auto 衝到章末格就開始跑摘要與下一章呼叫（auto 是玩家選的自動播放）；skip 會停在章末格。
- 5a 實測：下一章第一拍在播放器換到新幕後才開演；中途關掉播放器時這一拍不補，下次行動照既有「代落」補上。
- 5b 施工：新周目桌名不從原名解析周目後綴（後綴是在地化文字）：vn.json 記 `playthrough`（第幾周目＋第 1 周目桌名），前端傳在地化樣板 `{name}（第 {n} 周目）`。
- 5b 施工：「從某一章重玩」除了完結畫面，工具列 ⋯ 也有（遊玩中也能選）；章節選擇只列章號 1…目前章（同 5a 字卡不顯示章標題）；完結畫面的「紀錄」鈕拿掉（工具列有）。
- 5b 實測：變數模式桌開新周目不抄原桌控制檔（各幕紀錄），照原桌模式與代換值重建，第 0 幕種子取開場那則目前的表（若玩家在開場後用面板改過開場那則，改過的值會一起帶過去）。
- Sol 驗收 5b 第 1 輪：新周目另帶重構骨架 `interface-shell.html`、重構產物、生成圖庫、角色層與腳本層卡片變數，以及 `refactor_mode`、`statusbar_retired`、`regex_allowed`、`branch_bindings`；聊天層變數（`card-vars/chat.json`）、幕名與幕標籤、補課摘要、對齊幕號是遊玩進度，不帶。

## 7.1 作者裁決

- 容量與聊天視圖換幕不加章（§4 異議 1，總綱 §8 已改）〔作者裁決 2026-11-02〕。
- VN 工具列鈕名「下一章」（取代「換幕」）〔作者裁決 2026-11-02〕。
- 已達成 x／y 與結局一覽只算正式結局，保底結局 `default` 不算；沒有正式結局時不顯示 x／y〔作者裁決 2026-11-02〕。
- 拆 5a／5b：已照拆施工。

## 8 觀察

（施工與實測發現的範圍外問題寫這裡，一行一條。）
- 5a 實測（已修，見 §3 修正 7、8）：換幕摘要原本在 VN 畫面當旁白播，原樣顯示 markdown 並超出對話框。
- 5a 實測：角色道偶爾在原文寫 `[ending]`（已被過濾，不影響結局）。
- 5b 實測：Haiku 在變數模式桌連兩輪沒寫 `<UpdateVariable>`（即使玩家句明講）；luna 會寫，還會嘗試改唯讀 `/chapter`（被拒、有回饋）。
- 5b 實測：施工前的舊分岔幕沒有 `source`，祖先鏈退走 `parent`（發佈前不顧舊桌）。
