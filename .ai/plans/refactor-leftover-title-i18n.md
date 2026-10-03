# refactor-leftover-title-i18n — 計畫

已結案（2026-10-03）：Sol 審查與驗收同意，verify 全綠（vitest 658、cargo 778、harness 28）。

## 查證結論
- **後端取語言的既有做法**：`transport::ui_language(&config)`（`src-tauri/src/transport/client.rs:16`，讀 `preferences.language`、預設 zh-TW），command 層讀 config 後把 `lang: &str` 往下傳（如 `commands/refactor.rs:56`）。依語系出固定字的既有慣例是 `match lang` 回 `&'static str`、`zh*` 退繁中、其他退英文（`transport/messages.rs:28` `player_fallback_name`）。沿用。
- **沒有以餘段後綴作為特殊識別**：標題本身有參與比對——`assemble_splits` 依 title 合組（route=entry）與判斷撞名（`used_titles`），世界書匯入指紋含標題（`data/worldbook.rs:697`）。所以不同語系各自產出的同內容條目，重新匯入時可能保留兩份，這跟一般改名後的既有行為相同，不擋本案。套用走 `upsert_worldbook_entry` 一律新 uid（`refactor/apply.rs:252`）。帳本只記 `locked` 條目（`apply.rs:283`），餘段 `kind="setting"`、不帶 rules／triggers，不會 locked，不進帳本；`meta=None` 只影響預設欄位與排序。前端重構流程沒有依標題比對。
- **舊桌**：已落世界書的「X（餘段）」條目就是普通條目，重跑照一般條目盤點。舊 `refactor-outcome.json` 的 `refactor_span_leftover` 沒有 `title` 參數，前端照既有規則原文顯示；依發佈前舊桌可不相容的原則，主線指示不做退路。
- **會進 prompt**：套用後餘段條目跟其他條目一樣，標題會出現在下次重構盤點脈絡（`refactor_ai/context.rs:151` `#### uid=… 標題`）與聊天世界書段落（`transport/context.rs:63`，僅在條目被注入時）。標題只是條目名；重構盤點本來就要求 title 用介面語言（`refactor_ai/survey.rs:158`），聊天也有 `language_rule`，標題跟著介面語言一致。「後綴語言與卡片內容不同不影響模型判讀」是推論，未實證。audit detail 不進 prompt。

## 做法
1. `refactor_assemble.rs` 新增 `fn leftover_title_suffix(lang: &str) -> &'static str`，照 `player_fallback_name` 的 match 形狀，用詞對齊各語系既有 `be_refactor_drop_rule_leftover` 的「餘段」譯法：
   zh-TW `（餘段）`、zh-CN `（余段）`、ja `（残り段落）`、ko ` (남은 단락)`、en ` (leftover)`、es ` (sobrantes)`、pt-BR ` (sobras)`、de ` (Rest)`、fr ` (restes)`、ru ` (остатки)`；`zh*` 退繁中、其餘退英文。
2. `assemble_local` 加參數 `lang: &str` 傳到 `assemble_splits`；每個來源條目的餘段標題先算一次，同一個值給產物與該條每筆 audit。
3. `commands/refactor.rs` `refactor_assemble_local` 讀 config 取 `ui_language` 傳入（前端 invoke 參數不變）。
4. 訊息對齊：`UiMsg::RefactorSpanLeftover` 加 `title: String`（完整餘段標題），十語系 `be_refactor_span_leftover` 改成引用 `{title}`，`backend-msg.ts` 參數表補 `title: "string"`。理由：訊息直接帶實際標題，產出後切換語系也對得上，後綴只有 Rust 一份來源（Sol 審查同意）。玩家之後若改條目名，audit 仍是產生當時的快照。`ai_text` 英文句同步帶 title。
5. 與 `backend-zh-data-text`（存代碼、顯示時翻譯）的差別：餘段標題是玩家可改的世界書條目名，不是固定段標，產生時照當下介面語言寫入即可，不存代碼（Sol 審查同意，與那 13 處分開看）。
6. 順手：更新 `refactor_assemble.rs` 模組註解、`ui_msg.rs` 變體註解、`backend-msg-notes.ts` 第 4 行「照抄（餘段）」註解。

## 改動清單
- `src-tauri/src/refactor_assemble.rs`、`src-tauri/src/refactor_assemble/tests.rs`
- `src-tauri/src/commands/refactor.rs`
- `src-tauri/src/ui_msg.rs`
- `src/i18n/features/backend-msg.ts`、`src/i18n/features/backend-msg-notes.ts`、`src/shared/ui/backend-text.test.tsx`

## 測試清單
- Rust（`refactor_assemble/tests.rs`）：
  - 既有 `tests.rs:202`、`:323` 以 `"zh-TW"` 呼叫，仍找「…（餘段）」。
  - 後綴逐一比對十語系預期字串（含空格與全半形括號）、zh-CN、zh-HK 退繁中、未知與空字串退英文。
  - 四段只路由一段：en 下產物標題「條目A (leftover)」、三筆 audit 依序且 detail 全等於帶同一 title 的訊息；內容、順序、source_uids 與 zh-TW 結果相同。
  - 全部歸位：不新增餘段、不新增 audit。
  - `UiMsg::RefactorSpanLeftover` JSON 序列化讀回相等，`ai_text` 帶完整 title（含引號、反斜線、`{name}`、`TTMSG:`）。
  - en 產物經 `refactor::apply` 套用後讀回世界書與 `assemble_card_context`：英文標題保留、內容不變。
- 前端（`backend-text.test.tsx`）：新訊息解碼；切 ru 後仍引用原標題；標題含引號、反斜線、`{name}`、`TTMSG:` 原樣代入；舊存檔缺 title 整段原文。
- `npm run verify` 全綠。

## 未涵蓋
- 餘段標題與世界書既有同名條目撞名不處理（現狀行為，與本案無關）。
