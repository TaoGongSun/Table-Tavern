# room-cleanup-pre-stage3 方案

座位代理包 3 開工前整理三條（健康度檢查）。作者原話：「先整理再開工」。

## 與 room-seat-delegate 的衝突

第 1、2 條會碰到 seat-delegate 改過的五個檔：

| 檔案 | seat-delegate 改的地方 | 本案改的地方 | 衝突 |
|---|---|---|---|
| `src/features/room/DesktopRoom.tsx` | 「最近錯誤」那行加 `ROOM_PAUSED_REMOTELY` 特判（它的基準早於 `LastError` 元件） | `LastError` 改吃 kind、import 路徑 | 同一行直接撞；它 rebase 到 main 本來就得手解 |
| `src/features/room/useDesktopRoom.ts` | 第 5 行 import | 第 7 行 import 路徑 | 隔一行，應可自動合併 |
| `src-tauri/src/room.rs`、`room/run.rs`、`room/tests.rs` | `stop_reason` 旁常數、`exit_world`、`land_batch`、`on_ctl`、遠端控制測試 | `Status`、`note_error`、`classify`、`chain` Err 分支、錯誤分類測試 | 不同區塊 |

順序：本案先進 main，seat-delegate 再 rebase〔模型判斷·未裁決；主線裁定〕。`ROOM_PAUSED_REMOTELY` 屬沒有 kind 的錯誤，rebase 時在 `LastError` 前加特判即可。協定 `party/server/core/types.ts` 不動。

## 第 1 條：GM 錯誤分類統一

### 根因
- 分類有兩份在用：Rust `room/run.rs` `classify`（手機「GM 卡住」與重生灰掉模型看它）、TS `gm-error.ts` 的 `KINDS` regex（只給桌面「最近錯誤」）。
- GM／角色回合錯誤經 `chat_turn/gm.rs`、`character.rs` 的 `ai_call_failure` 包成 `AI_CALL_FAILED: TTMSG:{...}`，`classify` 只看外層，模型下架、缺金鑰、CLI 太舊都落 `provider_unreachable`。
- `note_error` 多數呼叫點不是 GM 錯誤，卻在桌面都顯示成「GM 連線失敗，錯誤碼 X」。

### 做法（已實作）
- `classify` 拆成 `code_of`＋`kind_of`；外層是 `AI_CALL_FAILED` 且後面緊接 `TTMSG:` 時用內層的碼分，內層落 other 才退回外層（比照前端 `ai-error.ts`）。
- `Status.last_error_kind`（`lastErrorKind`）只由 `note_gm_error`（`gen.failed`）設，`note_error` 清成 None。
- `gm-error.ts` 刪 `KINDS`、`GmErrorKind`、`holdKind`；文字直接以 `ExecutorErrorKind` 查，沒 kind 走「房間出了問題」；`modelBroken` 吃 `model_unavailable | not_ready`。

### 歸類（改後）〔模型判斷·未裁決；常識審查員〕

| kind | 碼 | 手機（不改字） | 桌面 |
|---|---|---|---|
| refusal | `AI_CONTENT_FILTERED` | 出額度者：被供應商擋下，你可以重試、換模型或跳過；其他人：被供應商擋下，等房主處理 | GM 這段被供應商擋下：換個模型重試，或跳過這段 |
| quota | `AI_HTTP_STATUS_429`／`402`、`AI_FREE_MODEL_BUSY` | 額度用完 | 額度或頻率撞到上限（不變） |
| scene_full | `scene_capacity_full`、`AI_CONTEXT_TOO_LONG` | 這一幕滿了 | 這一幕太長了：換幕後再繼續 |
| model_unavailable | `AI_HTTP_STATUS_404`、`cli_model_unavailable`（含包殼） | 模型已下架 | 已下架（不變） |
| not_ready | `AI_HTTP_STATUS_401`／`403`、`cli_not_found`、`cli_risk_not_accepted`、`openrouter_api_key_missing`、`codex_auth_missing`（含包殼） | 金鑰或登入失效 | 金鑰或登入失效（不變） |
| tool_format | `cli_too_old*`、`cli_output_unrecognized*`、`cli_reply_error`、`cli_no_reply`（含包殼） | GM 主機需要處理 | GM 的工具需要處理：去設定看看 |
| provider_unreachable | `AI_CALL_FAILED`（內層認不出）、`AI_STREAM_STALLED`、其餘 `AI_HTTP_STATUS_*`、`AI_INCOMPLETE_RESPONSE`、`AI_OUTPUT_RUNAWAY`、`AI_EMPTY_RESPONSE`、`cli_updating`、`cli_stalled*`、`cli_crashed`、`cli_turn_failed` | 連線不穩 | GM 連線失敗或逾時，可以直接重試 |
| other | 其他 GM 錯誤 | GM 出錯了 | GM 出錯了，錯誤碼 {code} |
| （沒有 kind） | 房間連不上、名冊、種子、同步、伺服器 err 碼、關房原因 | — | 房間出了問題，錯誤碼 {code} |

岔開的碼：`AI_STREAM_STALLED` 歸 provider_unreachable（協定只有 8 種，動作同為重試／換路線）；`openrouter_api_key_missing` 歸 not_ready（缺金鑰重試沒用）；回應半截／失控／空白與 `cli_updating` 從 tool_format 改歸 provider_unreachable（重試就會好，不叫人去設定）；`E_GEN_TIMEOUT` 是伺服器端判的、不進桌面，TS 那筆是死碼刪掉。手機的變化只在剝殼、缺金鑰、改歸類那幾列；剝殼後包殼的下架／金鑰類錯誤重生時也會灰掉原模型。句子不點名不一定在畫面上的按鈕（第 1 回合只有「跳過這段，繼續」；「換幕」在「房主」分頁）。held 時輸入區提示改「GM 卡住了，房主處理後再送（可以先打字）」；其他 held 種類的回合列原本就寫「等房主…」，只有被擋下那句分出額度者版。

### 已知限制
- `AI_HTTP_STATUS_403` 維持 not_ready：也可能是地區或內容被擋，這時「金鑰或登入失效」會誤導〔模型判斷·未裁決；常識審查員〕。

## 觀察
- 把重試／換模型鈕搬到回合列：進 room-desktop-layout。

## 第 2 條：party/src/App.tsx 不當函式庫用（已實作）

`useRoomSession`＋`RoomSession` 型別搬到 `party/src/features/room/useRoomSession.ts`，`RoomBody`＋`DesktopHooks` 搬到 `party/src/features/room/RoomBody.tsx`，`App.tsx` 只留 `Room`、`App`；桌面版三支檔改 import。`@party/App.css` 留根層。原樣搬、不改行為。

## 第 3 條：平台殼路由——不做〔模型判斷·未裁決；主線裁定〕

真能共用的只有路徑比對＋404／405 約 10 行，建房重試兩殼呼叫方式不同，抽出來核心反而淨增；路由又是信任邊界輸入，要另做方案送審。維持 `node/http.ts` 檔頭的取捨。

## 驗收
- `npm run verify`、party 測試；測試通道截桌面房主「最近錯誤」一列。
