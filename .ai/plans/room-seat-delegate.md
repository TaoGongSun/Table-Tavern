# room-seat-delegate — 房間階段三：AI 代管座位、到場整理、手機房主暫停

**狀態：施工中（2026-11-04 起，包 1、包 2 先做；包 3 等 room-host-actions 包 1 定型 HostPanel）。** Fable 常識審、Sol 方案審 2 輪都已併入；第 2 輪的 1 條必改由主線裁定照 Sol 的修法。

上層：[multiplayer-room.md](multiplayer-room.md)、協定 [multiplayer-room-protocol.md](multiplayer-room-protocol.md)、階段一 [room-host-desktop-play.md](room-host-desktop-play.md)（§3 末「階段三現在要預留的」是本案起點）。前面兩案：`room-player-cards`（階段二，施工中）、`room-host-actions`（已定案）；兩份方案都在各自分支。

**案別：大案**，核心估約 370 行（上限 740，§6）。原估中案 260，Fable 常識審後重估為 350，Sol 方案審第 1 輪後再重估為 370。方案與驗收各送一位審查；本案本來就碰到回合狀態機，也碰到一般玩家正常操作會遇到的資料遺失（代管時已送出的行動要退回草稿，§3.1），不論案別都要做方案審。協定增補先寫在本檔 §3，施工時再併進協定文件；本分支不動協定檔。

## 1 作者原話與原則

- 「可以重開。桌上多一個『重開房間』，接回同一個房間碼、同一批朋友和劇情繼續。但有可能不是所有人都能到場，所以要有玩家給AI託管或是移除指定玩家的地方。也要有能加入新玩家的地方。……也要有中途離開可以暫時給AI代管這種功能。這些可以在後續再補，但必須要考慮進去。」〔作者裁決 2026-11-03，出自 room-host-desktop-play §1〕
- 原則：房間＝現在桌面版的功能照搬成連線版。常識有答案的直接定，不出選項。〔作者裁決 2026-11-03〕
- 手機房主遠端暫停放階段三：執行端收到後走關房流程。〔作者裁決 2026-11-03，room-host-desktop-play §2.1〕
- 倒數到期目前把草稿當送出，跟房主手動結算（觀望、草稿保留）相反；之後改成到期也比照手動結算。〔作者裁決 2026-11-03，room-host-desktop-play §2.1〕
- 手機房主改世界書：「可以之後再考慮追加」〔作者裁決 2026-11-03〕。

## 2 成品樣子

本節與 §3–§5 的產品取捨，已經過 Fable 常識審查（2026-11-04），照審查結論寫，全部標〔模型判斷·未裁決〕；只有 W1 還要作者決定（§9）。

### 2.1 AI 代管（座位保留、人回來接回）〔模型判斷·未裁決〕

- **由誰演**：GM 把那張卡當 NPC 演，照劇情需要出場，不會每回合固定替他行動（§9 Q1）。路徑跟目前「離席→當 NPC」相同：GM 可以點名、角色線可以接話。差別是這次是暫時的，座位、卡的認領、他的草稿都保留。
- **玩家自己**：入座後，名單頁「暫時離開」旁多一顆「交給 AI 代管」。
  - 按下後，手機上方常駐一條「你的角色由 GM 代演中（照劇情需要出場）」，旁邊放「接回」鈕。
  - 常駐列寫清楚接回何時生效：「現在這回合沒人在等就立刻加入，否則下一回合起」。
- **代管中的人按送出**：行動格的送出鈕停用，改寫「代管中，接回後才能行動」。草稿照存，不會讓他收到錯誤。
- **重新連上不自動接回**（§9 Q5）：回來後看到的是常駐列和「接回」鈕。
- **「暫時離開」的確認**（§9 Q4）：
  - 只有「已入座、房間 playing」時才跳確認框，給兩顆鈕：「交給 AI 代管再離開」「直接離開（座位保留）」。
  - 在大廳或沒入座時，照舊一按就走。
- **房主**：可以替任何入座的人開或關代管，對象不限斷線的人（§9 Q6）。入口有兩處：
  - 房主面板的到場整理區（§2.2）。
  - 故事頁「還在等：A、B」那一列：離線的人旁邊直接放「交給 AI 代管」，房主不必切到房主分頁。
- **斷線不自動代管**（§9 Q2）：斷線照現在的做法處理（跳過、房主結算、倒數），代管要由房主或本人按。
- **代管時已送出的行動**：退回他的草稿，不計入這回合。下列三處，只要對象本回合已經送出，都要寫明「你這回合已送出的行動會退回草稿、不計入」：
  - 本人的「交給 AI 代管」鈕。
  - Q4 確認框裡的「交給 AI 代管再離開」。
  - 房主端的代管鈕。
- **行動名單**：代管中的人不進行動名單，大家不必等他，「還在等：A、B」也不列他。
- **故事串**（§9 Q3）：
  - 開始代管時多一行「（玩家暫離）〈卡名〉暫由 GM 代演，照角色原本個性行動，不替玩家做重大決定」。
  - 接回時多一行「（玩家回來）〈卡名〉」。
- **全員都在代管**：行動名單是空的，照現有規則不結算，房間停住，不會無人值守一直花額度。回合列寫明原因：「全員代管中，回合不會自動進行：用 GM 推進，或取消某人的代管」。GM 推進鈕來自 room-host-actions。
- **重開房間**：代管狀態保留。

### 2.2 到場整理（房主面板）〔模型判斷·未裁決〕

房主面板的「成員」區改成「到場整理」。每位入座的人一列，依序是：
- 連線狀態點、暱稱、卡名、「AI 代管中」標籤。
- 「交給 AI 代管／取消代管」。
- 「移除」：確認框直接選角色怎麼處理，「交給 GM 當 NPC」或「角色離席」，預設離席（同現行 `absent` 預設）。
- 既有的轉讓房主、封鎖、指派座位照舊。

沒入座的成員另列一段，按鈕照現有。

- **一鍵代管**：「把沒連線的人都交給 AI 代管」，只在有入座而且斷線的人時出現。
- **重開後的提醒**：執行端從暫停接回後，房主回合列上方出一條「還沒連上：A、B」，旁邊放「全部交給 AI 代管」和「去整理」（切到房主分頁）。所有入座的人都連上，或房主按過其中一顆，提醒就收起。
- **加入新玩家**：邀請連結本來一直有效，中途入座照「中途加入」登場，這部分已經做好。本案補兩件：
  - 到場整理區頂端放「邀請新朋友」，**顯示現行連結＋QR，不重生**。重生會讓還沒到場的老朋友手上的舊連結作廢。
    - 桌面房主：直接用資訊列那條。
    - 手機房主：怎麼拿到現行連結見 §3.6。
  - 「換新連結」另放一顆鈕，寫明「舊連結會作廢，還沒進來的人要重新拿」；做法就是現有的 `invite.regenerate`。
- 新朋友的卡照階段一自建角色或階段二匯入。

### 2.3 手機房主暫停〔模型判斷·未裁決〕

- 手機房主的房主面板多「暫停房間」。按下先確認：「所有玩家會看到房主暫時關房。要繼續，得在 GM 主機（<executor.label>）上按『重開房間』。」
- 確認後，GM 主機（電腦）走跟桌面「關房（暫停）」完全相同的流程：
  - 手機看到「房主暫時關房」。
  - 正在生成的那段中止，半截保留。
  - 執行端下線，桌留在列表。
- 電腦端回到桌面，跳出通知「房主在手機上暫停了房間」；桌卡上的「重開房間」照常可用。
- 執行端不在線時，這顆鈕停用，旁邊寫「電腦已離線，房間已經停著」。

### 2.4 倒數到期比照手動結算〔模型判斷·未裁決〕

- 倒數到期時，沒送出的人一律算「觀望」，草稿留著，下一回合原樣帶著；不再把草稿當送出。到期跟房主手動結算只差在觀望原因（`timeout`／`host_resolved`）。
- 倒數列在倒數期間常駐「沒送出就算觀望，草稿會留著」；到期後的回合提示寫同一句。

## 3 協定增補〔模型判斷·未裁決，送方案審〕

### 3.1 代管 `seat.delegate`

客戶端 → 伺服器：

| t | p | 誰 | 合法狀態 | 伺服器處理 |
|---|---|---|---|---|
| `seat.delegate` | `{ memberId, on: boolean }` | 本人（`memberId` 是自己）或 H | 非 closed；對象有座位；狀態跟現值不同，否則 `E_STATE_CONFLICT` | 見下；發 `seat.delegated { memberId, cardId, on, by }` |

**`on: true`**（Sol 第 1 輪必改 1、2：先把暫離記下，最後才重判結算，退回的原文要即時回到手機）：

依序做下列幾步，**中途都不呼叫 `settle`**。重判結算交給交易尾端統一處理（`reduce.ts` 每則輸入結尾本來就會跑 `settle`），這樣派工一定帶得到這次暫離。

1. 當前是 `acting`，而且他在 roster 裡：
   - 已 `submitted` 或 `skipped`：先照 `action.retract` 的做法退回，原文寫回 `drafts[m.id]`，發既有的 `action.retracted { round, memberId }`。客戶端 reducer 收到這則，就會把 roster 上的原文放回本人的輸入框（`room-state.ts` 已有）。在線的手機不用重連就看得到。
   - 再把他從 roster 移除。
2. 設 `m.delegated = true`。
3. 他**還沒登場**（`arriving` 為 true）時：`arriving` 保持原樣，`departures` 不推任何項。執行端從沒把他當在場玩家，所以不寫暫離。代管期間 `activeMembers` 不列他，`openRound` 不會消耗他的 `arriving`。
4. 否則，`departures` 推入 `{ cardId, mode: "delegate" }`，同一張卡已有的舊項換掉。
5. 發 `seat.delegated { memberId, cardId, on: true, by }`。客戶端收到後把他從當前 roster 拿掉。

**`on: false`**，依序判斷，命中一種就停：

1. **還沒登場**（`arriving` 為 true）：只清 `delegated`，不記 `returning`。下一次開 `acting` 照一般中途加入登場。
2. **抵消**：`departures` 裡還有這張卡沒派出的 `delegate` 項（開了又關，中間沒派工），表示執行端從沒收過暫離。
   - 清 `delegated`，拿掉那筆 `delegate` 項，`returning` 不記，`arriving` 不設。
   - 當前是 `acting`：直接把他放回當前 roster，狀態照有沒有草稿定 `drafting`／`pending`，不進 `arrivals`。
   - 不是 `acting`：什麼都不用補，下一次開 `acting` 會照一般入座者把他排進 roster。
3. **一般接回**：清 `delegated`，卡記進 `s.returning`，之後照 `claim()` 尾段的「中途加入」：
   - 當前 `acting` 而且 roster 是空的：直接加進當前 roster，並推進 `r.arrivals`。
   - 其他情況：設 `arriving = true`，下一次開 `acting` 才進 roster。

每種情況最後都發 `seat.delegated { …, on: false }`，結算一樣交給交易尾端。

協定是 `memberId` 不是階段一預留的 `cardId`：座位綁在成員上，`claimedBy` 可以反查卡；用 `memberId` 時，本人與房主走同一套驗證，也跟 `member.kick`／`member.reassign` 一致。

**排除代管座位**：`round.ts` 加 `activeMembers(s)`，就是 `seatedMembers(s)` 再濾掉 `delegated`。下列地方都改用它，一處定義：
- `openRound` 組 roster。
- `ensureDispatch` 組 `players`。
- `room.start` 的「至少一位玩家」。
- `round.resolve` 已經檢查 roster 不為空，不用動。

**派工**：
- `Dispatch.departures[].mode` 多一種 `"delegate"`。
- `Dispatch` 加 `returns: string[]`：本回合從代管回來的卡，一定是 `arrivals` 的子集。
- 伺服器在 `ensureDispatch` 帶出 `returns = returning ∩ arrivals`，**只從 `s.returning` 移除這幾張**，其餘留著（Sol 第 1 輪必改 4）。
  - 例：房主按 GM 推進開的回合，`arrivals` 是空的，這時 `returning` 整份保留，等下一個帶到他登場的派工。

**其餘規則**：
- 卡的 `claimedBy` 不動：別人選不走，名冊照舊顯示他坐這張卡。
- 草稿照舊只回給本人，代管中也能打字，只是送不出（不在 roster 裡就 `E_STATE_CONFLICT`，同現行）。
- **離座與轉移座位一律清 `delegated`**（Sol 第 1 輪必改 3）：
  - `unseat`（離座、被踢、名冊移除）把舊成員的 `delegated` 清成 false。不清的話，他之後再選卡入座，會一坐下就是代管中。
  - `member.reassign`：來源與接手者兩邊都清。
  - 代管中的座位被接手時，有人接手就是人在玩。座位上如果還有沒派出的 `delegate` 項，照「抵消」處理；已經派出的，照一般接回記 `returning`，並設 `arriving`。
- **`seat.release`、`member.kick` 對代管中的人**：照現有 `unseat`，但要改一處。現在 `unseat` 用「`departures` 裡已有同一張卡」防重複；如果那筆是還沒派出的 `delegate`，就不會推 `absent`，結果卡以 `delegate` 派出去，變成永久 NPC 卻寫著「暫離」。所以 `unseat` 遇到同卡還在等的 `delegate` 項，要改寫成 `absent`（`member.kick` 帶 `mode` 時就改寫成那個 mode），同時清掉 `returning` 裡這張卡。`delegate` 已經派出的情況（`departures` 已清空）照舊推一筆 `absent`，房主可以再用 `member.departure` 改成 `npc`。
- **快照**：`members[]` 每位多 `delegated: boolean`，`me` 也多 `delegated`。舊房間沒有這欄就當 false。
- **事件**：`seat.delegated` 是持久事件；客戶端 reducer、快照、匯出不用特別處理，它不是劇情事件。
- **跟 room-host-actions 的交會**：
  - `round.undo` 的 `edit` 模式會把原派工的 `arrivals`／`departures` 放回待帶清單；`returns` 一併放回 `s.returning`，`delegate` 模式的離席項照樣放回。只多一個欄位，不另做。
  - **「重生這段」（room-host-actions §3.2 `regenerate`）沿用原派工快照**（Sol 第 1 輪必改 5）：`players`、`arrivals`、`departures`、`returns` 全照原派工，不重算。
    - 收回到重生之間發生的座位變動（代管開關、離座、入座），照常記在 `departures`／`returning`／`arriving`，留到下一個正常派工才套用。
    - 原因：執行端已把原派工那批事件收掉，重生時用同一份名單重跑才對得上；如果改用當下名單，`party_batch` 會發現行動者不在 `players` 裡而整批拒收。
    - 施工時把這條寫進 room-host-actions 的 `lastDispatch` 用法：重生派工從 `lastDispatch` 拷貝這四欄，不呼叫 `activeMembers`。
- **施工備註**（2026-11-04 開工時，host-actions 還沒合併）：
  - `round.undo` 的 `edit` 模式把 `returns` 放回 `s.returning`，還有重生沿用快照，都要等 rebase 到 room-host-actions 之後才補。
  - `invite.current`（§3.6）要重用 room-player-cards 的 `live.cardReqs`，那案還沒進 main；等 rebase 到那案之後，再放進包 1 的補做。
  - 那案的同步閘門（`execReady`）擋的是結算與派工，跟代管無關：閘門期間照樣能開或關代管。

### 3.2 移除時一起決定角色處置

- `member.kick` 多一個可選的 `mode: "npc" | "absent"`。
- 有帶時，在同一筆交易裡做 `unseat`，再照 `member.departure` 寫入處置；不帶就照現狀（預設 `absent`，之後再改）。
- 原因：房主面板的「移除」確認框一次選完，不必送兩則、也不會卡在中間狀態。

### 3.3 手機房主暫停：遠端控制 `ctl { kind: "pause" }`〔模型判斷·未裁決〕

改走既有的遠端控制（協定 §7.3），不另開 `job.pause`。這是常識審查員複核後的建議，主線採用（2026-11-04）。原本的 `job.pause` 做法沒錯，但既有 `ctls` 機制已經具備暫停要的每一項性質，而且都測過：
- 一次性指令，記下 `conn`／`id`／`at`。
- 執行端先回 `ctl.result` 再動手。
- 30 秒逾時，由 `onAlarm` 回 `E_EXECUTOR_TIMEOUT`。
- 不進 `w.jobs`，`welcome` 不會重送，所以重開房間時不會又被暫停。

**協定**：`RemoteCommand` 加 `{ kind: "pause" }`。§7.3 加一句：「遠端控制的 `pause` 由房主下，其餘仍限出額度者。」

**伺服器**（`member.ts::ctl`）：
- 權限：`pause` 要房主，其餘照舊只限出額度者。
- 驗證沿用既有規則：執行端在線、`ctlId` 不重複。另加一條：已經有 `pause` 在等時，回 `E_STATE_CONFLICT`。
- `ctl.result` 照既有路徑轉回發令者。
- `setExecOnline(false)` 遇到還在等的 `pause`：`exec.paused` 已是 true 就回 ok（關房流程送完 `status` 才斷線），否則回 `E_EXECUTOR_OFFLINE`，並清掉那筆。

**執行端**（`run.rs::on_ctl` 加 `pause` 分支）：
1. 回 `ctl.result { ok: true }`。
2. `note_stop(world, "ROOM_PAUSED_REMOTELY")`。
3. 另開一個 task 呼叫 `room::exit_world(world)`：房間還開在這張桌才走 `exit()`，也就是跟桌面「關房（暫停）」同一條流程：
   - 送 `status { paused: true }`。
   - 中止生成與工作。
   - 斷線，桌子還原，`Saved.active = false`。

   `exit()` 要停掉連線迴圈本身，所以不能在迴圈裡等它。

**桌面前端**：
- `useDesktopRoom` 每 2 秒輪詢狀態。看過房間開著之後，讀到 `!active` 而且停止原因是 `ROOM_PAUSED_REMOTELY`，就跳通知「房主在手機上暫停了房間……」，再照「關房（暫停）」回大廳。
- 之後再進這張桌，讀到的是舊的停止原因，不再跳通知。
- 資訊列的最近錯誤也把這個原因顯示成同一句。

**手機**（包 3）：暫停鈕按下後立即停用，避免連按看到錯誤。

**跟 room-host-actions 同步閘門的相容**：那案的閘門看 `exec.paused` 和 `Saved.paused`。遠端暫停走同一支 `exit()`、送同一則 `status`，所以重開時閘門照樣打開。

**施工備註**：rebase 到 room-host-actions 之後，補一條測試「遠端暫停後重開，同步閘門照常打開」。

錯誤碼不新增。

### 3.4 倒數到期（協定 §3.2 第 3 條改寫）

- `round.ts` 的 `resolve`：`by === "timeout"` 時不再取 `drafts` 當送出，沒送出的人一律記 `watch`，原因 `timeout`，草稿照舊留著。
- 協定 §3.2 第 3 條改成「倒數到期：沒送出的人算觀望（原因 timeout），草稿留給下一回合」。
- `round.test.ts` 裡斷言「到期草稿當送出」的那條跟著改。

### 3.5 新欄位一覽

- 成員指令：`seat.delegate { memberId, on }`、`invite.current { candidate? }`；`member.kick` 多 `mode?`；`ctl` 的 `cmd` 多 `{ kind: "pause" }`（房主下）。
- 持久事件：`seat.delegated { memberId, cardId, on, by }`。
- `Member.delegated`、`RoomState.returning: string[]`。
- 快照 `members[].delegated`、`me.delegated`。
- `Dispatch.departures[].mode` 多 `"delegate"`，`Dispatch.returns: string[]`。
- 伺服器 → 執行端 `invite.get { reqId }`；執行端 → 伺服器 `invite.got { reqId, joinSecret }`。

### 3.6 手機房主拿現行邀請連結 `invite.current`

Fable 的前提是「手機房主手上就有現行密語」，這只對一半：
- 手機的網址只拿掉配對碼，`#k=` 會留著（`invite.ts` 的 `withoutPairCode`）。
- 但那是他**加入那時**的密語。之後電腦或別的房主按過「換新連結」，它就過期了。
- 手機若是從沒帶 `k` 的網址回來（書籤、只輸入配對碼），手上根本沒有密語。
- 伺服器只存雜湊，不存明文。

所以補一則指令，執行端那邊用同一套 `invite` 轉手：

| t | p | 誰 | 合法狀態 | 伺服器處理 |
|---|---|---|---|---|
| `invite.current` | `{ candidate?: string }` | H | 非 closed | 見下 |

1. 帶了 `candidate`，而且它的雜湊等於現行 `joinHash`：直接回 `ok { joinSecret: candidate }`，不必經電腦。
2. 否則，執行端在線時轉給執行端，**重用階段二 room-player-cards 的請求轉送機制**：記在 `live.cardReqs`，新增 `kind: "invite"`。
   - 伺服器送 `invite.get { reqId }`，執行端從 `Saved` 回 `invite.got { reqId, joinSecret }`。
   - 30 秒逾時、執行端斷線即失敗、DO 被逐出就一起消失，都照那套既有規則。
   - **回覆前重驗**：發問者現在仍是房主、那條連線仍屬同一位成員、`joinSecret` 的雜湊等於 `joinHash`。任一不成立就丟掉內容、回 `E_STATE_CONFLICT`。
   - 只回給發問的那條連線，不落地、不廣播。
3. 執行端不在線、手上也沒有能用的密語：回 `E_EXECUTOR_OFFLINE`。畫面寫「電腦離線，拿不到現行連結」，仍可按「換新連結」（換新同樣要電腦在線，現有規則）。

**不記錄**：`candidate` 和 `joinSecret` 不進 `RoomState`、事件、快照、匯出，也不寫日誌；伺服器、執行端兩邊都一樣。

- 手機打開「邀請新朋友」時送這一則，`candidate` 帶自己網址上的 `k`。
- 桌面房主不用這則，資訊列本來就有連結。
- 執行端這邊在 `link.rs` 分派加 `invite.get`，讀 `Saved` 的 `join_secret` 回覆。

## 4 執行端（Rust）

- **代管本身幾乎不用改**：`append_party_batch` 會用 `set_party` 把 `npc` 名單的卡移出 `player_card_ids` 和 `absent_card_ids`。之後它就是一般角色卡，`chat_assembly::Party::holds` 不擋，GM 可以點名、角色線可以接話。
  - `delegate` 模式在 `set_party` 跟 `npc` 走同一份清單。
  - 差別只在標記：寫 `PlayerDelegated`，不寫 `PlayerToNpc`。
- **接回**：卡回到 `players` 名單，照現有路徑登場。在 `returns` 裡的寫 `PlayerReturned`，取代 `PlayerArrival`。
- **新標記**：`EventMarker` 加 `PlayerDelegated { name }`、`PlayerReturned { name }`，`marker.rs` 接模板，`src/shared/contracts/transcript-marker.json` 補十語。
  - 標記本身就是 GM 讀得到的提示，文案帶「照角色原本個性行動，不替玩家做重大或無法挽回的決定」（桌上跑團代演缺席角色的慣例）。這句寫在標記文案裡，不改提示詞組裝。
  - 文案照 §2.1（§9 Q3）。
- `run.rs::land_batch` 解析 `mode == "delegate"` 和 `returns`。
- 遠端暫停（`ctl pause`）見 §3.3，`invite.get` 見 §3.6。

## 5 手機與桌面前端（party 元件，桌面房間畫面共用）

- `room-state.ts` 套 `seat.delegated`、快照 `delegated`；`useRoom` 收 `ctl pause`、`invite.current` 的回覆。
- `RoomScreen`：
  - 代管中的人：上方常駐列（§2.1 兩句文案）＋「接回」。
  - 名單頁多「交給 AI 代管」。
  - 「暫時離開」只在已入座、房間 playing 時跳兩鈕確認框，其他情況照舊一按就走。
  - `MemberList` 每列顯示「AI 代管中」標籤。
  - 對象本回合已送出時，代管鈕與確認框寫「你這回合已送出的行動會退回草稿、不計入」。
- `ActionBox`：代管中時送出鈕停用，改寫「代管中，接回後才能行動」，草稿照存。
- `HostPanel`：
  - 「到場整理」區（§2.2）：
    - 每列的代管／取消，對象已送出時附同一句提醒。
    - 移除，確認框選處置。
    - 一鍵代管。
    - 邀請新朋友：顯示現行連結＋QR，手機走 `invite.current`。
    - 另一顆「換新連結」，附作廢提醒。
  - 「暫停房間」：只有手機房主看得到，桌面版已有「關房（暫停）」。電腦離線時停用，寫「電腦已離線，房間已經停著」。
- `RoundBar`：
  - 「還在等：A、B」裡，離線的人旁邊放「交給 AI 代管」，只有房主看得到。
  - 全員代管時寫停住的原因（§2.1）。
  - 倒數期間常駐「沒送出就算觀望，草稿會留著」。
  - 重開後的「還沒連上」提醒。
  - 「剛從暫停接回」靠 `vol.executor` 的 `paused` 從 true 變 false 判斷，客戶端記在記憶體。
  - 頁面重新整理後就不再提醒；到場整理區照樣看得到誰沒到。
- 桌面版 `useDesktopRoom`／`DesktopRoom`：遠端暫停的通知（§3.3）。
- i18n：party 十語＋桌面版通知十語。

## 6 分包與估計

| 包 | 內容 | 核心估 |
|---|---|---|
| 1 伺服器 | `seat.delegate`（含先退回再記暫離、結算交給尾端、`arriving` 保留、抵消、`returning ∩ arrivals`）、離座與轉移清 `delegated`、`activeMembers` 取代三處、`returns`／`delegate` 模式進派工、`reassign` 清代管、`unseat` 改寫未派出的代管項、`member.kick mode`、`ctl pause`（房主權限、同時一個、下線時照 `exec.paused` 回覆）、`invite.current`（候選驗雜湊、轉執行端、只回房主、逾時）、倒數到期改觀望、快照欄位、驗證；重生沿用原派工快照（寫在 room-host-actions 的 `lastDispatch` 那處） | 130 |
| 2 執行端 | `land_batch` 解析、`party_batch` 兩個新標記分流、`EventMarker`＋模板、`on_ctl` 的 `pause` 分支＋另開 task 呼叫 `exit_world`＋`note_stop`、`invite.get` 回覆、桌面前端遠端暫停通知 | 55 |
| 3 手機／房間元件 | reducer、代管常駐列與接回時機文案、`ActionBox` 代管中停用送出、名單頁代管鈕、條件式離開確認框、已送出退草稿提醒（三處）、到場整理區（代管／移除選處置／一鍵代管／現行連結＋QR＋換新連結）、回合列（離線者代管鈕、全員代管原因、倒數觀望說明、重開提醒）、手機暫停鈕＋確認＋離線停用說明 | 165 |

**合計約 350**（上限 740，沿用前一版的上限）。Sol 第 1 輪必改讓伺服器那包多約 20 行：還沒登場的分支、`returning` 部分消耗、清 `delegated` 兩處、重生沿用快照。暫停改走 `ctl` 後少約 20 行：拿掉 `s.pausing`、`job.*` 比對分流、`doneJobs` 處理。Fable 常識審把原本的 260 推過 300：故事頁的代管入口、各種狀態說明文案、`invite.current` 這一則新指令。**案別因此從中案升為大案**，方案與驗收照樣各送一位審查，跟原本要做的一樣。

- 包 1 先做，包 2、包 3 照本檔的協定可以並行。
- 測試上限是核心的 2 倍（約 740）。故障注入只用在一條資料遺失路徑：代管時已送出的行動要回到草稿。
- 動到的檔案多在 `party/server/core/`（member.ts 353 行）、`party/src/features/controls/HostPanel.tsx`、`room/run.rs`（791 行）。都不會超過 1000 行；到場整理如果讓 HostPanel 變得太長，就拆成 `controls/Attendance.tsx`。

## 7 驗收

- **自動**：每包都跑 `npm run verify`、party 的 test＋typecheck、`cargo test`。
  - 伺服器：
    - 代管開關：本人、房主、非房主替別人（回 `E_FORBIDDEN`）、狀態沒變（回 `E_STATE_CONFLICT`）。
    - 已送出後代管：先發 `action.retracted` 再發 `seat.delegated`，原文回到草稿，批次裡沒有他。
    - 代管後 roster 只剩已送出的人：立刻結算，而且這次派工的 `departures` 帶著他的 `delegate` 項（必改 1）。
    - 還沒登場就代管、再取消：`arriving` 保留，下一回合照一般登場；期間派工不帶他的暫離項（必改 4）。
    - `returning` 只消耗 `returning ∩ arrivals`：GM 推進開的回合（`arrivals` 為空）不清掉 `returning`。
    - 代管中的人離座或被踢、再選卡入座：不是代管中；`reassign` 後來源與接手者都不是代管中（必改 3）。
    - 重生沿用原派工的 `players`、`arrivals`、`departures`、`returns`；收回後才代管的人，下一個正常派工才帶上（必改 5）。
    - 全員代管時不結算、倒數暫停。
    - 接回：空 roster 時直接加入當前 roster；非空時下一回合登場；派工 `returns` 帶上這張卡。
    - 開了又關、還沒派工：抵消，派工兩邊都沒有；當時是 `acting` 的話，他直接回到當前 roster，不進 `arrivals`。
    - 代管項還沒派出就被踢或自己離座：派出的是 `absent`（或 kick 帶的 mode），不是 `delegate`。
    - `reassign` 清掉代管、`kick` 帶 `mode` 時處置一筆到位。
    - `ctl pause`：只有房主能下（出額度者不行）、同時只能一個；`ctl.result` 回房主；等回覆時執行端下線，已報暫停回 ok、沒報回 `E_EXECUTOR_OFFLINE`；30 秒沒回 `E_EXECUTOR_TIMEOUT`。
    - 倒數到期：草稿留著、記觀望。
    - `invite.current`：候選正確時不經執行端；候選過期時轉執行端，回來的密語雜湊不符就不回；回覆前房主已轉讓就回 `E_STATE_CONFLICT`；執行端離線回 `E_EXECUTOR_OFFLINE`；回覆只送發問的那條連線；`state`、事件、快照、匯出全文搜尋不到密語。
    - 代管中的人送出：前端停用送出鈕；伺服器照舊擋（測一條）。
  - Rust：
    - `delegate` 模式落 `PlayerDelegated`，卡進 NPC 名單。
    - `returns` 落 `PlayerReturned`，不落 `PlayerArrival`。
    - `ctl pause` 先回結果、記下停止原因 `ROOM_PAUSED_REMOTELY`。
- **測試通道實測**：本機 Node 殼＋測試通道桌面版，瀏覽器當兩支手機 A、B。
  1. A 送出行動後按「交給 AI 代管」：A 的手機**不用重連**，原文就回到輸入框；回合不等 A；GM 演到 A 的角色時語氣一致；故事串有暫離那一行。
  2. A 按「接回」：下一回合 A 在行動名單裡，故事串有回來那一行。
  3. B 關掉頁面。房主在到場整理按「把沒連線的人都交給 AI 代管」，回合照常往下走。
  4. 房主移除 B，選「交給 GM 當 NPC」：B 的卡變成 NPC，B 再用原連結進來是無座成員。
  5. 電腦「交給手機」後，手機房主按「暫停房間」：手機顯示房主暫時關房，電腦跳出通知並回到桌面。電腦按「重開房間」後，不會又被暫停；房主看到「還沒連上」提醒。
  6. 倒數設 60 秒、A 打了草稿不送：倒數列看得到「沒送出就算觀望」；到期後 A 記觀望，草稿還在。
  7. 手機房主按「邀請新朋友」：顯示的連結跟電腦資訊列同一條，老朋友的舊連結照樣能進。電腦按過「換新連結」後，手機再按一次，拿到的是新連結。
  8. B 斷線，房主在故事頁「還在等」那一列直接把 B 交給 AI 代管。全員代管時，回合列寫明停住的原因。
  9. A 代管中：送出鈕停用、草稿還在；A 在大廳按「暫時離開」不跳確認框，在 playing 中跳出兩鈕確認框。

## 8 已知限制〔模型判斷·未裁決〕

- 代管只是「GM 可以演這張卡」，GM 不會每回合固定替他行動；GM 沒點到，他那回合就沒戲份（§9 Q1）。常駐列已寫「照劇情需要出場」。
- 手機房主在電腦離線、手上也沒有現行密語時，拿不到邀請連結（§3.6）。
- 等暫停回覆時房間被關（`room.close`），手機那則請求不會有回覆；這跟既有的 `ctls`／`inviteRegen` 是同一個洞。
- 舊版桌面 app 收到 `delegate` 模式會當成離席（`absent`）處理，所以要先部署伺服器、再發新版 app，照既有部署順序。
- 手機房主只能暫停，不能重開：重開一定要在 GM 主機上按（執行端在那台電腦上）。
- 「剛從暫停接回」的提醒只靠客戶端記憶體，頁面重新整理後就不再提醒；到場整理區照樣看得到誰沒到。
- 全員代管時房間停住，要房主處理。這是刻意的，避免無人值守一直花額度。

## 9 待裁決

**需作者**

- **W1 手機房主改世界書，併進本案還是另立？**
  - 建議：另立案，不併。
  - 理由：作者說的是「之後再考慮追加」，屬產品方向。做法是一整套世界書讀寫通道，規模接近 room-player-cards §3.2 的讀卡與存卡，還要加手機上的世界書編輯畫面，跟座位無關。世界書含 GM 專屬內容，也會經過伺服器記憶體。
  - 選另立：本案估計不變，主線開一條 BACKLOG（新待辦不由本案子代理立案）。
  - 選併：本案再多約 300 行，要再補一包方案。

**已由 Fable 常識審查定案（2026-11-04）**：全部照建議，標〔模型判斷·未裁決〕。

- **Q1 AI 代管由誰演**：GM 當 NPC 演，照劇情需要出場。不做每回合另打一發 AI 代寫行動，因為那樣每位被代管的人每回合多一次模型呼叫，核心也要多約 120 行。常駐列寫「你的角色由 GM 代演中（照劇情需要出場）」。〔模型判斷·未裁決〕
- **Q2 斷線不自動代管**：由房主（到場整理、故事頁「還在等」那一列）或本人按。〔模型判斷·未裁決〕
- **Q3 新開兩個標記**：「玩家暫離（照原本個性行動、不替玩家做重大決定）」和「玩家回來」。〔模型判斷·未裁決〕
- **Q4 「暫時離開」確認框**：兩顆鈕，「交給 AI 代管再離開」「直接離開（座位保留）」。只在已入座、房間 playing 時跳；大廳或沒入座時一按就走。〔模型判斷·未裁決〕
- **Q5 重連不自動接回**：常駐列加「接回」鈕，寫明「現在這回合沒人在等就立刻加入，否則下一回合起」。〔模型判斷·未裁決〕
- **Q6 房主能替別人開和關代管**。〔模型判斷·未裁決〕
- **Q7 倒數到期改觀望併進本案**。〔模型判斷·未裁決〕
- **Q8 未遊玩單人桌「用這張桌開房」另立小案**（估 30–50 行），由主線開 BACKLOG。〔模型判斷·未裁決〕

**Fable 常識修正**：已寫進 §2、§3.6、§5，標〔模型判斷·未裁決〕。

1. 邀請新朋友顯示現行連結，不重生；「換新連結」另放一顆並附作廢提醒。手機房主未必握有現行密語，補 `invite.current`（§3.6）。
2. 代管中的人：送出鈕停用、改寫說明，草稿照存。
3. 故事頁「還在等」那一列，離線的人旁邊直接有「交給 AI 代管」。
4. 全員代管時寫明房間停住的原因。
5. 倒數列寫「沒送出就算觀望，草稿會留著」。
6. 手機暫停鈕在電腦離線時停用，寫「電腦已離線，房間已經停著」。
7. 對象本回合已送出時，代管鈕與確認框寫明「你這回合已送出的行動會退回草稿、不計入」。
8. 接回的常駐列寫明何時生效。

## 10 觀察

（施工時補）
