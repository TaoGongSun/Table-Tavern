# room-host-actions — 房間的房主操作（重來這一回合、GM 鈕、重開補送）

**狀態：方案定案；包 1、包 2 施工中，包 3 等階段二包 2 落地。**

上層：[multiplayer-room.md](multiplayer-room.md)、協定 [multiplayer-room-protocol.md](multiplayer-room-protocol.md)、階段一 [room-host-desktop-play.md](room-host-desktop-play.md)。排序〔作者裁決 2026-11-03：「另排一個階段」〕：階段二 `room-player-cards` 之後、階段三 `room-seat-delegate` 之前。

**案別：大案**，核心估約 620 行（上限 850；Sol 方案審第 1 輪後重估，原 560），分三包（§6）。方案與驗收各送一位審查（Sol 優先）；動到回合狀態機與執行端落檔／回捲，本來就要方案審。協定增補先寫在本檔 §3，施工時併進協定文件（階段一還在那份檔上收尾，本分支不動它）。

## 1 作者原話與原則〔作者裁決 2026-11-03〕

- 基準：「基本就是跟現在一樣只是做成連線版。」
- 重來這一回合／收回：GM 這段寫歪、被拒答後換模型想重生、玩家誤送——房主能把上一回合 GM 段落作廢、玩家行動退回草稿、重新結算（單人版有「收回上一句」、復原、停止後重送）。
- 房主 GM 旁白／GM 推進／請某人發言：沒座位、只想當主持的房主也能推動故事（單人版有這三顆鈕）。
- 關房後房主把那張桌當單人桌玩了幾輪，重開房間時把這段劇情補送給朋友，讓朋友看到的等於 GM 讀到的公開劇情（作者選「重開時補送給朋友」）。

單人版怎麼做就照做；以下只寫連線版多出來的部分。

## 2 成品樣子

按鈕都在 party 的房間元件裡（`party/src/features/controls/` 與回合區），桌面房間畫面與手機房主共用同一套；條件不成立時停用並寫原因（同階段一「開始鈕」）。位置照單人版：GM 三顆鈕在行動格那一列，「收回」在故事串最後一段 GM 下方。

- **GM 旁白**（房主）：GM 寫一段旁白，點名不自動接話；點到 NPC 時出現「讓 X 接話」（同單人版 `gmNarrate`＋`callNominated`）。
- **GM 推進**（房主）：GM 旁白＋點名接力，到點名真人玩家、沒點名或 `max_round_speakers` 停（同單人版 `gmAdvance`，就是現有派工鏈不帶行動）。
- **請 X 發言**（房主）：從 NPC 清單選一張，那張卡直接接一句（同單人版 `requestReply`）。只列沒人認領、非舞台、未隱藏的卡；真人玩家的卡由玩家自己講。
- 三顆鈕在 `acting` 時可按，**沒有人入座也能按**（§3.2「名單空不結算」防的是無人值守迴圈，手動按鈕沒有迴圈）；要執行端在線。按下時這一回合已送出的行動退回草稿（同換幕，沿用 `round.cancelled`），GM 那段跑完開新的 `acting`，草稿原樣帶著。
- **重生這段**（房主；出額度者另可同時換路線／模型）：上一回合 GM 段落作廢，用**同一批行動**重跑 GM。管「寫歪了」「軟性拒答（拒答文字當正文落地）想換模型重來」。換了之後這一幕剩下的回合都用新的，全員看到「GM 改用 <模型>」〔作者裁決 2026-11-03：重生時可以換〕。
- **收回這回合**（房主）：上一回合 GM 段落作廢，那批行動退回各人草稿，開新的 `acting`；大家改完再送、房主照常結算。管「玩家誤送」。
- 兩者都要確認「GM 這段會作廢，所有人都會看到它消失」。收回就定案，不做復原〔作者裁決 2026-11-03〕。故事串撤掉那幾段，留一行系統提示「房主收回了上一回合」。
- **停止後重送**：既有 `gen.abort`（半截保留、開下一回合）＋「重生這段」組起來就是，不另做。
- **重開補送**：重開房間時，執行端把桌上目前的公開劇情跟房間故事串對齊：房主單人玩的部分補上；單人時收回掉的、房間還顯示的段落撤掉；單人時換過幕，房間也跟著補摘要、換幕。補完才開始結算。有補送內容時，暫停期間已送出的行動退回草稿，讓大家看完新劇情再送；沒有補送內容就不動〔作者裁決 2026-11-03〕。CastRail 在暫停期間改過的卡，重開時一併重送名冊。

## 3 協定增補〔模型判斷·未裁決，送方案審〕

### 3.1 房主 GM 回合

客戶端 → 伺服器：

| t | p | 誰 | 合法狀態 | 伺服器處理 |
|---|---|---|---|---|
| `host.gm` | `{ round, mode: "narrate"\|"advance"\|"call", cardId? }` | H | `acting(round)`、執行端在線且未暫停（§3.3）、沒有換幕或收回在跑；roster 可空 | `call` 時 `cardId` 必須是名冊裡 `playable`、未認領、未隱藏的卡，否則 `E_SEAT_TAKEN`。發 `round.cancelled(round)`（已送出的退回草稿），開 `round.opened { kind: "host" }`（r+1、直接 `queued`），派工 |

- `RoundKind` 加 `"host"`；`Dispatch` 加 `host?: { mode, cardId?, call?: boolean }`，`actions` 為空，`arrivals` 為空，`departures` 照常帶（離席標記照落）。
- `call: true`＝「讓 X 接話」（先寫點名列再接話，同 `callAndReply`）；`false`＝「請 X 發言」（不寫點名列，同 `requestReply`）。
- `gen.done` 加 `called?: cardId`：`narrate` 模式 GM 點到 NPC 時執行端回報；伺服器存在下一個 `acting` 回合（`RoundView.called`，只給房主看），那回合一結束就失效——對應單人版「最後一則故事已不是那則旁白就不接」。
- 失敗照現有：`gen.failed` → `held`，`retry`／換模型／`round.continue` 都通用；`round.continue` 開 `acting(r+1)`。
- 冪等與單飛：完全沿用派工的 `dispatchId`、重送佇列、`InflightReport`；一房同時只有一個 GM 請求在飛不變。

**不走 `ctl`**（回應審查員「共用房主遠端控制執行端通道」的建議）：`ctl` 只給出額度者、30 秒逾時、不進事件日誌也不經回合狀態機。GM 鈕和收回都會改回合與故事串，必須走伺服器狀態機（派工／`job.*`），否則伺服器與桌上會對不上。共用的是**伺服器轉執行端的 `job.*` 管道**（`jobId` 冪等、`welcome` 帶未完成工作重送、15 分鐘無進度標記）：收回用 `job.undo`，階段三手機房主「暫停」用 `job.pause`（本案只預留種類名，不做）。

### 3.2 收回與重生

**幕設定放寬**〔作者裁決 2026-11-03〕：原「同一幕固定、換幕可換」改成「同一幕固定，只有**重生**與**失敗重試**（`held` 時的 `switch_model`／`switch_route`）能換；換了之後這一幕用新的」。併進協定時改這幾處：計畫 §1「例外」條、協定 §2 `SceneSettings` 註解、§7.3「唯一的幕中切換」改成「兩種幕中切換」，`scene.settings_switched.reason` 多 `"regenerate"`。

客戶端 → 伺服器：

| t | p | 誰 | 合法狀態 | 伺服器處理 |
|---|---|---|---|---|
| `round.undo` | `{ round, mode: "regenerate"\|"edit", next?: { route, model } }` | H；`next` 只限房主本人是出額度者（否則 `E_FORBIDDEN`，同換幕） | 目標回合（`round`）＝最後一個做完的派工回合：當前 `acting(round+1)` 且它由該回合的 `gen.done` 開出、同一幕；或當前 `held(round)`。執行端在線且未暫停、沒有換幕或另一個收回在跑 | 存 `undo { jobId, round, mode, next }`，送執行端 `job.undo { jobId, dispatchId }`；期間所有回合指令回 `E_ROUND_NOT_ACTING`。重按回 `E_STATE_CONFLICT` |

- 伺服器記 `lastDispatch { round, dispatchId, kind, scene, batch, arrivals, departures, host? }`，在 `gen.done`／`gen.failed` 時更新；收回一次就清掉——**只收一層**（§5 已知限制）。種子歷史（round 0）、補送段落（§3.3）、上一幕都不是目標。
- 執行端回 `job.result ok` 後，伺服器同一筆交易：
  1. 當前是 `acting(round+1)` 就發 `round.cancelled(round+1)`（那回合已送出的退回草稿）。
  2. 發新持久事件 **`round.undone { round }`**：所有讀故事串的地方把該回合的 GM／NPC／系統段與玩家行動段（負段號）都拿掉。
  3. `regenerate`：照原回合種類重開 r'（`play` 帶原批次直接 `queued`；`open`／`scene_open`／`host` 照原設定），新 `dispatchId`，`arrivals`／`departures` 用原派工的。有 `next` 就先 `offered()` 驗、改 `sceneSettings`、發 `scene.settings_switched { reason: "regenerate" }`。
  4. `edit`〔模型判斷·未裁決；作者提案、常識審查員確認〕：伺服器把原批次的 `act` 文字一律存進只給本人的 `returned[memberId]`，草稿不動（跟草稿一樣不公開、落地、進本人快照 `me.returned`），以 `tx.toMember` 送 `vol { type: "returned", text }`（同 `quota_detail`／`error_detail` 的私人路徑；補事件的重連在補完事件後也送）。客戶端在套用到畫面那一刻判斷（重連、重開頁面同一條規則）：**輸入框空著**（去掉空白後）就把原文直接放回輸入框、上方顯示一行「房主收回了上一回合，改一下再送或直接送」，送出後消失，並送 `action.returned_dismiss {}` 清掉伺服器那份；**已有草稿**就草稿不動，行動格上方顯示「被收回的行動」＋原文，「帶回」接到草稿尾端、「丟掉」清掉（都送 `action.returned_dismiss`，伺服器回 `text: null`）。不截字（超過 2000 字照現有送出驗證擋）。當回合按跳過的人沒有原文，只看到故事串的收回提示。連續收回的提示合併成一行。下一次收回覆寫 `returned`；送出行動不清它。`round.undone`／`round.opened` 不帶私人原文。`arrivals` 放回「待登場」、`departures` 放回待帶清單。開 `acting` r'（`play`）。原回合是 `open`／`scene_open`／`host`（沒有行動）時 `edit` 等於只作廢、開 `acting`。
- 執行端回失敗（例：那批事件已不在桌尾）：狀態不動、清掉 `undo`，房主收 `E_STATE_CONFLICT`。
- 冪等：`jobId` 照現有 `findJob`／`doneJobs`；執行端以「桌尾還有沒有這個 `dispatchId` 的事件」判斷，已收過就直接回 ok。
- 讀故事串的三處都要認 `round.undone`：`snapshot.ts` 的 `storyOf`（快照）、`party/src/features/room/room-state.ts`（客戶端逐則套用，要在既有清單上過濾，不只靠 `storyOfFrames`）、`export.ts`（匯出自己的迴圈）。

### 3.3 重開補送與同步閘門

**閘門**：
- 執行端關房（暫停）時，`Saved` 記 `paused: true`，然後照階段一送 `status { paused: true }` 再斷線。
- 伺服器的 `executorHello`（`reduce.ts`）如果看到 `exec.paused` 為 true，就**不清 `paused`**，改設 `exec.sync = "needed"`。之後到「完整分片的 `story.sync` 交易成功」為止，下列入口全部關閉：
  - `welcome` 帶的 `dispatch` 一律給 null（執行端暫停時已中止鏈，本機不會有在跑的派工）；`executorHello` 結尾的 `pushDispatch` 不跑。
  - `round.ts` 的 `pushDispatch`、`settle`（結算條件 1）、`onAlarm` 的倒數到期結算都不動作，倒數保持暫停。
  - 成員指令 `room.start`、`round.resolve`、`host.gm`、`round.undo`、`scene.advance`、`seat.create` 回 `E_EXECUTOR_OFFLINE`。
  - 執行端 `status` 訊框（`link.rs` 的 `welcome` 會在同步前送一則 `paused: false`）在閘門期間不得清掉 `paused`。
- 用一個 `execReady(s)`＝`online && sync !== "needed"` 取代上述各處現在的 `exec.online` 判斷，一處定義、各處共用。
- 閘門期間仍照收：重送佇列裡的 `gen.part.end`／`gen.done`／`gen.failed`／`job.result`（這些是暫停前那批的結果，不是新派工；`gen.done` 開出的 `acting` 照開，但不結算）、`capabilities`、`roster.update`、`quota`。
- 一般斷網重連（`paused` 為 false）不設閘門、不送 `story.sync`，行為照現狀。
- `story.sync` 交易成功時清 `sync` 與 `paused`，執行端收到最後一片的 ok 再清本機 `Saved.paused`；沒收到 ok 就斷線的話，下次重連照樣有閘門、重新送一次。

**執行端 → 伺服器**：

| t | p | 合法時機 | 伺服器處理 |
|---|---|---|---|
| `story.sync` | `{ part, total, keepThrough: { round, part } \| null, items?: Array<{ part: StoryPartLike } \| { summary: string }> }` | 只在 `sync === "needed"` 時收；分片規則同 `room.seed`（每片 ≤ 512 KiB，中途斷片 `seed_restart`） | 到齊後一筆交易，順序見下 |

伺服器的處理順序：
1. 如果是 `held`，先照 `round.continue` 收掉。
2. 發 `story.truncated { after: keepThrough }`：本幕故事串保留到該段為止，之後全部撤掉；`null`＝本幕全撤。`keepThrough` 指不到本幕現有段落就回 `E_STATE_CONFLICT`，執行端改送 `null` 全量重送。
3. `items` 依序處理：
   - 段落：發 `story.part`，新配一個回合號（`++roundNo`），段號依序，說話者照 `room.seed` 的驗法。
   - `summary`：比照 `executor.ts` 的 `sceneResult` 做完整換幕：當前有回合就發 `round.cancelled`（已送出的行動退回草稿）→ 發 `scene.summary`（帶 `round`），`summarySeqs` 推入它的 `seq` → `scene + 1`，幕設定不變，發 `scene.started` → `sceneStartSeq = seq`。之後的段落就屬於新幕。
4. 有換過幕的話，最後開一個 `acting`（`play`）。不開 `scene_open`，因為新幕開頭單人時已經玩過，不要 GM 再開場一次。
5. 有補送內容（`items` 非空或撤了段）而且當下是 `acting` 時，該回合已送出的行動退回草稿（廣播 `action.retracted`）〔作者裁決 2026-11-03〕。第 3 步已經 `round.cancelled` 過的不重複退。
6. 清 `sync` 與 `paused`，重判結算，`pushDispatch`（處理閘門期間停在 `queued` 的回合）。

- 沒有差異也要送一則（`keepThrough`＝最後一段、沒有 `items`），它就是開閘門的訊號。
- **執行端怎麼比**：`welcome` 的快照帶房間本幕故事串。執行端用 `seed.rs` 的公開投影（抽成可指定幕號）算出桌上對應那一幕的段落，只比兩邊都有的種類（GM、NPC、玩家行動、幕摘要；觀望、狀態更新等系統段兩邊都略過），以（說話者種類、卡 id、正文）找最長共同前綴，得出 `keepThrough`；其後的桌上段落當 `items`。
- **單人時換過幕**：`Saved` 加 `synced_scene`，記房間目前那一幕對應的桌內幕號；建房、換幕工作、`story.sync` 成功時更新。桌上 `current_scene` 比它大時，`items` 依序是：舊幕剩下的段落 → 舊幕摘要 → 下一幕段落 → …… → 目前這幕的段落。
  - 桌面版把第 N 幕的摘要寫在第 N+1 幕逐字稿開頭（`SceneSummary` 標記，見 `run.rs::summary_of`）。那一則只拿來當 `summary` 項，**不再當一般段落補一次**；每一幕的投影都略過開頭的 `SceneSummary`。
- 補送段落的說話者：玩家句照種子做法（`memberId` 空、卡 id 照名冊），NPC 照卡名對名冊，對不到的不送。
- `welcome` 的送出順序：重送佇列 → `capabilities` → `status` → `roster.update`（暫停期間 CastRail 改過的卡）→ `story.sync` 各片。

### 3.4 新事件與型別一覽

- 即時訊息：`vol.returned { text | null }`（只給本人）。
- 持久事件：`round.undone { round }`、`story.truncated { after }`；成員指令 `action.returned_dismiss {}`；快照 `me.returned: string | null`；房間狀態 `returned`、`exec.sync`；`round.opened.kind` 多 `"host"`；`scene.settings_switched.reason` 多 `"regenerate"`。
- 伺服器 → 執行端：`job.undo { jobId, dispatchId }`；`Job` 聯集加 `{ kind: "undo" }`（`job.pause` 只預留名字）。
- `RoundView` 加 `called: string | null`（只房主看得到內容）。
- 錯誤碼不新增。

## 4 執行端做法（重用單人版路徑）

- **GM 三模式**：`run.rs::steps` 已有 `Narrate`／`Reply(card)`／`used >= max`，`DispatchRun` 加模式即可：
  - `narrate`：落批次（只有離席標記）→ 一次 `narrate` → 落正文與狀態更新、**不寫點名列** → 停；點到 NPC 就把卡 id 放進 `gen.done.called`。
  - `advance`：現有鏈原樣。
  - `call`：起點 `Reply(cardId)`，`call: true` 先寫點名列，接一句就停。
- **收回**（`job.undo`）：`data/scene/transcript/floor.rs` 加 `pop_action_floors(root, world, scene, action_id)`。在同一把 `with_commit` 裡讀一次本幕事件，用既有 `floor_start` 在記憶體裡從尾端逐樓往前算，**每樓先看開頭的 `action_id`**，不是這個派工就停，得出切點；然後一次 `edit_scene` 原子重寫，再 `rewind_state` 一次（狀態樹與變數表回到那批之前，同單人版收回）。結果分三種：本幕完全沒有這個 `action_id` 的事件＝已收過，回 ok；尾段有、切掉＝成功；這個派工的事件還在較早的位置、尾段卻是別的＝回失敗（`not_tail`），什麼都不改。持 `world_write_permit`、`table_changed`。不留單人版的復原疊（收回不做復原〔作者裁決 2026-11-03〕）。
- **重生**：伺服器開新派工，執行端照一般派工跑（落同一批行動、新 `action_id`）。
- **重開補送**：`seed.rs` 的 `history()` 抽出「某一幕的公開投影」；`link.rs::welcome` 在處理 `dispatch`／`job` 之前、回送佇列之後組 `roster.update`＋`story.sync` 分片，排在同一條送出順序裡。
- 前端（桌面版）不用新寫：房間畫面就是 party 元件。

## 5 已知限制與待作者決定

已知限制〔模型判斷·未裁決〕：
- 只收一層：收回或重生之後，再往前一回合收不了（單人版可以連按往回收）。
- 收回後故事串上留系統提示，GM 段消失對所有人可見。
- 收回後不能復原（單人版有）〔作者裁決 2026-11-03〕：按之前確認；收錯只能重生。

作者已裁決（2026-11-03）：收回不做復原；重開有補送內容時已送出的行動退回草稿；重生可換路線／模型（§3.2）。目前沒有待作者決定的題目。

## 6 分包

| 包 | 內容 | 核心估 |
|---|---|---|
| 1 GM 鈕 | 伺服器 `host.gm`、`host` 回合種類、`called`；執行端三模式；房主三顆鈕＋「讓 X 接話」＋卡片挑選；i18n 十語 | 190 |
| 2 收回／重生 | 伺服器 `round.undo`＋`lastDispatch`＋`job.undo`＋三處 `round.undone`；被收回行動另存 `returned`＋「帶回／丟掉」；重生換模型；執行端 `pop_action_floors`；兩顆鈕＋確認 | 240 |
| 3 重開補送 | 同步閘門（`execReady` 取代各處 `exec.online`、`executorHello` 不清 `paused`）；`story.sync` 分片、`story.truncated`（三處讀故事串）、跨幕比照 `sceneResult`；執行端投影抽幕號、略過開頭摘要、共同前綴比對、`synced_scene`、`Saved.paused`、名冊重送 | 190 |

三包可依序做（包 3 跟前兩包無相依，可並行）。測試上限＝核心 2 倍，故障注入只用在「`pop_action_floors` 中途失敗不得收一半」與「`story.sync` 中途斷片不落地」兩條資料遺失路徑。

## 7 驗收

- 每包：`npm run verify`、party 的 test 與 typecheck、Rust 測試。
- 伺服器核心測試：`host.gm` 在空名單、送出一半時（草稿退回）、執行端離線；`round.undo` 兩種模式各一條，含 `held` 收回、重按、與結算同時到、收回中執行端斷線重連（`welcome` 帶工作、同 `jobId` 收尾）、非房主帶 `next`；`round.undone`／`story.truncated` 在快照、客戶端、匯出三處結果一致；閘門期間 `welcome` 不帶派工、`status paused:false` 不解除、全員已送出與倒數到期都不結算、各成員指令回 `E_EXECUTOR_OFFLINE`，完整 `story.sync` 後才結算與派工；跨幕補送後 `sceneStartSeq`／`summarySeqs` 與快照一致、摘要不重複；收回時原文都進 `returned`、`vol.returned` 只送本人；客戶端輸入框空著就直接放回並提示一行，有草稿就草稿不動、另顯示原文可帶回（接在後面）或丟掉。
- Rust：三模式落檔對照單人版（旁白不寫點名列、`call` 寫點名列）；`pop_action_floors` 收乾淨含登場塊與離席標記、狀態樹回捲、不在桌尾拒收；補送比對（純追加、單人收回過、單人換過幕且摘要不重複）；`pop_action_floors` 三種結果（已收過、成功、不在桌尾）。
- 測試通道實測（本機 Node 殼＋桌面版＋瀏覽器當手機）：沒座位的房主按旁白、推進、請 NPC 發言、讓 X 接話；兩人送出後房主重生（換模型一次）、收回後改字再送；房主收回時在線的人**不用重連**：輸入框空著的人原文直接回到輸入框並看到提示，有草稿的人草稿不動、看到原文能帶回（接在後面）或丟掉；關房 → 單人玩兩輪並收回一句 → 重開，手機故事串等於桌上公開劇情；單人時換一次幕再重開。

## 8 觀察

- 桌面版房間畫面上方的房間資訊列很高，加了房主故事按鈕列後故事串只剩約 110px（1136 高的視窗）；已把收回與 GM 鈕併成一列、放不下才換行〔模型判斷·未裁決〕。
- 常識審查員建議（驗收第 1 輪）〔模型判斷·未裁決〕：收回／重生鈕改叫「重生上一回合」「收回上一回合」；GM 旁白、GM 推進下方加一行看得到的說明（各做什麼、都不結算、已送出的退回草稿）。
- 常識審查員驗收第 2 輪〔模型判斷·未裁決〕：GM 鈕說明改成各鈕 tooltip＋列尾「?」點開才列出，不常駐；桌面版資訊列「最近錯誤」改一句人話＋「詳情」（遮掉 user_id 與金鑰片段），錯誤碼分類放 `party/src/features/room/gm-error.ts`，跟重生時的換模型選單共用（單人版 `explainAiError` 只分到「API 拒絕請求」這一層，404 下架說不出來，所以只重用它的遮罩）。
- 卡住時重生〔模型判斷·未裁決；常識審查員共識〕：重生鈕不灰、不改字；上次是模型類錯誤（下架、金鑰／登入）就在換模型選單灰掉那個模型、預設選第一個能用的（不另存「上次成功的模型」）；暫時性錯誤預設原模型並提示可直接重試。
- 常識審查員驗收第 3 輪〔模型判斷·未裁決〕：執行端錯誤分類加 `model_unavailable`（404、`cli_model_unavailable`，可做的動作＝換模型／換路線／重試），回合列寫「GM 卡住：模型已下架或叫不到，等房主換模型」；`not_ready` 改寫「金鑰或登入失效，等房主處理」；`provider_unreachable` 原寫「重試中」但卡住時並不會自動重試，改成「等房主重試」。卡住且 GM 一段都沒寫出來時，重生警語改成「會用選的模型重跑這一回合」；停用的原模型鈕寫「用原模型重生（已下架）」；換模型下拉加「路線」「模型」標籤；「?」移到操作區最右邊。重生選單的灰掉判斷改看公開的卡住種類（`holdKind`，跟資訊列的錯誤碼對照放在同一個 `gm-error.ts`），非出額度的房主也看得到。
- 送出行動後行動格上方寫「已送出：（前 20 字）…（結算前可以收回修改）」，取名冊上本人那格的原文，結算或收回後消失；階段一原本只有一行「已送出，結算前可以收回修改」、看不到送了什麼〔模型判斷·未裁決；常識審查員〕。
- 帶回改成草稿與清原文同一筆交易（`action.returned_dismiss { draft }`）：合併後超過 2000 字存不進去，原文照留並提示先刪一些〔模型判斷·未裁決；Sol 驗收第 2 輪、常識審查員確認〕。
- GM 旁白、請發言先開輸入框帶方向提示：單人桌面版沒有（`gm_narrate` 不收文字、按鈕不讀輸入框），照搬原則本案不做，留給之後的房間版面整理案一起考慮〔模型判斷·未裁決〕。
