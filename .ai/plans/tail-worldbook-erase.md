# tail-worldbook-erase 方案

## 一、裁決
- GM 續聊線與角色共線的尾段世界書（本輪觸發的 keyword、機率、計時、作者註記、依深度等）回合後從 session 歷史抹掉，不再每輪疊一份；system 不動、維持快取；作者註記與依深度照 P2 留在回合尾段〔作者裁決 2026-10-11〕。
- 不在此列：Agy 一角一線、單人在場 Claude（`Hoist::All`，世界書已全在 system、變了就重開）；API／codex 單發（無狀態）。

## 二、現況（掛點）
- **抹寫機制**（`lanes/mod.rs` `run_turn`，回合完成後）：
  - Claude：`apply_rewrite`（`:785`）用 `TurnInput.confidential` 在 session jsonl 找「恰好一則 user 行、恰好出現一次」的子字串並刪掉（`session_file::erase_user_segment`），再補前綴，原子寫＋回讀。`confidential` 與 `prefix` 都是 None 時不讀檔。
  - Grok：`rewrite_grok` → `grok_session::rewrite`，兩檔（`chat_history.jsonl`、`updates.jsonl`）抹同一段、拿掉 reasoning、補前綴，寫完掃整個 session 目錄不得殘留。現在只在 `Lane::Chars` 呼叫（`:1376`），GM 線不碰。
  - 失敗：Grok 撤線刪目錄；Claude 記 `rewrite-failed`、`store.remove`，下一輪重開。中止／失控／空回覆走 `settle_abort`（Claude 照抹、失敗刪檔；Grok 撤線）。呼叫前先落 `pending_rewrite`，中途崩潰下一輪 `PendingRewrite` 重開。
  - Agy：沒有抹寫路徑；`run_turn` 開頭看到 `confidential`／`prefix` 就整輪回錯（`:1028`）。
- **角色共線尾段**（`transport/turns.rs` `chars_lane_turn` `:231`，`Hoist::None`）：`public`（本輪公開觸發、不在共用快照的條目，含作者註記／依深度注入段）＋ `confidential`（動態公開設定、私設、限定與私密觸發條目、角色狀態）＋本輪指定。`public` 不在 `confidential` 裡，所以不抹、每輪疊一份。兩段在 tail 裡本來就前後相接（`public`＋`\n`＋`confidential`＋`\n`）。
- **GM 線尾段**（`gm_lane_turn` `:369`）：`gm_dynamic_block`（`state_view.rs:14`）＝世界書段（段標＋不穩定條目＋注入段）＋目前狀態＋觸發器，接 `\n\n` 與導演指示；`confidential: None`。`chat_assembly::gm_lane_parts`（`:296`）只回 `(system, tail)`；呼叫端 `commands/chat.rs` `gm_lane_reply`（`:322`，narrate 與 suggest 共用）傳 `confidential: None`。三家 GM 線都不抹。

## 三、做法

### 1. 「回合後抹掉段」取代「機密段」
- `LaneTurn.confidential` 與 `TurnInput.confidential` 改名 `erase`（回合後要從 session 抹掉的子段）：內容＝本輪尾段世界書＋原本的機密段，在 tail 裡恰好連續出現一次。`PendingRewrite` 欄位跟著改名，加 `#[serde(alias = "confidential")]` 讀舊 `lanes.json`〔模型判斷·未裁決〕。抹寫函式（`apply_rewrite`、`grok_session::rewrite`、`settle_abort`、`record_rewrite_failure`）參數一起改名，邏輯不動。
- **角色共線**（`Hoist::None`）：`public` 改先寫進抹掉段，接著才是原機密段內容；tail＝抹掉段＋`\n`＋本輪指定。送出的 tail 逐字不變，只是抹的範圍多了公開世界書。`Hoist::StableConfidential`（API 單卡，無狀態）與 `Hoist::All` 不變。
- **GM 線**：`gm_lane_turn` 把世界書段（段標＋本文＋後面接的分隔換行）獨立成 `erase`，tail＝`erase`＋狀態／觸發器＋導演指示，tail 逐字不變。沒有尾段世界書時 `erase: None`（那輪不抹、不讀檔）。目前狀態與觸發器不進抹掉段（不在裁決內，見五之 Q2）。
- `gm_lane_parts` 改回 `(String, LaneTurn)`，`gm_lane_reply` 傳 `erase: turn.erase`（Agy 見下），`measure.rs` `gm_request` 照用 `turn.tail`。

### 2. GM 線接上抹寫（`run_turn`）
- Claude：`apply_rewrite` 本來就不分線，傳了 `erase` 就抹；`prefix` 仍是 None。抹寫有讀檔時沿用載入內容做提醒掃描，沒讀檔照舊 `scan_reminder_readonly`。
- Grok：`erase` 有值時 GM 線也呼叫 `rewrite_grok`，`prefix` 空字串、`cleaned` 傳 `reply.trim()`（不換寫回覆）；順帶拿掉 reasoning（同角色線）。`erase` 為 None 時 GM 線照舊不改檔。
- 失敗退路：
  - Claude GM 抹寫失敗：改用 `retire_claude_gm_session`（store 拿掉並落檔、成功才刪舊 session 檔、刪不掉記 `cleanup-failed`），本輪回覆照常回傳，下一輪 `first-turn` 重開；不只 `store.remove`，免得留下孤兒檔（比照 gm-line-system-change-restart 三之 3）。角色線失敗路徑不動〔模型判斷·未裁決〕。
  - Grok GM 抹寫失敗（含 CLI 自動壓縮 `compacted`）：照角色線撤線（`GrokRevoke::Failed`）。
  - 中止、失控、空回覆：`settle_abort` 已帶 `erase`，Claude 照抹、Grok 撤線；`pending_rewrite` 留著，下一輪重開。崩潰：`PendingRewrite` 重開，舊尾段不會被續用。
  - 抹掉段在本輪 user 行出現不只一次（例如玩家原文剛好含整段世界書）：走抹寫失敗，下一輪重開。
- **Agy GM 線**（沒有抹寫路徑）：建議比照 `Hoist::All`——把本輪尾段世界書接在 GM 凍結 system 末段、tail 不放，system 一變就靠既有 `SystemChanged` 整線重開〔模型判斷·未裁決，見五之 Q1〕。`gm_lane_parts` 多收一個「世界書提進 system」開關，實送與量測同一個（provider＝Agy 時開）。

### 3. 快取、水位、容量
- **Claude 快取**：CLI 只有最後一則訊息的斷點可重用，抹掉最新 user 行就作廢它（claude-resume-tail-cache 一之 3）。有抹寫的下一輪只中 system 段，歷史整段重寫一次；沒抹的輪照舊中到上一則 user。keyword 條目每輪都觸發的桌，Claude GM 線的命中會掉到跟現行角色共線一樣（只中 system）。system 本身不受影響。
- **Grok 快取**：實測抹 user 段只少最後一截增量（grok-shared-lane 實測表），GM 線影響小。
- **水位**：`sent_events`、`sent_hash`、`expected_reply` 只看事件與回覆，抹的是 tail 內文字，不受影響，不改。
- **帳本診斷**：`expected_cached`＝上輪總輸入；Claude 線上輪抹過後實際只能中 system，會被報成 `below-expected`，額度分頁標成異常。`LaneState` 加 `erased_last: bool`（`#[serde(default)]`，抹寫成功才設 true），Claude 線上輪抹過時 `expected_cached` 記 0（理論可中量不明，中了算 Hit、0 算 NotExpected）；Grok 不套〔模型判斷·未裁決〕。角色共線現在每輪抹機密段的誤報一併消掉。
- **容量量測**：`measure.rs` 本來就只算「重開全量＝事件＋一份尾段」，抹掉後 session 真的只剩一份，估計與實際對齊，不改碼。`path_budget` 取估計與上輪實報（`last_prompt_tokens`，含上輪那一份尾段）的較大者，下一輪換上新尾段，量級相同，不改。

## 四、測試清單
- `transport/turns.rs`：
  - `chars_lane_turn_isolates_confidential_segment` 改名並擴充：公開觸發條目在 `erase` 內、`erase` 在 tail 恰好一次、抹掉後只剩本輪指定；tail 與改動前逐字相同（比對固定字串）。
  - 只有公開條目、沒有私設與限定條目的角色：`erase` 有值且只含公開段。
  - `gm_lane_turn`：有尾段世界書時 `erase`＝段標＋本文＋分隔，tail 恰好一次、抹掉後狀態與導演指示原樣；沒尾段世界書時 `erase: None`；狀態區塊不在 `erase`。
  - Agy GM（Q1 若採提進 system）：世界書在 system 末段、tail 沒有。
- `scaffold_baseline` 三語系：tail 逐字不變，預期不變；有差異先查原因。
- `lanes/tests`（假 claude）新檔 `tail_erase.rs`：
  - GM 線三輪觸發不同 keyword：每輪回合後 session 檔裡沒有任何尾段世界書（本輪的也已抹）、第三輪送出的 prompt 只含本輪那份；續聊、無 `reopen`、無 `rewrite-failed`。
  - GM 某輪沒尾段世界書：不讀不寫 session 檔（修改時間不變）。
  - GM 抹寫失敗（抹掉段在事件原文也出現）：記 `rewrite-failed`、舊 session 檔已刪、下一輪 `first-turn`。
  - GM 中止：世界書已抹、下一輪 `pending-rewrite` 重開。
  - 角色共線只有公開條目的一輪：公開段被抹。
  - 上輪抹過的 Claude 線：帳本 `expected_cached` 0；上輪沒抹：照舊上輪總輸入。
- `lanes/tests/grok.rs`：GM 線兩檔都抹、reasoning 拿掉、回覆不改；`erase` 為 None 時不改檔；壓縮事件撤線。
- `run_turn` Agy GM：不帶 `erase`，不撞 `LaneRewriteUnsupported`。
- `lanes` 讀舊 `lanes.json`（`pending_rewrite.confidential`）：照讀成 `erase`。

## 五、待主線決定
- **Q1 Agy GM 線**：
  - A（建議）：尾段世界書提進 GM system、變了整線重開。結果：Agy GM 不疊；keyword 常觸發的桌幾乎每輪重開（Agy GM 歷史改成重開時壓平的逐字稿）；作者註記／依深度在 Agy GM 上進 system 而非尾段（同 `Hoist::All` 現行裁決）。
  - B：Agy GM 維持每輪疊一份，列已知差異。結果：Agy GM 照舊續聊、舊條目留在歷史、量測低估。
- **Q2 GM 尾段的「目前狀態」與觸發器**：同樣每輪疊一份（角色共線的狀態區塊在機密段、會抹）。
  - A：一起抹。結果：GM 只看到本輪狀態；有狀態欄的桌 Claude GM 每輪都抹、只中 system。
  - B（現行，本案預設）：不抹，裁決只涵蓋世界書。結果：GM 歷史裡留著每輪的狀態快照、量測低估這部分。

## 六、驗收
- `npm run verify` 全綠；不留 tsc 產生的未追蹤檔。
- 測試通道（claude 低階檔位）：帶 keyword 世界書的測試卡，GM 推進三輪觸發不同條目，讀 `~/.claude` 該 session jsonl：回合後沒有任何尾段世界書；帳本無 `rewrite-failed`、無 `reopen`（首輪除外），抹過後的續聊輪快取不被標成 `below-expected`。多卡桌角色共線一輪公開條目觸發：session 裡已抹。
- Grok 低階檔位 GM 兩輪觸發 keyword：無 `rewrite-failed`、session 兩檔沒有舊尾段（驗 GM 回覆原文與檔內一致的假設）。
- 文件同步：`worldbook-st-trigger-parity.md` 七「GM 續聊線與角色共線的公開段在回合後不抹」改成已處理；`lanes/mod.rs` 模組註解（chars 線抹的內容、GM 線也抹尾段世界書）；`transport/turns.rs` `LaneTurn` 註解。

## 七、已知差異
- Claude 線（GM 與角色共線）有尾段世界書的輪，下一輪只中 system 段（CLI 只留最後一個斷點）；ST 每次生成重送全部，沒有續聊快取可比。
- 抹掉段與事件原文撞字時那輪抹寫失敗、下一輪重開（只損快取）。
