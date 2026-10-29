# vn-prestructure-split 拆法（作者 2026-10-29 確認）

交接：[handoffs/vn-prestructure-split.md](../handoffs/vn-prestructure-split.md)。純結構搬遷，原封搬移、不改寫；行數為估計。

## 包 1：測試模組搬出（三檔，零邏輯）
`#[cfg(test)] mod tests { … }` 原封搬進同名子資料夾的 `tests.rs`，原處留 `#[cfg(test)] mod tests;`（沿用 `response.rs`＋`response/tests.rs` 慣例，不必 `#[path]`）。

| 檔 | 前 | 後（本體） | 測試落點 |
|---|---|---|---|
| `data/scene/transcript.rs` | 1300 | ~730 | `transcript/tests.rs`（資料夾已存在） |
| `transport/turns.rs` | 1149 | ~620 | `transport/turns/tests.rs` |
| `transport/context.rs` | 1018 | ~455 | `transport/context/tests.rs` |

chat.rs 的測試不在包 1 搬，跟包 2 一起依新模組分（免得搬兩次）。

## 包 2：commands/chat.rs 業務邏輯搬到 domain 模組 `chat_turn`
新 Rust 模組 `src-tauri/src/chat_turn.rs`＋`chat_turn/`（與既有 `chat_assembly` 並列：assembly 管「送什麼」，chat_turn 管「一個回合怎麼跑、怎麼收」）。VN v1 的劇本格式解析、選項與 VN 回合變體會以同層新檔加入。

| 新檔 | 內容（原 chat.rs 行號） | 估計 |
|---|---|---|
| `commands/chat.rs` | 三個 `#[tauri::command]`（chat_with_character／gm_narrate／chat_abort）薄殼，簽名與參數名不變、轉呼叫 chat_turn；lib.rs handler 路徑不變 | ~50 |
| `chat_turn.rs` | mod 宣告＋對外 re-export | ~20 |
| `chat_turn/character.rs` | ChatReply、character_reply、角色回合本體（13–34、86–301） | ~250 |
| `chat_turn/gm.rs` | GmNarration、StateUpdate、gm_lane_reply、GM 回合本體（327–386、495–521、523–810） | ~390 |
| `chat_turn/gm_reply.rs` | GmReplyParts、gm_reply_parts、aborted_narration（388–452） | ~75 |
| `chat_turn/commit.rs` | gm_commit_apply、TurnGuard、state_update_lines／side（454–493、812–944） | ~185 |
| `chat_turn/stream.rs` | Spoken、push_delta、take_delta、take_abort_or_finish、prepare_play_lane_call（36–80、1105–1128） | ~80 |
| `chat_turn/arrivals.rs` | load_hidden_cards、append_arrival、record_person_arrivals、record_card_arrivals（303–325、952–1103）——**待裁決，見下** | ~185 |
| 測試 710 行 | 依受測對象分到 `chat_turn/` 各模組的 tests（回覆剝殼／提交、登場、GM 提示詞傾印各自跟著住），總數不變 | ~710 |

連帶改動：可見性（`pub(super)`／`pub(crate)`）、use、`#[cfg(test)] use gm_turn_instruction` 跟著測試走、`scene_budget/measure.rs` 與 `lanes/tests/opening_raw.rs` 兩處文件註解的路徑。

**在場規則待裁決**：BACKLOG 有〔作者裁決 2026-10-27〕「在場規則搬出 chat.rs 跟著 character-presence ④做」。登場紀錄（上表 arrivals 一列）屬在場規則。
- 定案 A：本包原封搬進 `chat_turn/arrivals.rs`，規則不改；搬位置提前於本案做〔作者裁決 2026-10-29〕，character-presence 之後在新位置改規則。
- 包 2 各檔行數若跟上表差很多（例如某檔超過 500），先回報主線再繼續〔作者裁決 2026-10-29〕。

## 本輪不動〔作者裁決 2026-10-29〕（App.tsx、apply.rs、response.rs、transport/ 分組；transcript 開場 checkpoint 也不抽）
- `transport/response.rs` 728：已在線內、測試已在子資料夾。
- `refactor/apply.rs` 1001：肥的是單一函式 `apply_into`（81–674 約 590 行），拆檔救不了，拆函式屬邏輯改寫，超出本案 0 行預算；不在 VN 路徑。
- `src/App.tsx` 1091：前案 [app-tsx-split](app-tsx-split.md) 已把切桌 hydration、撤銷匯入、重構後刷新、範例桌重生等跨域協調留在 root，剩的就是這些；再抽桌次生命週期 hook 要傳十幾個 setter、改 `check-structure.mjs` 外殼清單，且 VN 開桌型會再碰 enterTable，等 VN v1 真要動時一起做。
- `src/features/play/` 頂層 16 項、`chat/useChatController.ts` 905：都在線內。
- `transport/` 頂層 26 支：職責混了提示詞組裝、回覆處理、傳輸三類，但重分組要改大量內部 `super::` 路徑、零行為價值；VN 回合邏輯放 chat_turn 後 transport 不會因 VN 明顯變肥。

## 包 1 結果
transcript.rs 1300→729（tests.rs 569）、turns.rs 1149→620（tests.rs 527）、context.rs 1018→455（tests.rs 563）。`cargo test -- --list` 前後皆 2491 且清單逐行相同；搬出內容去空白與 rustfmt 尾逗號後雜湊一致；verify 13 步綠。

## 觀察
- 範圍外但超過千行：`data/world_file.rs` 1415、`smart_free/mod.rs` 1250、`smart_free/failover.rs` 1035、`lanes/mod.rs` 1014，皆不在 VN 路徑。
