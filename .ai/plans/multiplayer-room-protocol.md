# multiplayer-room — 房間協定 v1（包 0）

整體計畫見 [multiplayer-room.md](multiplayer-room.md)，立案與作者裁決見 [handoffs/multiplayer-room.md](../handoffs/multiplayer-room.md)。本文件是包 1（伺服器）、包 2（桌面版執行端）、包 4（手機客戶端 `party/`）共同的實作規格。標〔作者裁決 2026-11-02〕的照裁決；其餘標〔模型判斷·未裁決〕，同一節開頭標過的，該節各條不再重複標。主線裁定與已知限制在 §13。

## 0 範圍與用語

- **房間**：一個 Durable Object（DO）實例，以房間碼命名。一房一個 DO〔作者裁決 2026-11-02：伺服器選丙，Cloudflare Workers＋DO 先做，核心純 TS〕。
- **成員**：用手機（或瀏覽器）連進房間的人。**玩家**＝已選角（握有一張真人玩家卡）的成員。
- **執行端**：替房間打 GM 那一發的程式。v1 只有桌面版，**固定為建房那一台，整個房間不換**〔作者裁決 2026-11-02，裁決 7、8〕；協定把它當一種「角色」，v2 的伺服器內建執行端扮同一個角色（§11）。
- **出額度者**：與執行端配對過的那一位成員，GM 用的是他的額度；只有他能下遠端控制指令。每房最多一位。
- **房主**：管房間的成員（結算、踢人、換幕、關房）。預設＝出額度者，可轉讓。
- **幕（scene）**：房間層的幕號，從 1 起算；與桌面版桌內的 `current_scene` 不同步，執行端自己對照。
- **回合（round）**：房間層的回合號，整房單調遞增、換幕不歸零；兼作版本號（§3.3）。
- **伺服器不持有任何 AI 金鑰、任何平台帳號**；金鑰從未離開出額度者的桌面版（任務檔 Constraints、計畫 §8）。

## 1 角色與身分憑證〔模型判斷·未裁決〕

### 1.1 角色

| 角色 | 綁在什麼上 | 能做什麼 | 數量 |
|---|---|---|---|
| 成員 | 成員憑證 | 看劇情、改暱稱、選角 | 每房上限 12 |
| 玩家 | 成員＋一張已認領的卡 | 上述＋行動格送出／跳過／收回 | 上限＝房間設定 `maxPlayers`（2–8，預設 6） |
| 房主 | 某位成員 | 結算、房間設定、重生邀請、踢人、離席處置、換人裝置、換幕、關房、匯出、中止生成 | 1 |
| 出額度者 | 某位成員×執行端 | 遠端控制（§7.3）、中止生成、設定「下一幕設定」 | 0 或 1 |
| 執行端 | 執行端憑證 | 接派工、回串流與結果、回報狀態與額度、發配對碼 | 1（建房那台） |

一位成員可同時是房主、玩家、出額度者。執行端不是成員、不佔座位。

### 1.2 憑證

| 憑證 | 誰產生 | 長相 | 存在哪 | 伺服器存什麼 | 活多久 | 怎麼撤銷 |
|---|---|---|---|---|---|---|
| 裝置識別碼 `deviceId` | 客戶端（WebCrypto 128 位元亂數） | base64url 22 字 | 客戶端 localStorage，跨房間共用一份 | `SHA-256(roomCode + ":" + deviceId)` | 到房間刪除或被踢 | 房主踢人；換裝置＝新成員，由房主 `member.reassign` 把座位移過去 |
| 房間碼 `roomCode` | 伺服器 | Crockford base32 8 字（去 I L O U），約 40 位元 | 邀請連結、各端 | 明碼（DO 名稱） | 到房間刪除 | 不撤銷，非秘密 |
| 加入密鑰 `joinSecret` | 伺服器 | base64url 22 字（128 位元） | 邀請連結的 `#` 片段 | 雜湊 | 到房主重生或關房；一份可多人用，**只在第一次加入那一刻用**，之後重連只用 `deviceId` | 房主 `invite.regenerate`，舊的立刻失效（已加入的人不受影響） |
| 執行端憑證 `executorToken` | 伺服器（建房時簽發） | base64url 43 字（256 位元），不透明亂數，不是 JWT | 桌面版本機（設定目錄，只限本人可讀；包 2 定位置） | 雜湊 | 到房間關閉或刪除；桌面版「退出／暫停房間模式」只斷線、**不撤憑證**（§7.4） | 只有房主 `room.close` 與閒置清理 |
| 配對碼 `pairCode` | 伺服器（執行端請求時發） | Crockford base32 8 字，顯示為 `XXXX-XXXX` | 只顯示在桌面版畫面 | 雜湊＋到期時間＋`grantHost` 旗標 | 10 分鐘、一次性 | 用掉即失效；再發新碼時舊碼作廢；全房累計錯 5 次鎖 10 分鐘 |

- 邀請連結形如 `https://<主機>/r/<roomCode>#k=<joinSecret>`：密鑰放片段，不進伺服器存取紀錄與 Referer。
- 出額度者自己用的連結（桌面版 QR）可多帶 `&p=<pairCode>`，手機掃了自動加入並配對，一步完成「一鍵」〔作者裁決 2026-11-02：出額度者一鍵、其他人零設定〕；給朋友的邀請連結不帶 `p`。
- `deviceId` 等同密碼：伺服器絕不廣播、不寫進事件；對外一律用伺服器配的 `memberId`（`m1`、`m2`…，非秘密）。
- 同一 `deviceId` 同時開第二條連線：新的生效，舊的收 `E_SESSION_REPLACED` 後關閉。執行端同理（`E_EXECUTOR_REPLACED`）。
- 配對碼規則：還沒有出額度者時，用碼的人成為出額度者（`grantHost` 碼另外成為房主）；已有出額度者時，一般碼回 `E_STATE_CONFLICT`，`grantHost` 碼只授予房主。建房後執行端發的第一張碼是 `grantHost`。房主失聯救濟＝執行端再發一張 `grantHost` 碼（桌面版面板的「重設房主」）；出額度者換手機＝執行端 `pair.revoke` 後再發碼。

## 2 房間生命週期〔模型判斷·未裁決〕

```
          建房(執行端 HTTP)        room.start(房主)          room.close(房主)
 (無) ───────────────▶ lobby ─────────────────▶ playing ─────────────────▶ closed ──(7 天)──▶ 刪除
                         │                         │  ▲
                         │                         ▼  │ 執行端 job.result
                         │                      transition（換幕中）
                         └──────────────── 閒置 N 天 ─────────────────────────────────────▶ 刪除
```

- **建房**：只由執行端建（`POST /api/rooms`，§6.1），接著送這一桌的公開名冊與公開歷史（`room.seed`，§6.2）。建好即 `lobby`；執行端接著發一張 `grantHost` 配對碼顯示在面板。
- **加入**：成員開連結 → 填暱稱 → `hello`（帶 `joinSecret`）→ 成為成員；`lobby` 與 `playing` 都能加入，`closed` 不能。
- **選角**：名冊每張卡有 `playable`（舞台卡為 false，沿用桌面版 `stage` 旗標）。成員 `seat.claim` 認領一張 `playable` 且未被認領的卡＝那張卡成為**真人玩家卡**；沒被認領的卡都是 **NPC 卡**，照桌面版 GM 點名接話。房間設定 `allowCustomCards`（預設 true）開著時，成員可 `seat.create` 自建簡易玩家卡（名字＋公開描述），由執行端在桌上建卡後回送名冊。一人最多一張卡。
- **開始**：房主 `room.start`（至少 1 名玩家、執行端在線）→ `playing`，開第 1 回合（`kind: "open"`，§3.1）。
- **換幕**：見 §3.5；期間房間狀態 `transition`。
- **結束**：房主 `room.close` → `closed`：所有連線收 `room.closed` 後伺服器關閉連線，執行端憑證撤銷；成員憑證只剩唯讀＋房主可匯出，保留 7 天後刪除。這是結束房間的唯一途徑。
- **閒置清理**：最後一次「持久事件或任何連線建立」之後 N 天，DO 鬧鐘整房刪除（storage `deleteAll`）。**N 預設 30**，部署設定 `ROOM_IDLE_DAYS` 可調（1–365）。房間頁固定顯示「閒置 N 天會清除，記得匯出」。

```ts
type RoomStatus = "lobby" | "playing" | "transition" | "closed";
interface RoomSettings {
  maxPlayers: number;            // 2–8，預設 6
  actionTimeoutSec: number | null; // null＝關（預設）；開時 30–3600
  shuffleOrder: boolean;         // 預設 false＝照入座順序
  allowCustomCards: boolean;     // 預設 true
}
interface SceneSettings {        // 同一幕固定，換幕才換〔作者裁決 2026-11-02〕
  route: string;                 // 執行端自報的路線 id（不透明，例：桌面版的傳輸設定），≤64 字
  model: string;                 // 執行端自報的模型 id，≤200 字
  modelLabel: string;            // 顯示用，所有人只看得到「GM：<modelLabel>」〔作者裁決 2026-11-02：玩家零設定、看不到供應商〕
}
```

## 3 回合狀態機〔模型判斷·未裁決，計畫 §5〕

### 3.1 回合種類與階段

每一回合有一個 `kind`：
- `open`：房間開始的第一回合，沒有行動格，直接生成（GM 開場、介紹在場角色）。
- `scene_open`：換幕後的第一回合，同上（GM 接新幕）。
- `play`：一般回合，先行動格再生成。

```
           ┌──────────────────────── 下一回合 r+1（kind=play）◀─────────────────────────┐
           ▼                                                                              │
 play：  acting(r) ──結算──▶ queued(r) ──執行端在線──▶ generating(r) ──gen.done──────────┘
                              (執行端離線時停在這)         │   ▲
                                                           │   │ retry
                                                      gen.failed
                                                           ▼   │
                                                         held(r) ──round.continue──▶ acting(r+1)
 open／scene_open：從 queued(r) 起跳，沒有 acting。
 gen.abort：generating(r) → 執行端回 gen.done{aborted:true}（半截保留、標中斷，同桌面版）→ acting(r+1)。
```

```ts
type RoundPhase = "acting" | "queued" | "generating" | "held";
interface RoundView {
  round: number; kind: "open" | "scene_open" | "play"; scene: number; phase: RoundPhase;
  roster: Array<{ memberId: string; cardId: string; status: "pending" | "drafting" | "submitted" | "skipped"; text: string | null }>; // text＝已送出的行動（公開）
  deadline: number | null;       // epoch ms；倒數關或暫停時為 null
  pausedRemainingMs: number | null;
  dispatchId: string | null;     // queued 之後才有
  hold: PublicExecutorError | null; // held 時的公開錯誤
}
```

### 3.2 結算條件（先到先算）

在 `acting(r)`，以下任一成立就結算，結算只發生一次（伺服器單執行緒保證）：
1. **全員已行動或已跳過**：`roster` **不為空**，且每人 `submitted` 或 `skipped`。
2. **房主結算**：`round.resolve { round }`。沒送的人算「觀望」，草稿留給下一回合。
3. **倒數到期**（房間設定開了才有）：有草稿的自動當送出、沒草稿的算觀望；不靜默吃掉，派工裡明確標出（§4.4 `watch`）。

**`roster` 為空時一律不結算**：停在 `acting`，三條都不成立（倒數暫停、房主結算回 `E_STATE_CONFLICT`），直到有人入座。避免空批次派工 → 開下一回合 → 再結算的無人值守迴圈。

**執行端離線時一律不結算**（計畫 §2「回合不結算」）：倒數暫停（`deadline` 改 null、記 `pausedRemainingMs`），條件 1 成立也先等；房主結算回 `E_EXECUTOR_OFFLINE`。執行端回來後倒數照剩餘時間續跑、條件 1 立刻重判。

結算 → 伺服器依順序（入座順序；`shuffleOrder` 時每回合用伺服器亂數洗）組成批次 → `queued(r)` → 執行端在線即派工 → `generating(r)`。

### 3.3 回合號＝版本號、行動公開

- 所有「針對某回合」的客戶端指令都帶 `round`；伺服器比對 `round === 當前回合` 且階段合法，否則回 `E_ROUND_STALE`（回合已往下走）或 `E_ROUND_NOT_ACTING`（GM 生成中）。客戶端收到後草稿原樣保留。
- **一房同時只有一個 GM 請求在飛**（任務檔 Constraints）：`queued`／`generating`／`held`／`transition` 期間不開下一回合、不接新派工。
- `action.draft` 不帶回合號：草稿是「我下一次要送的字」，GM 生成中照收（**可打字不可送**），開下一回合時原樣帶著。
- **送出即公開**〔作者裁決 2026-11-02，裁決 9〕：`action.submitted` 帶行動全文廣播給全房，大家結算前就看得到彼此寫了什麼。**草稿不公開**〔模型判斷·未裁決〕：只存伺服器、只回給本人，別人只看到狀態 `drafting`。
- 送出後、結算前可 `action.retract` 收回成草稿再改（廣播 `action.retracted`，畫面上撤掉那段公開文字）；結算後不能改。

### 3.4 中途加入與離開

- **中途加入**：`playing` 中入座的人先記為「待登場」；下一次開 `acting` 時進 `roster`，該回合派工的 `arrivals` 帶上他，GM 旁白補一句登場（包 3 處理提示）。`roster` 為空時停著的 `acting`（§3.2）有人入座就直接把他加進當前 `roster`。
- **斷線不等於離開**：斷線的人仍在 `roster`，靠跳過鈕、房主結算或倒數推進。
- **主動離開**（`member.leave`、`seat.release`）或**被踢**：座位立刻空出，該卡立刻從當前與之後的 `roster` 移除（當下重判結算條件 1；移完為空就照 §3.2 停住）；房主用 `member.departure { cardId, mode }` 決定 `npc`（卡回 NPC 池由 GM 演）或 `absent`（標離席、不點名）。房主沒決定前預設 `absent`。決定隨下一次派工的 `departures` 送執行端。
- **換裝置**：新裝置以新成員加入，房主 `member.reassign { from, to }` 把座位（含草稿）移過去，舊成員變無座。
- 執行端 `roster.update` 讓被認領的卡消失或變 `playable:false` 時，照「主動離開」處理。

### 3.5 換幕

- 只有房主能發 `scene.advance { round, next? }`，合法條件：`acting(round)`（或 `held` 且錯誤種類 `scene_full`）**且執行端在線**，否則 `E_ROUND_NOT_ACTING`／`E_STATE_CONFLICT`／`E_EXECUTOR_OFFLINE`。執行端不換〔作者裁決 2026-11-02，裁決 8〕；`next` 只能給 `route`／`model`，且只在房主本人就是出額度者時接受，否則用出額度者事先以 `ctl.set_next` 存的「下一幕設定」，沒存就沿用目前設定。〔作者裁決 2026-11-02：同一幕固定、換幕可換供應商與模型〕
- 流程：`round.cancelled(r)`（已送出的行動退回成草稿）→ 房間 `transition`（帶 `jobId`）→ 執行端收 `job.scene_advance` → 回 `job.ack`，摘要期間至少每 5 分鐘一則 `job.progress` → 回 `job.result`（`ok` 帶 `summary`，走桌面版既有換幕摘要）→ `scene.summary`＋`scene.started { scene+1, settings }` → `playing`，開 `scene_open` 回合 r+1。
- **`transition` 只由執行端的 `job.result` 結束**，伺服器不自己判失敗——桌面版的 `advance_scene` 是一次 LLM 呼叫，做完會把 `current_scene` +1，房間若自行退回同幕，幕號就對不上。
  - 不套 30 秒逾時。15 分鐘沒有任何 `job.*` 訊框 → 房間標 `stalled`（廣播 `scene.stalled`），房主可 `scene.advance_cancel` → 伺服器發 `job.cancel { jobId }`，執行端中止摘要（既有 `turn_id` 中止）後照實回 `job.result`：中止成功回失敗、已經提交回 `ok`。伺服器照收到的結果走。
  - 執行端離線：房間停在 `transition` 等；重連後伺服器在 `welcome` 帶同一個 `jobId` 的工作，執行端以 `jobId` 冪等處理（做完了就重送結果、還在做就繼續、沒收過就開始），伺服器以 `jobId` 冪等收下結果。
  - 結果失敗：發 `scene.failed`，房間退回 `playing`，重開 `acting(r+1)`（kind=play、同一幕），房主可再試。
- 桌面版的**換幕容量鎖**（`scene_capacity_full`）擋下生成時，執行端回 `gen.failed`，錯誤種類 `scene_full`，房主看到「這一幕滿了，請換幕」；這種 `held` 只能 `scene.advance`。

## 4 傳輸與共通框架〔模型判斷·未裁決〕

### 4.1 端點

| 端點 | 用途 |
|---|---|
| `POST /api/rooms` | 執行端建房（§6.1） |
| `GET  /api/rooms/<roomCode>/ws?role=member\|executor` | WebSocket；憑證在第一則 `hello`，不放 URL |
| `GET  /api/rooms/<roomCode>/export` | 匯出（`Authorization: Bearer <deviceId>`，只房主；§9.3） |
| `GET  /r/<roomCode>` | `party/` 靜態頁（Workers Static Assets） |

### 4.2 信封

```ts
// 任一方向都是 JSON 文字訊框
interface Frame<T extends string = string, P = unknown> {
  t: T;          // 訊息種類
  id?: string;   // 發送方自配的請求 id（1–36 字 [A-Za-z0-9_-]），要回應的指令必帶
  p: P;          // 內容
}
// 伺服器 → 發送者：請求結果
type Reply = Frame<"ok", { id: string; result?: unknown }> | Frame<"err", { id?: string; code: ErrorCode; retryAfterMs?: number; detail?: string }>;
// 伺服器 → 廣播：持久事件（有序號）與即時訊息（無序號）
type EventFrame = Frame<"ev", { seq: number; at: number; e: RoomEvent }>;
type VolatileFrame = Frame<"vol", VolatileEvent>;
```

- **序號 `seq`**：每房從 1 起單調遞增，伺服器在同一筆交易裡寫事件與房間狀態時配發（§9.1）。持久事件只增不改。
- **即時訊息**（串流 delta、在線狀態、草稿狀態、額度、執行端狀態）不配序號、不落事件日誌，重連時由快照帶回當下值。
- 先落地再廣播：事件寫進 storage 成功後才送出 `ev`。
- `hello` 之前送別的 → `E_AUTH_REQUIRED` 並關閉（4401）。`hello` 10 秒內沒到 → 關閉（4401）。
- **心跳，兩種連線分開**：
  - 成員連線：客戶端每 15 秒送固定字串 `{"t":"ping"}`，DO 用 WebSocket 自動回應回 `{"t":"pong"}`、**不喚醒**。成員在線＝連線還開著（關閉事件或下次喚醒時檢查自動回應時間戳超過 45 秒就當斷線）；成員在線只影響顯示，不牽動回合規則。
  - 執行端連線：每 15 秒送 `{"t":"hb","p":{}}`，這是一般訊息、**會喚醒 DO**，伺服器記 `lastSeen` 並把鬧鐘排在 `lastSeen + 30 秒`；到期沒新訊框＝執行端離線（廣播、暫停倒數，§3.2）。執行端在線判定牽動「不結算」，所以不走自動回應。
- 關閉碼：4400 訊框錯、4401 認證、4403 被踢或封鎖、4404 房間不存在、4409 被同憑證新連線取代、4410 房間已關或已刪、4426 版本過舊。
- 版本：`hello.p.protocol = 1`；伺服器不認得就 4426。

### 4.3 重連與狀態同步

客戶端與執行端都一樣：`hello` 帶 `lastSeq`（第一次為 0）。伺服器：
- `lastSeq` 落後 ≤ 500 筆：回 `welcome`＋逐筆補 `ev`（`seq > lastSeq`）。
- 落後更多、或 `lastSeq` 為 0：回 `welcome { snapshot }`，客戶端整個換掉本地狀態，從 `snapshot.seq` 接著收。
- 快照帶當下的即時值：在線名單、草稿狀態、執行端狀態、額度、**正在串流的那一段**（伺服器記在記憶體的累積文字；DO 被逐出後遺失，就只等該段 `gen.part.end`）。
- 更早的劇情用 `history.page { beforeSeq, limit ≤ 200 }` 往回翻。

```ts
interface Snapshot {
  seq: number;
  room: { code: string; status: RoomStatus; settings: RoomSettings; hostId: string; payerId: string | null; scene: number; sceneSettings: SceneSettings | null; transition: { jobId: string; since: number; stalled: boolean } | null; createdAt: number; idleDays: number };
  me: { memberId: string; nickname: string; cardId: string | null; isHost: boolean; isPayer: boolean; draft: string | null };
  members: Array<{ memberId: string; nickname: string; cardId: string | null; connected: boolean }>;
  cards: PublicCard[];
  executor: ExecutorView;
  round: RoundView | null;
  story: StoryPart[];            // 本幕全部段落＋各幕摘要；更早的用 history.page
  streaming: { round: number; part: number; speaker: Speaker; text: string } | null;
}
```

### 4.4 共用型別

```ts
interface PublicCard {           // 執行端上送的公開投影；私設、世界書、GM 專屬內容一律不送
  cardId: string;                // 桌面版卡 id，1–64 字 [A-Za-z0-9_-]
  name: string;                  // 1–64 字
  color: string | null;          // #rrggbb
  avatar: string | null;         // data:image/(png|jpeg|webp);base64，≤ 64 KiB，縮圖由執行端產
  blurb: string;                 // 公開設定節錄，≤ 1000 字
  playable: boolean;             // 舞台卡 false
  hidden: boolean;               // 桌面版自動隱藏區的卡（回歸後由名冊更新改 false）
  claimedBy: string | null;      // 伺服器填，執行端送的值忽略
}
type Speaker = { kind: "gm" } | { kind: "npc"; cardId: string; name: string } | { kind: "player"; memberId: string; cardId: string; name: string } | { kind: "system"; marker: "scene_summary" | "card_arrival" | "person_arrival" | "state_update" };
interface StoryPart { round: number; part: number; speaker: Speaker; text: string; interrupted: boolean; at: number } // 只有顯示文字；不帶 raw、變數表（手機不做卡片介面，§13）
interface ActionItem {           // 派工批次的一格，順序即提示順序
  memberId: string; cardId: string; name: string;  // name＝卡名（GM 看到的角色名）
  kind: "act" | "watch";
  text?: string;                 // kind=act 必有
  watchReason?: "skipped" | "timeout" | "host_resolved";
}
```

## 5 客戶端 ↔ 伺服器訊息〔模型判斷·未裁決〕

### 5.1 客戶端 → 伺服器

「誰」：M＝任何成員、P＝玩家（有座位）、H＝房主、F＝出額度者。「狀態」指房間狀態／回合階段。除註明外成功回 `ok`。

| t | p | 誰 | 合法狀態 | 伺服器處理 |
|---|---|---|---|---|
| `hello` | `{ protocol:1, role:"member", deviceId, joinSecret?, nickname?, pairCode?, lastSeq, client:{ name:"party", version } }` | 任何人 | 非 closed；closed 時只認既有成員（唯讀） | 新裝置必帶 `joinSecret`＋`nickname`；驗過建成員、發 `member.joined`；帶 `pairCode` 時接著做 `pair.claim`。回 `welcome` |
| `profile.rename` | `{ nickname }` | M | 非 closed | 發 `member.renamed`；已入座的卡名不變（GM 看卡名） |
| `seat.claim` | `{ cardId }` | M（無座） | lobby、playing | 卡要 `playable`、未被認領、座位未滿；發 `seat.claimed`；playing 中照 §3.4 |
| `seat.release` | `{}` | P | lobby、playing | 發 `seat.released`；playing 中照 §3.4 離開處理 |
| `seat.create` | `{ name, blurb }` | M（無座） | lobby、playing；`allowCustomCards` 開；執行端在線 | 轉執行端 `job.card_create`；執行端回名冊後自動替他 `seat.claim` |
| `action.draft` | `{ text }` | P | playing、transition | 存草稿（最多每 500 ms 一次），**不公開**，只廣播 `vol.roster_status`；空字串＝清草稿 |
| `action.submit` | `{ round, text }` | P（在 roster） | acting(round) | 發 `action.submitted { round, memberId, text }`（全文公開），清草稿，判結算條件 1 |
| `action.skip` | `{ round }` | P（在 roster） | acting(round) | 發 `action.skipped`，判結算條件 1 |
| `action.retract` | `{ round }` | P | acting(round) 且自己已 submitted／skipped | 發 `action.retracted`，原文回成草稿 |
| `round.resolve` | `{ round }` | H | acting(round)、roster 不為空、執行端在線 | 結算（§3.2-2）；roster 空回 `E_STATE_CONFLICT`、執行端離線回 `E_EXECUTOR_OFFLINE` |
| `round.continue` | `{ round }` | H 或 F | held(round) | 放棄這回合剩下的生成，已落的段落保留，開 acting(round+1) |
| `gen.abort` | `{ round }` | H 或 F | generating(round) | 轉執行端 `abort`；等執行端回 `gen.done{aborted:true}` |
| `room.start` | `{}` | H | lobby、≥1 玩家、執行端在線 | 發 `room.started`、開 `open` 回合 |
| `room.settings` | `{ patch: Partial<RoomSettings> }` | H | lobby、playing（倒數改動下一回合起生效） | 驗範圍，發 `room.settings_changed` |
| `invite.regenerate` | `{}` | H | 非 closed | 回 `{ joinSecret }`（只回給房主，不廣播），發 `invite.regenerated`（不含密鑰） |
| `host.transfer` | `{ memberId }` | H | 非 closed | 發 `host.changed` |
| `member.kick` | `{ memberId, ban }` | H | 非 closed | 發 `member.kicked`；`ban` 時記雜湊，同裝置再也加不回；關閉對方連線 4403 |
| `member.departure` | `{ cardId, mode:"npc"\|"absent" }` | H | playing | 發 `seat.departure`，下次派工帶上 |
| `member.reassign` | `{ from, to }` | H | lobby、playing | `to` 必須無座；發 `seat.reassigned` |
| `member.leave` | `{}` | M | 任何 | 發 `member.left`，伺服器關連線；成員資料保留（可再用同裝置回來，成為無座成員） |
| `scene.advance` | `{ round, next?: { route, model } }` | H | 見 §3.5 | §3.5 |
| `scene.advance_cancel` | `{}` | H | transition 且 `stalled` | 發 `job.cancel` 給執行端；房間仍等 `job.result`（§3.5） |
| `room.close` | `{}` | H | 非 closed | §2 |
| `pair.claim` | `{ code }` | M | 非 closed | 驗碼與 §1.2 配對碼規則，發 `pair.bound { memberId }`；`grantHost` 碼另發 `host.changed`；通知執行端 `pair.claimed` |
| `pair.leave` | `{}` | F | 任何 | 發 `pair.unbound`；沒有出額度者時遠端控制關閉，直到再配對 |
| `ctl` | `{ ctlId, cmd: RemoteCommand }` | F | 見 §7.3 | 轉執行端，執行端回 `ctl.result` 後轉回發令者 |
| `history.page` | `{ beforeSeq, limit }` | M | 任何 | 回 `{ events: EventFrame[] }`（只含劇情類事件） |
| `ping` | — | 任何 | 任何 | 自動回應 `pong`（§4.2） |

### 5.2 伺服器 → 客戶端

- `welcome { snapshot? , memberId }`、`ok`、`err`、`pong`。
- 持久事件 `ev.e`（`RoomEvent`，各帶 `type`）：`room.created`、`room.started`、`room.settings_changed`、`room.closed`、`member.joined { memberId, nickname }`、`member.renamed`、`member.left`、`member.kicked`、`host.changed`、`invite.regenerated`、`seat.claimed { memberId, cardId }`、`seat.released`、`seat.reassigned`、`seat.departure { cardId, mode }`、`roster.updated { cards }`、`pair.bound`、`pair.unbound`、`scene.advancing { jobId }`、`scene.stalled { jobId }`、`scene.failed { error: PublicExecutorError }`、`scene.summary { scene, text }`、`scene.started { scene, settings }`、`scene.settings_switched { settings, reason }`（§7.3 幕中例外）、`round.opened { round, kind, roster, deadline }`、`action.submitted { round, memberId, text }`、`action.skipped`、`action.retracted`、`round.resolved { round, by:"all"|"host"|"timeout", batch: ActionItem[] }`、`round.cancelled { round }`、`gen.dispatched { round, dispatchId }`、`story.part`（`StoryPart`）、`gen.failed { round, error: PublicExecutorError }`、`gen.done { round, aborted }`。
- 即時訊息 `vol`：
  - `delta { round, part, speaker?, text }`：`speaker` 只在該段第一則帶。客戶端把同 `(round, part)` 的 `text` 依序串接成暫時顯示；收到 `story.part` 同一段時**以它的 `text` 整段取代**（delta 是執行端串流的原樣增量，可能含狀態區塊與點名行；定稿才是剝乾淨的顯示文字）。沿用網頁版 `stream-chat.ts` 的 `onDelta(string)` 形狀，只多一層 `(round, part)` 定位。
  - `presence { memberId, connected }`、`roster_status { round, memberId, status }`、`executor { ExecutorView }`、`quota { coarse: QuotaCoarse }`（全員）／`quota_detail { report: QuotaReport }`（只出額度者）、`error_detail { round, error: ExecutorError }`（只出額度者，§7.1）。

```ts
interface ExecutorView {
  label: string;                          // 執行端自報裝置名，≤ 40 字
  online: boolean; since: number;
  keepAwake: boolean | null;              // 防休眠是否生效（§7.4）
  modelLabel: string | null;              // 下一幕設定的顯示名
}
```

## 6 執行端 ↔ 伺服器訊息〔模型判斷·未裁決〕

### 6.1 HTTP

```ts
// POST /api/rooms   （未認證；同 IP 每小時 10 次，由殼的限流負責，§12.1）
interface CreateRoomReq { protocol: 1; label: string; client: { name: "desktop"; version: string } }
interface CreateRoomRes { roomCode: string; joinSecret: string; executorToken: string; inviteUrl: string }
```

`joinSecret` 給桌面版面板產生邀請連結與 QR。

### 6.2 執行端 → 伺服器

| t | p | 合法時機 | 伺服器處理 |
|---|---|---|---|
| `hello` | `{ protocol:1, role:"executor", roomCode, executorToken, lastSeq, inflight: InflightReport \| null, client }` | 連線後第一則 | 驗憑證，回 `welcome { snapshot, dispatch?, job? }`；廣播 `vol.executor` 上線；去重見 §6.5 |
| `hb` | `{}` | 每 15 秒 | 記 `lastSeen`（§4.2） |
| `capabilities` | `{ routes: RouteOffer[], current: { route, model, modelLabel }, maxRoundSpeakers }` | 任何時候（連上後必送一次） | 存起來給 `ctl`／換幕驗證；`maxRoundSpeakers` 只顯示用（上限照桌面版偏好 `max_round_speakers`，執行端自己套） |
| `room.seed` | `{ part, total, cards?: PublicCard[], history?: StoryPart[] }` | 建房後、`room.start` 前 | 分片（每片 ≤ 512 KiB）；最後一片到齊後一次落地為 `roster.updated`＋歷史段落（標 `round: 0`） |
| `roster.update` | `{ cards: PublicCard[] }` | 任何時候 | 整份取代名冊（保留 `claimedBy`）；受影響座位照 §3.4 |
| `dispatch.ack` | `{ dispatchId }` | 收到 `dispatch` 後 | `queued` → `generating` |
| `gen.part.begin` | `{ dispatchId, part, speaker: Speaker }` | generating | `part` 從 0 起、逐段 +1；伺服器開記憶體緩衝 |
| `gen.delta` | `{ dispatchId, part, text }` | generating，該段 begin 後 | 累積並廣播 `vol.delta`；執行端應合併成每 ≥ 80 ms 或 ≥ 200 字一則 |
| `gen.part.end` | `{ dispatchId, part, text, interrupted }` | 該段 begin 後 | 落 `story.part`（冪等：同 `(dispatchId, part)` 第二次直接回 ok） |
| `gen.done` | `{ dispatchId, aborted, cardsArrived?: string[] }` | generating | 發 `gen.done`；開 acting(r+1)；`cardsArrived` 是本回合回歸的隱藏卡 |
| `gen.failed` | `{ dispatchId, atPart: number \| null, error: ExecutorError }` | generating | 發 `gen.failed`（公開版）＋`vol.error_detail`（只出額度者）；→ `held` |
| `job.ack` | `{ jobId }` | 收到 `job.*` 後 | 記進度時間 |
| `job.progress` | `{ jobId }` | 工作執行中，至少每 5 分鐘 | 記進度時間；`stalled` 時解除 |
| `job.result` | `{ jobId, ok, result?, error?: ExecutorError }` | 收到 `job.*` 後 | 以 `jobId` 冪等；依工作種類推進（換幕 §3.5、建卡） |
| `pair.create` | `{ grantHost: boolean }` | 任何時候 | 回 `{ pairCode, expiresAt }`，舊碼作廢 |
| `pair.revoke` | `{}` | 有出額度者時 | 解除配對（桌面版面板「不是我」／出額度者換手機） |
| `status` | `{ keepAwake: boolean \| null, busy: boolean }` | 任何時候 | 廣播 `vol.executor` |
| `quota` | `QuotaReport` | 任何時候 | 廣播 `vol.quota`（粗）與 `vol.quota_detail`（細） |
| `ctl.result` | `{ ctlId, ok, error?: ExecutorError }` | 收到 `ctl` 後 | 轉回發令者 |

### 6.3 伺服器 → 執行端

| t | p | 執行端該做的 |
|---|---|---|
| `welcome` | `{ snapshot, dispatch: Dispatch \| null, job: Job \| null }` | 對照本機紀錄決定重送或續跑（§6.5） |
| `dispatch` | `Dispatch` | 回 `dispatch.ack`，走既有 GM 流程（§6.4） |
| `abort` | `{ dispatchId }` | 呼叫既有中止（`chat_abort` 對應的 `turn_id`）；之後照常送 `gen.part.end{interrupted:true}`＋`gen.done{aborted:true}` |
| `ctl` | `{ ctlId, by: memberId, cmd: RemoteCommand }` | §7.3 |
| `job.scene_advance` | `{ jobId, scene, nextSettings }` | `job.ack` → 桌面版既有換幕（摘要），期間 `job.progress` → `job.result { summary }`；不套 30 秒逾時（§3.5） |
| `job.card_create` | `{ jobId, memberId, name, blurb }` | `job.ack` → 在桌上建玩家卡、送 `roster.update` → `job.result { cardId }` |
| `job.cancel` | `{ jobId }` | 中止該工作，照實回 `job.result`（已提交就回 ok） |
| `pair.claimed` | `{ memberId, nickname, grantHost }` | 面板顯示「X 已配對」並給「撤銷」 |
| `vol.*` | 同客戶端（在線、草稿狀態） | 面板顯示在線玩家用，可忽略 |

### 6.4 派工

派工內容＝玩家行動文字＋在場名單＋幕設定，**不是提示詞**；提示詞在桌面版組（計畫 §2、§8）。

```ts
interface Dispatch {
  dispatchId: string;           // ULID；桌面版拿它當這一批事件的 action_id，作為冪等鍵
  round: number; kind: "open" | "scene_open" | "play"; scene: number;
  settings: SceneSettings;      // 執行端照這組 route/model 打；以房間覆寫值為準、不寫回桌面版全域設定（§13）
  actions: ActionItem[];        // 依序；open／scene_open 為空陣列
  players: Array<{ cardId: string; memberId: string; name: string; connected: boolean }>; // 在場真人玩家卡
  arrivals: string[];           // 本回合登場的玩家卡 cardId（§3.4）
  departures: Array<{ cardId: string; mode: "npc" | "absent" }>;
}
type Job = { jobId: string; kind: "scene_advance"; scene: number; nextSettings: SceneSettings } | { jobId: string; kind: "card_create"; memberId: string; name: string; blurb: string };
```

執行端流程（沿用桌面版既有流程，細節屬包 2／包 3）：
1. 本機桌事件裡已有 `action_id === dispatchId` 的玩家事件 → 不重落（去重）。否則把 `actions` 依序落成玩家事件，走既有玩家輸入路徑（`append_player_event` 同一條，含換幕容量鎖）；`watch` 落成觀望標記。
2. 跑 GM 鏈：`gm_narrate` → 點名 NPC 就 `chat_with_character` → 直到點名玩家、沒點名或達 `max_round_speakers`。每一次呼叫是一段（`part`）。
3. 每段：`gen.part.begin` → `on_delta` 的每則增量（合併後）→ `gen.part.end`（`text`＝桌面版剝乾淨的落檔正文，`GmNarration.text`／`ChatReply.text`）。點名列、GM 專屬（`gm_only`）事件、狀態原文（`raw`）、變數表都不上送，協定也沒有給它們的欄位；公開的系統事件（狀態更新、登場）可以 `speaker.kind = "system"` 一段送出。
4. 全鏈完成 → `gen.done`。

### 6.5 斷線、重連與去重

```ts
interface InflightReport {      // 執行端本機持久記住的最後一筆派工與最後一筆工作
  dispatch: { dispatchId: string; state: "running" | "done" | "failed"; lastEndedPart: number | null } | null;
  job: { jobId: string; state: "running" | "done" } | null;
}
```

- 執行端把「已送出但沒收到 `ok`」的 `gen.part.end`／`gen.done`／`gen.failed`／`job.result` 排在本機佇列，重連後依序重送；伺服器以 `(dispatchId, part)`、`dispatchId`、`jobId` 冪等收下。
- 伺服器在 `welcome` 給當前派工與當前工作（若有）。執行端判斷：
  - ID 相同且 `running` → 繼續跑，不重落玩家事件、不重做摘要。
  - 相同且 `done`／`failed` → 只重送佇列。
  - 不同或沒有 → 當作新的，派工照 §6.4-1 先查 `action_id` 去重再跑。
- 同一回合只派一次：伺服器只在 `queued → generating` 那一刻產生 `dispatchId`，之後重送都用同一個；`retry` 也用同一個（從 `atPart` 續跑）。
- 執行端 `generating` 中斷線：房間停在 `generating`、廣播離線（手機顯示「GM 主機離線，等待中」），不結算、不改派。執行端在線但 15 分鐘沒有任何 `gen.*` 訊框 → 伺服器自行轉 `held`，錯誤 `E_GEN_TIMEOUT`。換幕工作的離線與逾時見 §3.5。

## 7 執行端專屬〔模型判斷·未裁決，計畫 §2〕

### 7.1 無人值守錯誤上送

```ts
type ExecutorErrorKind =
  | "refusal"            // 供應商拒答（AI_CONTENT_FILTERED）
  | "quota"              // 額度上限／限流（AI_HTTP_STATUS_429、AI_FREE_MODEL_BUSY、訂閱超額通知）
  | "tool_format"        // 命令列工具升版或輸出格式不對（CliTooOldRequired、CliUpdating、AI_INCOMPLETE_RESPONSE 等）
  | "provider_unreachable" // 連不上、串流停滯（AI_CALL_FAILED、AI_STREAM_STALLED、其他 AI_HTTP_STATUS_*）
  | "scene_full"         // 換幕容量鎖、上下文過長（scene_capacity_full、AI_CONTEXT_TOO_LONG）
  | "not_ready"          // 執行端沒設定好（金鑰缺、命令列未登入、風險說明未同意等）
  | "other";
type RemoteAction = "retry" | "switch_model" | "switch_route" | "continue" | "scene_advance";
interface ExecutorError {        // 只給出額度者
  kind: ExecutorErrorKind;
  code: string;                  // 桌面版既有錯誤碼前綴（例 AI_CONTENT_FILTERED），≤ 64 字
  message: string;               // 執行端在地化的短說明，≤ 500 字；不得含回應本文、金鑰、帳號、路徑
  actions: RemoteAction[];       // 這次可做的動作
  resetsAt: number | null;       // quota 類已知恢復時間
}
interface PublicExecutorError { kind: ExecutorErrorKind; actions: RemoteAction[]; resetsAt: number | null } // 給全員
```

| kind | 預設 actions | 全員看到 | 出額度者看到 |
|---|---|---|---|
| refusal | switch_route, switch_model, retry, continue | 「GM 這段被供應商擋下，等出額度者處理」 | 原因＋「切境外重送」等鈕〔作者裁決 2026-11-02：拒答明確告知、給切境外一鍵〕 |
| quota | switch_route, switch_model, retry | 「GM 額度用完，約 hh:mm 恢復」 | 重置時間＋「臨時切 OpenRouter」 |
| tool_format | retry, switch_route | 「GM 主機需要處理」 | 「命令列工具版本不符，請到電腦更新或切 OpenRouter」 |
| provider_unreachable | retry, switch_route | 「GM 連線不穩，重試中」 | 錯誤碼＋重試 |
| scene_full | scene_advance | 「這一幕滿了，請房主換幕」 | 同左 |
| not_ready | retry | 「GM 主機需要處理」 | 執行端給的說明 |
| other | retry, continue | 「GM 出錯了」 | 錯誤碼＋說明 |

不靜默重試到失敗：執行端自己只做桌面版本來就有的重試，失敗即上送。

### 7.2 額度狀態

```ts
interface QuotaReport {
  route: string; routeLabel: string;
  window: "rolling" | "daily" | "weekly" | "credits" | "unknown"; // 訂閱是時段制上限
  state: "ok" | "near" | "exhausted" | "unknown";
  usedRatio: number | null;      // 0–1，執行端估得出才給
  resetsAt: number | null;       // epoch ms
  creditsLeft: string | null;    // 顯示用字串（例「餘 $1.20」），≤ 40 字
  observedAt: number;
}
type QuotaCoarse = Pick<QuotaReport, "state" | "resetsAt">;
```

執行端在每次 `gen.done`／`gen.failed` 後與狀態改變時送；伺服器只轉播、存最新一份進快照，不累計。

### 7.3 遠端控制指令

```ts
type RemoteCommand =
  | { kind: "retry" }                                   // held：從 atPart 續跑同一 dispatchId
  | { kind: "switch_model"; model: string }             // held 且錯誤 actions 含它：幕中例外
  | { kind: "switch_route"; route: string; model: string } // 同上
  | { kind: "abort" }                                   // generating：同 gen.abort
  | { kind: "set_next"; route: string; model: string }  // 任何時候：存「下一幕設定」，換幕才生效
  | { kind: "refresh_quota" };                          // 任何時候：要執行端重報額度
interface RouteOffer { route: string; label: string; models: Array<{ id: string; label: string; laxity: 1 | 2 | 3 | null }> } // laxity＝寬鬆度三級（計畫 §4），手寫維護，可為 null
```

- 只有出額度者能發（伺服器驗 `pair.bound`），否則 `E_FORBIDDEN`。已有出額度者時別人 `pair.claim` 一般碼回 `E_STATE_CONFLICT`（§1.2）。
- `route`／`model` 必須在執行端 `capabilities.routes` 裡，否則 `E_MODEL_NOT_OFFERED`。
- `switch_model`／`switch_route` 是**唯一的幕中切換**（計畫 §1 例外）：生效後發 `scene.settings_switched { settings, reason: error.kind }`，全員看到「GM 改用 <modelLabel>」，接著自動 `retry`。不在 `held` 或錯誤不允許 → `E_STATE_CONFLICT`。
- 執行端回 `ctl.result`；30 秒沒回 → 伺服器回發令者 `E_EXECUTOR_TIMEOUT`。這個逾時只套遠端控制指令，不套 `job.*`。

### 7.4 在線狀態與防休眠（只寫可觀測行為）

- 執行端連上／斷開（含 `hb` 逾時）→ 伺服器廣播 `vol.executor { online }`，快照帶當下值。
- 執行端在房間模式期間持有防休眠時送 `status { keepAwake: true }`，系統拒絕或平台不支援送 `false`，不知道送 `null`；手機出額度者面板照實顯示。
- 執行端斷線後應以退避（1、2、5、10、30 秒，之後每 30 秒）自動重連；協定不限次數。重連成功的判定＝收到 `welcome`。
- **桌面版「退出／暫停房間模式」與關掉程式都只是斷線**：憑證保留在本機，下次開桌面版自動接回同一房間。協定沒有讓執行端自行撤憑證的訊息；送 `executor.leave` 一律回 `E_STATE_CONFLICT`（位子留給另案，§11）。結束房間只走房主 `room.close`。

## 8 錯誤碼表〔模型判斷·未裁決〕

| 碼 | 意義 | 誰會收到 | 客戶端顯示 |
|---|---|---|---|
| `E_BAD_FRAME` | 不是 JSON、缺欄位、型別不對、未知 `t`、超過大小 | 發送者（並關 4400） | 「版本不符，請重新整理」 |
| `E_VERSION` | `protocol` 不認得或客戶端版本過舊 | 發送者（關 4426） | 「請更新 app／重新整理」 |
| `E_AUTH_REQUIRED` | 沒 `hello` 就送指令 | 發送者（關 4401） | 自動重連 |
| `E_AUTH_FAILED` | `deviceId` 不是本房成員且沒帶有效 `joinSecret`；執行端憑證不對 | 發送者（關 4401） | 「連結已失效，請向房主要新連結」／桌面版「房間憑證失效」 |
| `E_ROOM_NOT_FOUND` | 房間不存在或已清除 | 發送者（關 4404） | 「房間不存在或已過期」 |
| `E_ROOM_CLOSED` | 房間已結束，只能唯讀與匯出 | 發送者 | 「房間已結束」 |
| `E_BANNED` | 被房主封鎖 | 發送者（關 4403） | 「你已被移出房間」 |
| `E_KICKED` | 被房主移出（未封鎖） | 被踢者（關 4403） | 同上，可用新連結回來 |
| `E_ROOM_FULL` | 成員或座位滿 | 發送者 | 「房間已滿」 |
| `E_NAME_INVALID` | 暱稱或卡名不合規則（§10） | 發送者 | 欄位下紅字 |
| `E_TEXT_INVALID` | 行動、草稿、描述長度或字元不合 | 發送者 | 輸入框下紅字，原文保留 |
| `E_FORBIDDEN` | 角色不夠（非房主、非出額度者、無座位） | 發送者 | 「只有房主／出額度者可以這樣做」 |
| `E_ROUND_STALE` | 帶的回合號不是當前回合 | 發送者 | 「回合已經往下走了」，草稿保留 |
| `E_ROUND_NOT_ACTING` | 當前不在行動格階段（GM 生成中、換幕中） | 發送者 | 「GM 還在說，等一下再送」（輸入框照可打字） |
| `E_STATE_CONFLICT` | 指令在目前狀態不合法：名單空時結算、已有出額度者時配對、`executor.leave`、不在 `held` 時幕中切換等 | 發送者 | 重抓快照後以畫面為準，不另跳窗 |
| `E_SEAT_TAKEN` | 卡已被認領或不可認領 | 發送者 | 「這張卡已有人選」 |
| `E_SEAT_REQUIRED` | 沒座位卻送行動 | 發送者 | 「先選角色」 |
| `E_EXECUTOR_OFFLINE` | 需要執行端在線的指令（結算、開始、換幕、自建卡），但它離線 | 發送者 | 「GM 主機離線，等待中」 |
| `E_EXECUTOR_TIMEOUT` | 遠端控制指令 30 秒沒回 | 發令的出額度者 | 「GM 主機沒回應」 |
| `E_GEN_TIMEOUT` | 生成中 15 分鐘沒有進度（轉 `held`） | 全員（`gen.failed` 公開版） | 同 `provider_unreachable` |
| `E_PAIR_CODE_INVALID` | 配對碼錯、過期、已用過 | 發送者 | 「配對碼不對或已過期，請在電腦上重發」 |
| `E_PAIR_LOCKED` | 錯太多次，暫時鎖住 | 發送者（帶 `retryAfterMs`） | 「請 N 分鐘後再試」 |
| `E_MODEL_NOT_OFFERED` | 指定的路線／模型不在執行端清單 | 發送者 | 「這個模型目前不能用」 |
| `E_SETTING_INVALID` | 房間設定值超出範圍 | 發送者 | 欄位下紅字 |
| `E_RATE_LIMITED` | 太頻繁（含建房限流，HTTP 429） | 發送者（帶 `retryAfterMs`） | 草稿類靜默延後；其餘「太快了，稍等」 |
| `E_DISPATCH_UNKNOWN` | 執行端送了不屬於當前派工或工作的結果 | 執行端 | 桌面版記日誌、丟掉該筆，不重送 |
| `E_SESSION_REPLACED` | 同裝置在別處開了新連線 | 舊連線（關 4409） | 「已在其他分頁開啟」，不自動重連 |
| `E_EXECUTOR_REPLACED` | 同執行端憑證有新連線 | 舊連線（關 4409） | 桌面版「此房間已在另一處執行」，不自動重連 |
| `E_INTERNAL` | 伺服器內部錯誤 | 發送者 | 「伺服器出錯，請稍後再試」，自動重連 |

換幕工作 15 分鐘沒進度不是錯誤碼，是 `scene.stalled` 事件（§3.5）。

## 9 持久化與匯出〔模型判斷·未裁決〕

### 9.1 DO storage（SQLite 後端）

| 表 | 內容 | 備註 |
|---|---|---|
| `room` | 單列：狀態、設定、房主、出額度者、幕號、當幕設定、回合狀態（`RoundView` 全部）、換幕工作（`jobId`、進度時間、`stalled`）、加入密鑰雜湊、執行端憑證雜湊與 `capabilities`／下一幕設定／最新 `QuotaReport`／`lastSeen`、建立與最後活動時間 | 與事件同交易更新 |
| `members` | `memberId`、裝置雜湊、暱稱、座位、入座序、已封鎖旗標 | |
| `cards` | `PublicCard`（名冊） | |
| `drafts` | `memberId → text` | 不公開；倒數到期自動送要靠它，所以落地 |
| `codes` | 配對碼雜湊、到期、`grantHost`、全房錯誤計數 | |
| `events` | `seq`（主鍵）、`at`、`type`、`payload` JSON | 對話（`story.part`）、行動（`action.submitted`）、幕摘要（`scene.summary`）、在場（`round.opened` 的 roster）、設定變更都在這裡 |

- 寫入一律「核心算出新狀態＋新事件 → 一筆交易寫 `room`＋`events`＋相關表 → 成功後才廣播」。
- 不存：deltas、任何 AI 金鑰、提示詞、卡片原檔、私設、世界書、`deviceId` 原文、IP。
- 單房事件量 v1 不設上限，靠閒置清理收（§13）。

### 9.2 與桌面版桌檔的關係

- 桌面版執行端的本機桌是 **GM 的正典**：私設、世界書、狀態樹、變數表、原文都只在那裡，提示詞從那裡組。
- 房間事件日誌是 **玩家看得到的正典**：只有公開段落（剝乾淨的正文）、行動、摘要、名冊與在場紀錄。
- 兩者靠 `dispatchId`（＝桌事件 `action_id`）與 `(dispatchId, part)` 對齊；執行端的重送佇列保證桌上落了的段落最終也進房間。反向（房間有、桌上沒有）不會發生：段落一定先在桌上落檔才上送。

### 9.3 匯出成網頁存檔契約

`GET /api/rooms/<code>/export`（房主；`closed` 後 7 天內仍可）回一份照 [web-save.md](../../src/shared/contracts/web-save/web-save.md) v1 的 JSON。**只帶文字（旁白、對話、行動、幕摘要、角色名），不帶任何卡片原檔**：伺服器手上本來就沒有原卡，也不該有。契約是「一桌一張卡、`user`／`char` 兩種角色、沒有幕」，所以這是**有損**匯出：

| 契約欄位 | 對應 |
|---|---|
| `format`／`version`／`exported_at` | 照契約 |
| `card` | 合成的最小卡殼，形狀照 fixture `minimal.json`：`{ spec: "chara_card_v2", spec_version: "2.0", data: { name: "GM", first_mes: <第一段 GM 正文，沒有就「（多人房間紀錄）」> } }` |
| `import_route`／`regex_allowed` | `"character"`／`true` |
| `card_png` | 不帶 |
| `user_name` | 房主暱稱 |
| `opening_index` | `null` |
| `messages` | 依 `seq` 攤平各幕：玩家行動（`round.resolved` 的 `act`）→ `user`，`text`＝`<卡名>：<行動>`；GM 段 → `char`；NPC 段 → `char`，`text`＝`<卡名>：<正文>`；幕摘要 → `char`；觀望與系統段不輸出；`interrupted` 照段落；`id`＝`r<round>p<part>` 或 `r<round>a<序>` |
| `world_info` | `{ entries: [], timed: {}, last_message_id: null, message_effects: {} }` |
| `mvu` | `null` |
| `card_storage` | `{}` |
| 多人版新增 | 頂層 `extensions.table_tavern_room`：`{ version: 1, scenes: [{ scene, summaryMessageId, firstMessageId }], speakers: { <messageId>: Speaker（不含 memberId） }, cast: [{ cardId, name, player: boolean }] }`；契約說「不認得的頂層鍵略過」，現行匯入器會忽略；包 1 在 `web-save.md` 補一段說明這個頂層鍵 |

不含暱稱以外的任何成員資料（無 `deviceId`、無 `memberId`）。

## 10 信任邊界清單〔模型判斷·未裁決〕

所有客戶端與執行端送來的東西都是不可信輸入；伺服器以純函式驗證，失敗回對應錯誤碼、狀態不動。

| 輸入 | 驗什麼 |
|---|---|
| 任何訊框 | UTF-8 JSON 物件；成員訊框 ≤ 16 KiB、執行端訊框 ≤ 1 MiB（Workers 上限）；`t` 在白名單；`p` 欄位齊全、型別對（多的欄位忽略）；字串不含孤立代理字元 |
| 頻率 | 成員每連線 20 則／秒、草稿每 500 ms 一則；配對碼全房錯 5 次鎖 10 分鐘；建房同 IP 每小時 10 次；`history.page` 每秒 2 次 |
| `deviceId`／`joinSecret`／`executorToken`／配對碼 | 格式與長度精確符合 §1.2；比對雜湊用定長比較 |
| `roomCode` | 8 字 Crockford base32，大小寫不分（伺服器轉大寫） |
| 暱稱 | NFC 後去頭尾空白 1–24 個字元（code point）；不含 C0／C1 控制字元、換行、雙向覆寫字元（U+202A–202E、U+2066–2069）、零寬字元以外的格式字元；同房可重名（以 `memberId` 區分） |
| 行動文字／草稿 | NFC、0–2000 字元（送出時 ≥ 1）、最多 30 行；只允許 `\n` 這一種控制字元（`\r\n` 正規化）；不做內容審查（計畫 §8） |
| 自建卡 `name`／`blurb` | 名字同暱稱規則但上限 40；描述 0–1000 字元、規則同行動文字 |
| 回合號、序號、`limit` | 非負安全整數；`limit` 1–200 |
| 身分與狀態 | 每則指令先查角色（§5.1「誰」欄）再查狀態（「合法狀態」欄）；房主指令驗 `hostId`，遠端控制驗 `pair.bound` |
| 房間設定 | 各欄範圍見 §2；未知鍵拒收 |
| 執行端上送的 `PublicCard` | `cardId` 格式、`name` 1–64、`blurb` ≤ 1000、`color` `#rrggbb`、`avatar` 只收三種 MIME 的 data URL 且 ≤ 64 KiB、名冊 ≤ 64 張 |
| 執行端上送的段落 | `text` ≤ 64,000 字元、`part` 0–31 且依序、`dispatchId` 必須是當前派工、`speaker.cardId` 必須在名冊；`ExecutorError.message` ≤ 500、`code` ≤ 64 |
| 執行端 `capabilities`／`job.result` | 路線 ≤ 16、每路線模型 ≤ 200、各字串長度上限；摘要 ≤ 64,000 字元；`jobId` 必須是當前工作 |
| WebSocket `Origin` | 成員連線只收 `party/` 自己的來源；執行端連線不看來源、只看憑證 |

- **客戶端顯示**：所有文字以純文字呈現（`party/` 不插 HTML；若做 Markdown 子集，先跳脫）。頭像只用 `<img>` 吃 data URL。別人的公開行動同樣當不可信文字顯示。
- **玩家行動文字進執行端**：走桌面版既有玩家輸入路徑（同打字送出，落成玩家事件再組提示），不另做拼接；玩家名、自建卡名同樣當不可信字串進提示。執行端仍要自己驗一次長度（不只信伺服器）。
- **執行端上送前的過濾**：只送公開投影——`gm_only` 事件、私設、世界書、卡片原檔、`raw`、變數表、狀態原文、錯誤回應本文一律不出那台電腦。
- 本文件與實作的測試樣本不得含真實暱稱、裝置碼、憑證或金鑰；測試一律用產生器。

## 11 v2／另案保留

- **伺服器內建執行端（v2）**：同一個「執行端」角色，跑在伺服器程序裡。核心照樣產生 `dispatch`、`abort`、`ctl`、`job.*`，它照樣回 `gen.*`、`job.*`、`capabilities`、`quota`、`ctl.result`、`roster.update`，所以房間與回合狀態機、客戶端協定、錯誤上送與遠端控制一字不改。它**用不到**：HTTP 建房、`hello`／執行端憑證／`hb`／`InflightReport`（同程序、無網路斷線）、`status.keepAwake`、`room.seed`（桌在伺服器上）、`pair.create`／`pair.claimed`（改由出額度者在手機上交金鑰，屬 v2 新增的 `key.submit`，金鑰只在房間存活期間保存、不落日誌、不給其他玩家——任務檔 Constraints）。核心只看到 `ExecutorPort` 介面（§12.1），桌面版走 WebSocket 轉接，v2 走程序內呼叫。
- **執行端交接（另案）**：v1 不做〔作者裁決 2026-11-02，裁決 8〕；位子留在 `scene.advance.next`（將來加 `executor` 欄）與 `executor.leave`（目前一律 `E_STATE_CONFLICT`），細節隨整桌匯出另案定。

## 12 給各包的實作提示

### 12.1 核心與平台殼的邊界（包 1、包 7 共用）

核心＝純 TS，只用語言內建、`TextEncoder` 與 WebCrypto（`crypto.subtle`、`crypto.getRandomValues`，Workers 與 Node ≥ 20 都有），不 import 任何 Workers 或 Node 模組。

```ts
// 純函式（可直接單元測試）
function validateFrame(raw: string, from: "member" | "executor"): { ok: true; frame: Frame } | { ok: false; code: ErrorCode };
function reduce(state: RoomState, input: CoreInput, ctx: { now: number; rng: () => Uint8Array }): CoreOutput;
function buildSnapshot(state: RoomState, events: StoredEvent[], viewer: Viewer): Snapshot;
function toWebSave(state: RoomState, events: StoredEvent[]): WebSave;
type CoreInput =
  | { kind: "frame"; conn: ConnInfo; frame: Frame }
  | { kind: "connect" | "disconnect"; conn: ConnInfo }
  | { kind: "alarm" }              // 倒數到期、執行端 hb 逾時、生成與換幕無進度、閒置清理都走同一個鬧鐘
  | { kind: "http"; route: "create" | "export"; body: unknown };
interface CoreOutput {
  state: RoomState;                // 新狀態（整份可序列化）
  events: Omit<StoredEvent, "seq">[]; // 殼配 seq、與 state 同交易落地
  sends: Array<{ to: ConnSelector; frame: Frame }>; // 落地成功後才送
  close: Array<{ conn: string; code: number }>;
  alarmAt: number | null;          // 下一次要叫醒的時間（取所有計時器最早者）
  deleteRoom?: true;
}
// 平台介面（殼實作）
interface RoomStore { load(): Promise<{ state: RoomState | null; tailSeq: number }>; commit(state: RoomState, events: StoredEvent[]): Promise<void>; eventsAfter(seq: number, limit: number): Promise<StoredEvent[]>; eventsBefore(seq: number, limit: number): Promise<StoredEvent[]>; deleteAll(): Promise<void> }
interface Connections { send(conn: string, text: string): void; close(conn: string, code: number): void; list(): ConnInfo[] }
interface Alarm { set(at: number | null): Promise<void> }
interface ExecutorPort { deliver(frame: Frame): void } // v1：轉成 WebSocket 送桌面版；v2：程序內執行端
```

殼的職責：
- **Workers 殼**：DO 用 SQLite storage 與 WebSocket Hibernation（連線以 `serializeAttachment` 記 `ConnInfo`），喚醒時 `load()` 重建狀態；串流緩衝在記憶體，被逐出就遺失（§4.3 已容許）。成員心跳設 `setWebSocketAutoResponse(ping→pong)`，喚醒時用自動回應時間戳清掉死連線；執行端 `hb` 走一般 `webSocketMessage`（§4.2）。
- **建房限流**：Worker 進入點沒有狀態，同 IP 每小時 10 次由殼用 Workers Rate Limiting binding 擋（超過回 HTTP 429＋`E_RATE_LIMITED`），核心不碰 IP。
- **房間碼產生**在 Worker 進入點：亂數產碼 → `idFromName` → DO 內檢查 `room` 表為空才建，否則重產。
- **Node 殼**（包 7）：`ws`＋SQLite 檔；**每房一條序列化佇列**，對應 DO 的單執行緒保證；鬧鐘用 `setTimeout`＋啟動時掃描；限流用程序內計數。

### 12.2 包 1（伺服器；目錄位置由包 1 照 STRUCTURE 定）

實作 §1–§10 的伺服器側全部：建房與匯出 HTTP、兩組訊息、回合狀態機、結算三條件（含空名單不結算）與倒數暫停、派工產生與去重、換幕工作（`jobId` 冪等、無進度標 `stalled`、只由 `job.result` 結束）、串流轉播與記憶體緩衝、重連補事件與快照、兩種心跳、錯誤碼、DO 落地、有損匯出（並在 `web-save.md` 補頂層鍵說明、用 `minimal.json` 形狀的卡殼過兩端 `parseWebSave`）、閒置清理。對接包 2 與包 4，兩者都還沒好時用核心單元測試＋假執行端（Node 腳本模擬 `gen.*`／`job.*`）驗。驗收對照：§3 每個轉移各一條測試；併發重點＝過期回合拒絕、同一回合只派一次、執行端重連時不重派（`InflightReport` 三種情況）、結算只發生一次（全員行動與房主結算同時到）、空名單不結算不派工、換幕中執行端離線後以同 `jobId` 收尾；配對碼暴力鎖；`ev` 一定在落地後才送（殼的整合測試）。故障注入只用在「`commit` 失敗時不廣播、狀態不前進」這一條資料遺失路徑。

### 12.3 包 2（桌面版執行端，`src/features/room/`＋Rust `room` 模組）

實作 §6 執行端側全部與 §7：建房、執行端憑證本機保存（退出／暫停只斷線不刪）、WebSocket 連線、`hb` 與退避重連、`room.seed`（公開名冊與公開歷史投影）、`capabilities`、派工 → 既有 GM 鏈 → `gen.*`、`action_id` 去重、`jobId` 冪等與本機重送佇列、中止、錯誤分類（§7.1 表左欄的桌面版錯誤碼對應）、額度回報、遠端控制、換幕摘要工作（`job.ack`／`job.progress`／`job.cancel`）、防休眠、面板（房間碼、QR、配對碼、重設房主、在線玩家、目前模型、最近錯誤、退出）。不改 GM 引擎；多玩家提示屬包 3。注意兩個現況：(1) 桌面版的 GM 鏈（`gm_narrate` → 寫點名列 → `chat_with_character` 的接力、`max_round_speakers` 上限）與正文落檔目前由前端 `src/features/play/chat/useChatController.ts` 驅動，不在 Rust；包 2 方案先實測視窗縮小／螢幕鎖定下前端接力是否照跑，不行再下移 Rust。(2) GM 的路線與模型目前是全域設定（`preferences` 的 `gm_tier`／`transport`），派工的 `settings` 用房間覆寫值、不寫回全域。驗收對照：拿包 1 的伺服器（本機 `wrangler dev` 或 Node 殼）＋一個腳本客戶端跑完「建房→配對→入座→三回合→斷網重連→換幕（含換幕中斷網）→退出再開」，比對房間事件與本機桌事件的段落一一對得上、沒有重複玩家事件、幕號一致。

### 12.4 包 4（手機客戶端 `party/`）

實作 §5 客戶端側全部：加入（連結解析、暱稱、`deviceId` 產生與保存）、選角（名冊、認領、自建）、回合介面（行動格、草稿上傳節流、送出／跳過／收回、別人已送出的行動即時顯示、倒數顯示、房主結算、觀望標示、GM 生成中可打字不可送）、串流顯示（delta 串接、定稿取代）、成員心跳、重連與快照、往回翻頁、出額度者面板（執行端狀態、額度、錯誤細節與 §7.1 動作鈕、`set_next`）、房主面板（設定、邀請重生、踢人、離席處置、換人裝置、換幕與卡住時取消、關房、匯出）、§8 每個錯誤碼的顯示。不做卡片介面（§13）。驗收對照：對假伺服器（包 1 核心＋Node 殼）跑兩支以上瀏覽器分頁完成一場三回合遊戲；錯誤碼逐一觸發看顯示；`E_ROUND_STALE` 時草稿不丟。

## 13 主線裁定與已知限制

主線已裁定（原「待主線確認」，2026-11-02，見交接檔）：
- 加入密鑰一份多人用、只在第一次加入時用、房主重生即作廢（§1.2）。
- 匯出照 §9.3 有損，不帶卡片原檔；`web-save.md` 補一段說明 `extensions.table_tavern_room` 頂層鍵。
- 房主結算時沒送的人草稿不自動送、算觀望；倒數到期才自動送草稿（§3.2）。
- 單房事件量不設上限，靠閒置清理。
- 幕設定的路線／模型用房間覆寫值、不寫回桌面版全域設定（§12.3 注意 2）。
- 包 2 方案先實測視窗縮小／螢幕鎖定下前端 GM 接力是否照跑（§12.3 注意 1）。

以下列已知限制：
- 手機端 v1 不顯示卡片介面（MVU 狀態面板）〔作者裁決 2026-11-02，裁決 10〕；派工與上送訊息都沒有 `raw`、變數表等為它留的欄位。
- 執行端永久失聯（電腦壞掉、硬碟沒了）時房間無法推進，只能匯出；桌面版按「退出／暫停」不算失聯（§7.4）。
- 執行端程序重啟後，`InflightReport` 報 `running` 的派工要等 15 分鐘逾時才進 `held`。
- `held` 之後晚到的 `gen.part.end` 丟掉（`E_DISPATCH_UNKNOWN`）。
- 去重以派工為單位：玩家事件落一半就斷，重來時會跳過剩下的行動。
- 倒數暫停與恢復沒有專屬事件（包 1 順手可加 `round.deadline`）。
- 封鎖只記裝置雜湊，清掉瀏覽器資料就能用新連結回來（房主可再重生邀請）。
- 配對碼錯 5 次是全房鎖，惡意成員可以讓出額度者 10 分鐘配不了對。
