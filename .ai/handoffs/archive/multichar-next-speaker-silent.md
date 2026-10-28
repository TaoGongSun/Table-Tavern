> 結案 2026-10-28：Sol、opus-review 複審 PASS，進 main（結構、行為兩筆）。

# multichar-next-speaker-silent — 對 GM 發言後角色不接話

Status: done

## Summary
實測佇列第 19 項②（2026-10-28，Sonnet CLI 3 回合）：多角色（非介面）桌，玩家對 GM 打字送出，GM 每輪都點名「下一位：X」，角色 0 次接話。

## 根因（不是回歸）
- 打字送出且對象是 GM 時，`useChatController.ts` 的 `submitTurn` 只跑一次 `narrateOnce`，回傳的點名直接丟掉；點名→角色接話的接力只在「GM 推進」按鈕（`gmAdvance`）裡跑。
- 自 2026-07-28 `90bba6fb`（GM 能當發言對象，「GM 以旁白回應」）起就是這樣，介面桌裁決（`7ab0b220`）沒動這條路。
- 點名行由 `transport/response.rs` 從顯示文字剝掉，玩家看不到 GM 點了誰；模型照樣付費產出這一行。
- 證據：19 項逐字稿 3 則旁白的 `raw` 都有「下一位：X」、0 則 `gm_call` 點名事件（沒按過推進）；前端單元重現（假 invoke）：打字送出只呼叫 `gm_narrate`、零次 `chat_with_character`，同條件 `gmAdvance` 會接話。

## 拍板
作者選 C〔作者裁決 2026-10-28〕：預設維持一次旁白，旁白下方出現「讓 X 接話」鈕；送出鍵旁加「送出並讓點名角色接話」走 B（GM 旁白＋被點名角色接一句就停）。

## 現況
- 方案 [plans/multichar-next-speaker-silent.md](../../plans/multichar-next-speaker-silent.md)（三方共識）。進 main 兩筆：結構（`chat/useTranscriptStore.ts`）、行為；verify 12 步綠。rebase 到 play-folder-split 與「收回一樓」之後，點名列屬附屬行，收回對白後「讓 X 接話」重試不疊列。
- 測試通道實測（迷霧酒館範例桌、Claude CLI haiku）：一般送出 1 次旁白、旁白下出「讓狐狸接話」0 則對白；點鈕→「GM 請「狐狸」發言」＋狐狸一句、鈕消失；「送出並接話」一次得到旁白＋騎士一句就停；收回旁白後鈕消失。全程 5 次模型呼叫（1＋1＋2＋1）。點名到玩家的情形 3 輪都沒遇到，只有單元測試覆蓋。驗收修正輪後補拍：範例桌匯入自帶介面的卡（WestFantsy）→ 不點名、B 藏；改成 auto_hidden → B 出現；旁白帶它出場 → B 仍在（後端旗標未變、GM 仍點名，一致）；換幕後旗標清掉 → B 藏。截圖在 scratchpad `mcshots/`（10–16）。

## 後續
另案：gm-advance-hide-no-nomination（不點名桌的 GM 推進）、character-send-failure-draft（對角色送出落檔後失敗仍帶原文）。
