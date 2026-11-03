# vn-mode 包 1 方案：劇本格式、導演指示、停點、換一組

總綱 [vn-mode.md](vn-mode.md) §2／§3／§8／§10／§11；包 0 已落 `table_kind`。大案：本方案 Sol 審 2 輪（第 1 輪必改 3、第 2 輪必改 2，全採納）後施工；核心估 400（寫到 800 停下檢討）。本檔除標〔作者裁決〕者皆〔模型判斷·未裁決〕。

## 0 查證現況（2026-11-01）

- GM 回合：`chat_turn/gm.rs::gm_narrate` → `chat_assembly::gm_instruction` → `transport::gm_turn_format`（CardFormat／InterfaceTakeover／Narration）決定導演指示與收尾句；回覆經 `gm_reply::gm_reply_parts` 剝 ```state 圍欄、`<UpdateVariable>`、「下一位」行後落檔（`text`＝顯示文、`raw`＝原文）。`next`＝點名（角色名或 `__PLAYER__`）。
- 角色回合：`chat_turn/character.rs::chat_with_character` → `transport::finish_character_reply` 剝前綴與控制區塊。
- 推進迴圈在前端 `useChatController.gmAdvance`：`narrateOnce` → `next` 非玩家就 `callAndReply` → 再 narrate，上限 `max_round_speakers`（預設 3）；`submitTurn(relay)` 只做「旁白＋接一句」不迴圈。
- 事件已有 ULID `id`（落檔配發）；幕＝`current_scene`＋`scene_labels{base,version,parent,forked}`。
- GM system 穩定前綴由 `transport::gm_lane_system_with` 組；導演指示在回合尾（每輪重組、不吃快取）。

## 1 三件定死（之後各包不回頭改）

1. **格的識別**＝`<事件 ULID>#<格序>`。格由 Rust 解析器從事件 `text` 決定性導出，**不另存格**；解析器改規則＝格序可能變，所以已讀標記、存檔點（包 6）只存事件 id＋格序，對不上時退到該事件第 0 格。事件 `text` 存模型的劇本文字原樣（只剝既有的圍欄、更新區塊、點名行；角色線另過濾見 §3），聊天視圖直接看得到標記。
2. **分支識別**＝既有內部幕號＋`scene_labels`；從選項點分岔（包 6）建新幕並在標籤加「分岔自哪一則事件」欄，包 1 不動。
3. **終止標記優先序**：同一則 GM 事件裡以「驗證後仍有效」的標記為準，`[ending]` > `[chapter]` > `[choices]`；高優先者生效時低優先者從格清單剝掉（不顯示）。**「停接力」與「壓制其他終止格」是兩件事**（Sol 第 2 輪必改 2）：三種標記任一出現都停接力、球交回玩家；但只有「驗證後有效」的標記才取得壓制資格。包 1 的 `[chapter]`／`[ending]` 解析成格、不驗證、**不取得壓制資格**（同則出現 `[ending]`＋有效 `[choices]` 時選項照留），包 5 接上驗證後才壓制。

## 2 解析器 `chat_turn/vn_script.rs`（純函式，單一來源）

輸入：事件顯示文、名單（角色 id↔名、玩家名）、每角色表情清單（包 1 固定五種）。輸出 `Vec<VnCell>`：

```
Narration { text }
Dialogue  { speaker_id: Option<id>, speaker_name, expression, variant: Option<String>, thought: bool, directive: Option<Closeup|Shake>, text }
Bg        { location, time: Option<String> }
Choices   { options: Vec<String> }
Chapter
Ending    { id }
```

規則（照總綱 §3，寬容）：
- 空行分格；連續旁白行併成一格；對白行永遠自成一格。
- 對白行＝`名字 (表情[ @變體][, 指令])[ thought]: 台詞`，冒號全半形皆可，台詞外層的「」／“”／"" 剝掉；括號內容整組都可省。名字對上名單就填 `speaker_id`，對不上也算對白格（`speaker_id: None`，名牌照出、不擺立繪——沒有卡的配角）。表情不在清單退 `neutral`；指令只認 `closeup`／`shake`，其餘忽略；每則事件最多 2 個指令，超出清掉；`@變體` 原樣保留，解析在包 7。
- `[bg: 地點 | 時間]`、`[choices]`＋`- ` 行、`[chapter]`、`[ending: id]` 都是整行、行首標記、關鍵字英文不分大小寫。`[choices]` 選項少於 2 個＝整塊丟掉（沒有選項）；多於 4 個取前 4。
- 認不得的 `[...]` 行與零散文字併入相鄰旁白格；整段沒有任何標記＝一格旁白。
- 同一事件多個 `[choices]` 取最後一塊；§1 第 3 條的優先序在解析器出口套用。

另提供 `filter_character_reply(text, self_id, roster)`：角色線只留旁白格與自己的對白格；別人名字的對白格改成旁白（文字原樣）；`[bg]`／`[choices]`／`[chapter]`／`[ending]` 行整行剝掉。回傳過濾後的文字（落檔用 `text`，`raw` 照舊存原文）。剝 `[choices]` 連其下的 `- ` 選項行整塊剝（不留選項散文）。

**前綴規則要涵蓋整條角色傳輸路徑**（Sol 必改 2）：查證「名字：」前綴在四處共用同一字面——串流剝除 `OwnPrefixStream`（`chat_turn/character.rs:213` 與 `lanes/mod.rs:667`）、落檔收尾 `finish_character_reply`、回聲對帳 `lanes/plan.rs::expected_reply_for`（它用同一個 `finish_character_reply` 算「前端落了什麼」，對不上就 ReplyDiverged 重開 session）。VN 對白行省略表情時就是 `名字: 「台詞」`，任何一處剝了前綴都會變成沒有說話者的旁白格或讓對帳失準。所以 VN 桌的規則只定一次、四處共用：
- VN 桌的角色線 **own prefix＝空字串**（`OwnPrefixStream::new("")` 直通、`finish_character_reply` 不剝），正文整理只剩控制區塊與未閉合尾巴處理，再套 `filter_character_reply`；這個組合封成一個純函式 `finish_vn_character_reply(reply, self_id, roster, aborted)`，`chat_with_character` 與 `expected_reply_for`（`ReplyEcho::Dialogue` 加 `vn: Option<VnFilterSpec>`）都呼叫它，落檔文字與預期回聲逐字相同。
- VN 桌送模型的歷史，角色對白事件**不加** `speaker_prefix`（`transport/turns.rs:61`；文字本身已含名字，加了模型會學成「名字：名字 (表情): 台詞」）；GM 事件照舊加「GM：」。
- 解析器測試含無表情寫法 `名字: 「台詞」` 與有表情寫法各一。

Tauri 命令 `read_vn_cells(world_id, scene) -> Vec<{event_id, kind, cells}>`：把一幕事件逐則解析（player／system 事件原樣包成一格旁白或略過，由包 2 決定顯示），包 2 播放器只消費這個，**不另寫 TS 斷格器**。

**串流契約**（Sol 方案審第 1 輪必改 1、第 2 輪必改 1）：串流中前端只有增量字串，所以解析器另有一個「未落檔文字」入口 `parse_vn_text(world_id, source, text) -> {cells, tail_tentative: bool}`（同一個解析器、同一份名單）。`source` 是 `Gm` 或 `Character{self_id}`，入口**先做各自的顯示文清理、再解析**：GM＝剝已閉合的 ```state 圍欄與 `<UpdateVariable>`、剝「下一位」行、切掉未閉合的控制區塊尾巴（照 `gm_reply::aborted_narration` 的做法：`strip_self_closing_controls` → `extract_state_block` → `extract_next_speaker` → `cut_unclosed_tail`）；角色＝套同一個 `finish_vn_character_reply(…, aborted: true)`。最後一格標 `tentative`（可能還在長），**串流中不套會刪格的規則**（多個 `[choices]` 取最後一塊、終止優先序剝除、指令每事件上限都只在落檔版套用）。前端節流呼叫（每 100–200 ms 或每個換行），畫面以「暫定格」顯示；事件落檔後改讀 `read_vn_cells` 的該事件正式格，暫定格整組換成正式格；因為刪格規則只在正式版套，格序不保證對得上，所以**正式化時播放游標重設到該事件玩家已讀到的文字位置**（以已顯示文字長度對位，不以格序對位），包 2 實作。包 1 只做命令與清理→解析的純函式，播放器在包 2。

**優先序與驗證的接口**：終止優先序在 `apply_terminal_priority(cells, accepted: impl Fn(&VnCell) -> bool)` 套用，`accepted` 回傳「這個終止格是否取得壓制資格」：包 1 對 `Chapter`／`Ending` 恆 **false**、`Choices` 恆 true（所以包 1 實際只剝「多個 `[choices]` 取最後」）；包 5 換成「結局 id 在表內且條件成立」與「章結束流程受理」（達上限仍進強制收尾、不是拒收 `[chapter]`）；解析器本身維持純語法。

## 3 導演指示與格式規範

- `GmTurnFormat::VnScript`：`table_kind == vn` 時一律選它（VN 桌不走卡片格式與介面接管；有重構介面殼的卡匯進 VN 桌照樣 VnScript，列已知限制）。
- 新檔 `transport/vn_format.rs`（response.rs 已 728 行）：
  - `vn_format_rules(lang, expressions)`：格式規範全文（§3 範例＋表情清單＋指令上限＋選項 2–4、只在分岔點、放尾端＋`[bg]` 只在變動時寫、換地點＝清場＋角色名照名單原字＋對白短句、旁白惜字的文體要求），**放 GM system 穩定前綴**（`gm_lane_system_with` 依桌型附加一段），不放回合尾。
  - `vn_narrate_instruction(lang, roster, player_name, updates, statusbar_retired)`：回合尾導演指示＝「以 VN 劇本格式寫這一拍」＋ ```state 圍欄（time／place／present 照舊，換幕結算與在場判定沿用）＋增量桌的 `<UpdateVariable>` 註記＋「下一位」行（照舊；本拍以 `[choices]` 收尾時仍寫，引擎忽略）。
  - `vn_character_rules(lang, expressions)`：角色線 system 附加段，**不帶名字**（`chars_lane_system` 是全卡共用的穩定前綴，這一輪演誰在回合尾指定）：只輸出被指定角色自己的對白格（可多格、可 thought、可表情），不寫 `[bg]`／`[choices]`／`[chapter]`／`[ending]`，不替他人與玩家說話。每角色的額外表情與變體清單（包 7／9）之後放回合尾、跟指定演誰同一段。
  - GM 的輸出順序寫死在指示裡：正文（含 `[bg]`、對白格）→ `[choices]` 區塊 → ```state 圍欄 → 「下一位」行；並要求圍欄的 `place`／`time` 與最近一次 `[bg]` 一致。
  - 收尾句各一（zh／en 骨架，其餘語系沿用 `scaffold_en` 既有分流）。
- 文體要求放格式規範，不另加「VN 文體」指示段。

## 4 停點與推進

- 後端 `gm_narrate` 提交前：VN 桌的回覆過解析器；格清單含 `Choices`／`Chapter`／`Ending` 任一 ⇒ 回傳的 `next` 改成 `None`（不是 `__PLAYER__`：點到玩家會在 `callAndReply` 多寫一則「GM 請玩家發言」系統列，VN 不要這一列，`None` 在 `gmAdvance`、`submitTurn` 的 relay 與 `rememberCall` 三處都是「就此停下、不寫任何東西」），`GmNarration` 另加 `vn_stop: Option<"choices"|"chapter"|"ending">` 說明停的理由。既有 `gmAdvance` 迴圈因此自然停下，**迴圈本體不改**。施工時確認前端沒有別處靠 `__PLAYER__` 判「輪到你」。
- **選項與點名同時出現的讀法**〔模型判斷·未裁決〕：`[choices]` 一出現就停、忽略「下一位」，被點名的角色不再接話。總綱 §3「選項格永遠排在該回合所有線的格之後（角色還在接話時選項先扣住）」讀成顯示層的規則（包 2 播放器把選項格扣到本回合已落的格都播完），不讀成「點名角色先接話、選項再顯示」；理由：GM 寫 `[choices]` 時劇情已到分岔點，角色再接一句會把分岔點推走。
- 前端：`useChatController` 新增輸入 `vnTable: boolean`；VN 桌的 `submitTurn` 在玩家句落檔後改走 `gmAdvance` 的迴圈（抽出共用 `advanceFrom(started)`），而不是「旁白＋接一句」。到上限沒出選項＝停下，玩家按「繼續」（包 2 的鈕）走 `gmAdvance`。
- 角色回合：VN 桌提交前過 `filter_character_reply`。

## 5 換一組 `vn_reroll_choices(world_id)`〔作者裁決 2026-11-01：只重生成選項塊〕

- 前置：本幕最後一則故事事件是 GM 旁白且解析出 `Choices`；否則回錯。
- 呼叫：**不呼叫 `gm_lane_reply`**（它內建 `world_scan::landing::land` 會落世界書計時與變數，`lanes::run_turn` 會更新正式 GM session 與預期回聲；Sol 必改 3）。改成獨立的一次性呼叫：只重用上下文**組裝**（`chat_assembly::gm_lane_parts` 組出凍結 system＋回合尾，掃描用唯讀 `readonly_lane_scan`、不落地），導演指示改成「只輸出一個新的 `[choices]` 區塊（2–4 個、與目前這組不同），不要正文、不要圍欄」，呼叫走換幕摘要那種一次性路徑（`scene_budget/summarize.rs` 的 caller／`transport::dispatch::stream_via_transport`，不經 `lanes::run_turn`）；非串流、不落新事件。不套 `gm_commit_apply`、不落世界書、不結算在場、不開 TurnGuard、不記回合、不動 GM session；只記 usage log 與改寫事件文字。驗收測試：換組前後 `state.json` 與世界書落地檔逐位元相同。
- 結果：解析出 ≥2 個選項才接受；在那則事件的 `text`（與 `raw`）裡把舊 `[choices]` 區塊換成新的，**事件 id 不變**（新 data 函式 `replace_transcript_text(root, world_id, scene, event_id, text, raw)`，照 `set_last_transcript_state` 的寫法走同一寫入閘門）；回傳新選項。失敗＝原組保留並回錯。
- 額度照 narrate 記 usage log。前端按鈕在包 2。

## 6 預算、落點、測試

- 核心估 **400**（Sol 第 1 輪：分項合計 380 偏緊，加串流入口）：解析器＋過濾＋串流入口 170、指示與規範 80、gm.rs 停點與角色線四處接線 60、換一組 70、前端迴圈 20；寫到 800 停下檢討。新檔 `chat_turn/vn_script.rs`、`transport/vn_format.rs`、`commands/vn.rs`（薄殼）；gm.rs 391、useChatController 905 只加接線；`useChatController` 若因抽出共用迴圈而超 1000，把推進迴圈抽到 `play/chat/advance.ts`。
- 測試（≤核心 2 倍）：解析器表驅動（寬容案例每條一組）、過濾、停點覆寫、優先序剝除、換一組的改寫與拒收、VnScript 指示的選擇。不跑測試通道（無介面行為；包 2 再跑）。
- 不碰：`[chapter]`／`[ending]` 語意（包 5）、變體與額外表情（包 7／9）、旗標協定抽象（包 4）、播放器（包 2）。

## 7 已知限制

- 聊天視圖看 VN 桌紀錄會看到原始標記（`[bg]`、`[choices]`）；VN 視圖才排版。
- 模型把對白寫成「名字：台詞」以外的寫法（如「名字說：」）＝旁白格，靠格式規範與範例壓低頻率；不做語意猜測。
- VN 桌沿用 ```state 圍欄，`[bg]` 與圍欄的 place 仍可能不一致（指示已要求一致）；演出以 `[bg]` 為準、換幕結算以圍欄為準。
- 解析器用「現在的名單」對名字：角色改名後，舊事件的對白格對不到 id（名牌照出、立繪不擺），與既有「事件存發言當下的顯示名」拍板一致。
- `事件 id＋格序` 在文字被改寫（換一組）或解析規則變動後可能「索引仍在但指到別格」；包 6 的已讀與存檔點加文字指紋（Sol 建議），包 1 不做。
