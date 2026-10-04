# quota-insufficient-alert — 計畫

範圍見 [handoffs/archive/quota-insufficient-alert.md](../handoffs/archive/quota-insufficient-alert.md)。

## 結論
2026-10-03 結案進 main：Sol 四輪計畫審查、兩輪驗收同意；verify 全綠（vitest 699、cargo 820、harness 28）。下方「已知限制」照舊成立；實機項目排在[實測佇列](../reference/verification-queue.md)梯 1。

## 真實流程（現況）

- 打字送出：`PlayView` 表單 → `App.lockedChat.send`（`whenTableFree`，只查桌次鎖）→ `useChatController.send` → `submitText(input)`：`setInput("")` → `appendEvent(玩家句)`（`append_transcript` 落檔、進畫面、清收回疊）→ `narrateOnce`（`gm_narrate`）或 `replyOnce`（`chat_with_character`）→ 回覆經前端 `appendEvent` 落檔 → `refreshWorlds`。
- `gm_narrate` 成功路徑在後端就先寫檯面、追加人物登場／角色回歸事件，之後才由前端追加旁白與狀態更新事件。「玩家句之後前端沒再落檔」不代表逐字稿尾筆還是玩家句。
- `append_transcript` 經 `commit_world_append` 直接 append，失敗時可能留下半行。
- `pop_transcript`／`remove_transcript_event`／`set_last_transcript_state`／`sync_scene_state_tree`（經 `rewrite_scene`）與 `lifecycle.rs` 的退回上一幕、改寫摘要，都是「讀逐字稿→整檔寫回或刪檔」，讀與寫之間沒有互斥。
- `world_write_permit` 是桌鎖的讀鎖，多個寫入命令可同時持有，不是同檔寫入互斥；`read_transcript` 無鎖，遇到任何壞行（含半行）整幕報錯。
- 收回／復原有自己的 `undoBusy`，回合入口只看 `busyRef`，兩者不互斥。
- 輸入框 `disabled` 吃 `busy`（`generating !== null`），玩家句落檔等待期間 `generating` 還沒設，框可編輯。
- 失敗：四個回合入口的 catch 都呼叫 `onError` ＝ App 的 `setError`，顯示成 `ErrorNote`。串流殘句在 `finally` 清空、不落檔。
- 續聊線：呼叫前就把 `pending_rewrite` 寫進線狀態，失敗不清，下一輪 `plan_turn` 判 `PendingRewrite` 重開線；逐字稿被收短判 `HistoryRewound`、已送段被改判 `HistoryEdited`、上輪回覆對不上判 `ReplyDiverged`，都重開線。收回玩家句不需要另做清線流程。
- 生圖：`CardImageDialogs` 失敗時在對話框內顯示 `explainAiError(..) ?? aiGenFailed` ＋原文小字，已是同套文案。

## 做法

### 1. 聊天失敗改攔截式彈窗

- `useChatController` 多一個參數 `onTurnFailed({ raw, draft? })`；四個回合入口（`submitText`、`requestReply`、`gmNarrate`、`gmAdvance`）的 catch 改呼叫它，開始時仍 `onError("")` 清舊錯誤列。`postOpening`／`undoLast`／`restoreUndone` 維持走 `onError`。
- 回合 catch 不另分「AI 失敗」與「寫檔失敗」：回合沒完成一律進彈窗，標題用中性的「這一輪沒能完成」，內文沿用分流；分流認不出時只顯示原文（同 `ErrorNote`）。〔模型判斷·未裁決〕
- `draft`＝打字送出但沒有自動收回時，玩家剛送出的原文。彈窗多一段「你剛送出的內容」：可選取的唯讀文字區塊，關掉彈窗前都拿得到。
- 彈窗還開著且帶 `draft` 時，後續失敗不覆寫它（保留原本那則）；沒有 `draft` 的彈窗照常被新的失敗取代。失敗資訊先交給彈窗，之後才做可能失敗的重讀。
- 新元件 `src/features/play/TurnFailedDialog.tsx`：共用 `Dialog`，Esc 可關、點遮罩不關，底部只有「關閉」（沿用 `closeBtn`，`data-autofocus`）。內文抽出 `atoms.tsx` 的 `AiErrorText`（人話＋原文小字；`ErrorNote` 改用它）。
- App 持有 `turnFailure` state，`AppDialogs` 渲染彈窗（App 層，卡片覆蓋層開著時一樣蓋在上面）。
- 生圖：對話框內已用同套文案，不改。〔模型判斷·未裁決〕

### 2. 打字送出失敗：只在乾淨路徑自動收回

原則：自動收回只走乾淨路徑；任何不確定一律不收回、不還原輸入框，原文改放在彈窗裡讓玩家複製。〔模型判斷·未裁決〕

**乾淨路徑**＝玩家句追加成功並拿到收據（該行起始位元組），之後回覆失敗、世代未變。

**後端：同檔鎖（`world_file.rs`）**
- 保證範圍：同一行程內；正式路徑由固定 root＋受控名稱組成，以該組成路徑為鍵，不做 canonicalize。〔模型判斷·未裁決〕
- 全域 `Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>`，`lock_world_file(path)` 回 guard。`commit_world_write`／`append`／`remove` 內部各拿一次；另提供吃 guard 的 `*_locked` 版本給讀改寫路徑用。鎖順序固定：先 `world_write_permit`（讀鎖）再檔案鎖；檔案鎖只在同步程式內短暫持有，不跨 await、不巢狀拿第二把、持有期間不拿 permit。
- `commit_world_append`（鎖內）：檔尾非空且最後一個位元組不是換行 → 先補一個換行（避免兩筆黏成一行）；記長度 L、寫入；失敗就盡力截回 L，截回也失敗就放著，回原本的錯；成功回 L＝收據。
- 逐字稿讀改寫路徑（`rewrite_scene` 系列：收回、復原匯入收開場白、改末筆快照、補樹；`lifecycle.rs` 的退回上一幕、改寫摘要）改成在逐字稿的檔案鎖內「讀→算→提交」。只做逐字稿這個檔，其他檔不擴。

**後端：讀者（`transcript.rs`）**
- 鎖內一律用 guard 上的讀寫方法，不重進會取鎖的 `read_transcript`／`commit_world_*`；檯面回捲與前幕快照讀取在放開逐字稿鎖之後做。
- `read_transcript` 在檔案鎖內一次讀整檔 bytes、鎖外解析；最後一行沒有換行且解析失敗就略過，其他位置的壞行照舊報錯。最後一行沒換行但 JSON 完整（截回失敗留下的完整事件）照常讀進來，當已持久化事件。其他讀逐字稿的地方都經 `read_transcript`，不另改。

**後端：命令**
- `data::append_transcript` 改回傳收據；新命令 `append_player_event` 回 `{ event, offset }`，只給玩家句用（既有 `append_transcript` 命令形狀不變）。
- 新命令 `discard_unanswered_player(world_id, scene, offset, ts, text) -> bool`，持 permit，在逐字稿檔案鎖內一次做完：讀整檔 → `offset` 必須是行首、從 `offset` 到檔尾恰為最後一行且以換行結尾、解析後 `kind=player` 且 `ts`、`text` 相符 → `set_len(offset)`。不符回 `false` 不動。`set_len` 回錯就在同一鎖內量實際長度：等於 `offset`＝已刪（回 `true`），其他（含量長度也失敗）一律回 `false`。回 `true` 後檯面同步成剩下事件的最後快照（沿用 `rewrite_scene` 找快照的邏輯抽共用），同步失敗不回錯——權威在逐字稿。

**前端（`useChatController`）**
- 單一互斥：`undoLast`／`restoreUndone` 執行期間也佔 `busyRef`，與送出、連點彼此擋住。收回玩家句在回合的 catch 內完成，`finally` 才放開。
- 換桌／換幕世代：世代計數 ref，`worldId` 或 `scene` 一變就 +1；每個回合入口記下發起世代，一路傳給 `replyOnce`／`narrateOnce`／`appendEvent`（helper 不自行認領目前世代），串流 callback、更新畫面、刷新檯面、登場併入前都核對；世代已變就不再發新的 AI 呼叫（玩家句落檔後、接力下一步前），已在路上的回覆照樣落進原桌但不碰現在的畫面。收回／復原 invoke 回來後同樣核對。世代核對也放進 helper 內部：`reload`、`appendEvent` 等會 `setEvents` 的 helper 在 invoke 返回後、更新畫面前核對發起時的世代。收回已送出後才換桌：只保證不再發新收回、不更新新桌、彈窗原文保住，原桌那句可能已被收回。
- `submitText` 內部拆成 `submitTurn(raw, fromComposer)`；`send` 走 `fromComposer=true`，卡片介面的 `submitText` 走 `false`，只有 composer 路徑清輸入框。
- composer 送出一開始就鎖輸入框：`sending` state，`busy = generating !== null || sending`。
- 比對與還原分開：落檔與收回比對用 `raw.trim()`；輸入框還原用原始 `raw`。
- 畫面移除只認本次追加放進畫面的那一個物件（物件身分）。
- 失敗處理（composer 路徑）：
  - 乾淨路徑 → `discard_unanswered_player`；`true` → 畫面移掉那個物件、輸入框寫回原始 `raw`、`refreshState()`（刷新失敗走 `onError`，不影響已還原的原句），彈窗不帶 `draft`。
  - 其他所有情況（玩家句追加回錯、收回回 `false` 或錯、世代變了）→ 不收回、不還原輸入框，彈窗帶 `draft=raw`。
  - 任何追加回錯之後（世代未變）用既有 `reload()` 重讀這一幕同步畫面；重讀失敗只走 `onError`。不用事件索引猜位置。
- 卡片介面送出的失敗：玩家句留在逐字稿、不收回、彈窗不帶 `draft`（原文在卡片自己那邊）。〔模型判斷·未裁決〕
- 收回的玩家句不放進「復原」疊；玩家句落檔時已清掉的舊收回疊不救回。〔模型判斷·未裁決〕
- 中止：只有後端正常回傳 `aborted` 才不算失敗；無輸出時玩家句照舊留著，有本文照舊落 `truncated`。中止之後的刷新錯誤照樣進彈窗。〔模型判斷·未裁決〕
- 按鈕觸發的動作（請 X 發言、GM 旁白、GM 推進）只換成彈窗，不做收回／還原。

## 已知限制

- 同檔鎖不 canonicalize：跨行程、symlink 別名不保證共鎖。〔模型判斷·未裁決〕
- 追加失敗且截回也失敗時留下的完整 JSON 行，視為已持久化事件。〔模型判斷·未裁決〕
- 無效殘段只補換行，下一次追加後它成為中段壞行，`reload` 會報錯；玩家原文仍能從彈窗取回。〔模型判斷·未裁決〕
- 模糊時保留玩家句，玩家照彈窗原文重送可能出現重複句（可自行收回）。〔模型判斷·未裁決〕
- 關掉彈窗後不保存 `draft`。〔模型判斷·未裁決〕

## 改哪些檔

- `src-tauri/src/data/world_file.rs`：同檔鎖、`*_locked`、追加補換行／失敗截回／收據。
- `src-tauri/src/data/scene/transcript.rs`：鎖內讀、讀者略過殘段、追加收據、讀改寫進鎖、`discard_unanswered_player`、快照回捲抽共用。
- `src-tauri/src/data/scene/lifecycle.rs`：退回上一幕、改寫摘要的讀改寫進鎖。
- `src-tauri/src/data/mod.rs`、`src-tauri/src/commands/scene.rs`、`src-tauri/src/lib.rs`：匯出與兩支新命令。
- `src/shared/contracts/backend-contracts.ts`：收據型別 `PlayerAppend`。
- `src/features/play/useChatController.ts`、`src/features/play/TurnFailedDialog.tsx`（新，含 `nextTurnFailure`）、`src/shared/ui/atoms.tsx`、`src/App.tsx`、`src/views/AppDialogs.tsx`、`src/styles/dialogs.css`。
- `src/i18n/*.ts` 十語系：`turnFailedTitle`、`turnFailedDraftLabel`。
- 既有用到 `useChatController` 的測試補新參數。

## 測試清單

**Rust**（擴充 `world_file.rs` 既有注入點；新增截檔失敗注入，不拿不可寫桌代替）
1. 追加半行失敗 → 截回原長度、照讀；截回也失敗 → 回錯、讀者略過殘段。
2. 前一行缺換行但 JSON 完整 → 讀者讀進來；下一次追加先補換行、兩筆各自成行。
3. 兩個追加者交錯：A 半行失敗截回、B 成功，B 完整保留（逐字稿與機制紀錄各一）。
4. 讀改寫（收回上一句）與追加交錯：不丟追加的事件。
5. 讀者在追加與截回之間：鎖內讀不會讀到半行；檔案已有殘段時略過、不報錯；中間壞行照舊報錯。
6. `discard_unanswered_player`：相符 → 截掉、前面位元組不變、檯面回到前一則快照、`true`；尾筆是登場／回歸／旁白 → `false` 不動；同 `ts` 同文舊句在前 → 只刪尾筆；offset 非行首、非最後一行、`kind`／`text` 不符、空檔 → `false`。
7. 截檔失敗注入：實際長度等於 offset → `true`；不等於 → `false`、檔案不變。截成功但檯面寫入失敗注入 → 仍 `true`。
8. 既有開場白「部分追加＋回復失敗」測試保留並照新行為調整預期。
9. `plan_turn`：失敗那輪留下 `pending_rewrite`、收回後重送同文 → 重開線；線水位已含玩家句、收回後 → `HistoryRewound`；收回後重送不同文 → 重開線；失敗發生在寫線狀態之前、收回後重送同文 → 照常續聊、水位不變。

**前端**（`src/features/play/chat-send-failure.test.tsx`）
1. 乾淨路徑（角色、GM 各一）：`discard` 帶收據、`ts`、`raw.trim()`；畫面移掉那個物件、輸入框回到原始 `raw`、`refreshState` 被叫、彈窗收到原錯誤且不帶 `draft`、`onError` 沒收到它。
2. 串流已有 delta 後失敗 → 殘句不落檔、照常收回。
3. 收回 `true` 但 `refreshState` 失敗 → 輸入框照樣還原。
4. 收回回 `false`／回錯 → 不還原、彈窗帶 `draft`。
5. 玩家句追加回錯 → 不收回、不還原、彈窗帶 `draft`、`reload` 被叫；`reload` 失敗 → `onError`。
6. GM 旁白已落檔、之後失敗 → 收回回 `false` 不動、彈窗帶 `draft`。
7. 畫面上有同 `ts` 同文的舊句 → 只移掉本次那個物件。
8. 世代：追加等待中換桌、回覆等待中換幕、換走又換回 → 不碰畫面與輸入框、不發收回、彈窗帶 `draft`；收回已送出後換桌 → 不更新新桌、彈窗帶 `draft`（不宣稱原桌沒被收回）；`reload` 等待中換桌／換走又換回 → 不覆蓋新桌逐字稿。
13. 帶 `draft` 的彈窗未關時再失敗 → 原 `draft` 保留。
9. 中止且無輸出 → 不收回、不叫 `onTurnFailed`；中止有本文 → 落 `truncated`、不收回。
10. 卡片介面 `submitText` 失敗 → 不收回、輸入框不動、彈窗不帶 `draft`；卡片送出不清輸入框。
11. `requestReply`／`gmNarrate`／`gmAdvance` 失敗 → 走 `onTurnFailed`，不收回。
12. 互斥：收回／復原等待中送出被擋；連點送出只送一次；送出期間 `busy` 為真。

`TurnFailedDialog.test.tsx`：人話＋原文小字、認不出只顯示原文；有 `draft` 時顯示唯讀原文；按「關閉」與 Esc 關、點遮罩不關、開窗焦點在「關閉」。

`npm run verify` 全綠（含 check-i18n）。不打真 AI、不啟動正式 app。
