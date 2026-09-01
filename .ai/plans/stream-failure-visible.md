# stream-failure-visible — 規格

實測起點：2026-08-21 使用者以 OpenRouter 免費 `deepseek/deepseek-v4-pro-0813-free` 連打，5 次呼叫中 2 次「思考完就停」——串流正常走完 `[DONE]`、usage 尾塊有回，但 `delta.content` 零字。空回應被當正常 GM 回合寫進 transcript、進入下一次呼叫的歷史，直接造成第 3 次答非所問。全機 950 則 transcript 事件中 3 則空白事件全出自這天。

## 拍板結論（2026-08-21，與 Sol 三輪收斂）

**四段防守，缺一不可**
1. `transport::stream_chat`：SSE 的 `error` 塊原樣拋出；收工時判 `trim()` 後正文為空、判異常 `finish_reason`。
2. `gm_narrate`：`extract_state_block`／`extract_next_speaker` 剝殼後再判一次空，**擋在 `apply_block` 之前**——否則失敗回合照樣重擲一輪骰。
3. 通過語意檢查才准寫 state、記 mechanism log、處理登場。
4. 前端 `appendEvent` 拒收空白事件（資料污染保險）。

**判定優先序**（固定，`length`＋空正文歸 incomplete 不歸 empty）
SSE error 原話 → `content_filter` → `length`／未知終止原因／無 `[DONE]` 就 EOF → 正常收尾但正文 trim 後空 → 成功

**三個錯誤碼**（錯誤字串前綴，不動 `DataResult<String>`）
- `AI_EMPTY_RESPONSE`：正常收尾但可見正文為空（含 GM 剝殼後 `display` 為空；前端保險層觸發時同碼）
- `AI_INCOMPLETE_RESPONSE`：`finish_reason` 為 `length`／`tool_calls`／未知，或無正常收尾就 EOF
- `AI_CONTENT_FILTERED`：`finish_reason` 為 `content_filter`
SSE `error` 塊**不加碼**，原樣 `Err(error.message)`——免費層 429 的原話能被既有 `QUOTA_ERROR` 正則接住，玩家看到「額度用完」而非籠統錯誤。`error.message` 缺失或非字串時序列化整個 `error` 物件，不得靜默忽略。

`explainAiError` 先 `startsWith` 認碼，認不到才退回既有 quota／auth 正則；既有分流不動。

**不做**：自動重試；拒絕偵測（2026-08-21 使用者裁決：那句簡體拒絕是刻意測 NSFW 的預期結果）；`delta.reasoning` 心跳（獨立小改動，另案）。

**不改回傳型別**：實測失敗全為零正文，「半截正文」尚未發生；真出現再擴成 `{ text, finish_reason, complete }`。reasoning token 只供錯誤診斷，走 API 路徑獨立 `Option<u64>`，不擴充 `PromptCacheUsage`（避免連帶改 CLI parser 與測試資料）。

## 自驗（2026-08-21 完成，全綠）
cargo 504／vitest 141／`tsc --noEmit`／`npm run build`／`npm run check:i18n` 十語系。
新增測試涵蓋：優先序全案例、空 `error.message` 回退整包、`finish_reason` 取最後非 null、壞 JSON 不爆、`[DONE]`＋正文＋無 finish_reason＝成功、端到端 mock server 零內容回 `Err`，以及 `explainAiError` 的碼優先分流四案。

## 待實測清單（T1／T2 已通過，其餘未測試）

失敗態靠碰運氣重現——實測 2026-08-21 是免費 `deepseek/deepseek-v4-pro-0813-free` 五次中兩次，2026-08-21 打完包後連送數次反而都順。**遇到再逐項核對，不必特地製造。**

事後判定 T2 最省力的方式（不必記得當下畫面），掃全機 transcript 有沒有新的空白事件。
`assert` 那行是防呆——讀不到檔案時要當場喊停，不能靜默回報 0 則假裝通過：

```bash
cd ~/Documents/TableTavern/worlds && python3 -c "
import json,glob
files = glob.glob('*/transcript/*.jsonl')
assert files, '讀不到任何 transcript，這次檢查無效（不等於通過）'
n = 0
for p in files:
    for line in open(p, encoding='utf-8'):
        line = line.strip()
        if not line: continue
        e = json.loads(line)
        if not (e.get('text') or '').strip():
            n += 1; print('空白事件', p, e.get('ts'), e.get('speaker_name'))
print(f'掃了 {len(files)} 個檔案，空白事件 {n} 則')
"
```

基準線：2026-08-21 修復前共 3 則空白事件（`01KZ54TYVTKS3930H476ETWF2M/transcript/0.jsonl` 兩則 03:39:18／03:45:14、`01M0A1VXYXY3ZZ8BWFN30QDJ4D/transcript/3.jsonl` 一則 03:49:39）。**修復後這個數字不該再增加。**

| # | 什麼時候會踩到 | 看哪裡 | 通過條件 | 狀態 |
|---|---|---|---|---|
| T1 | AI 思考完卻零內容、或回覆被截斷 | 聊天室錯誤列 | 出現人話錯誤（「AI 這次沒有回出內容…」或「…沒寫完就中斷了」），底下小字帶 `finish_reason` 與 token 數。**不是一片安靜** | **通過（2026-08-21）**：角色接話踩到零內容，錯誤列顯示人話＋`AI_EMPTY_RESPONSE: model=deepseek/deepseek-v4-pro-0813-free finish_reason=stop reasoning_tokens=568`。順帶證實供應商真的會回 `completion_tokens_details.reasoning_tokens`（實作時只能推論） |
| T2 | 同 T1 | 故事本體＋上面的掃描指令 | 畫面上沒有多出 GM 空白泡泡；掃描結果仍是 3 則（沒有新增） | **通過（2026-08-21）**：T1 那次失敗後掃 63 個檔案仍是 3 則，時間戳全在修復前的 03:39–03:49，零新增 |
| T3 | 同 T1，且該桌機制是每回合重擲骰（`incremental`） | 狀態欄骰值 | 失敗前後數值一樣，沒有被白轉一輪 | 未測試 |
| T4 | 免費層當日額度用完（429） | 聊天室錯誤列 | 顯示「這個 AI 來源的額度用完了。換一個 AI 來源，或等額度重置再試。」——不是 T1 那句 | 未測試 |
| T5 | 供應商以 `finish_reason=content_filter` 擋下 | 聊天室錯誤列 | 顯示「這個 AI 來源擋下了這次回覆。換個說法，或改用其他模型。」<br>註：模型改以「一句拒絕文」回應時本案不處理（2026-08-21 使用者裁決），那種情況 T5 不會觸發 | 未測試 |
| T6 | 任何時候（可拿那桌現存的兩則舊空白事件測） | 收回／復原按鈕 | 連按收回把空白收掉後，復原放回的是**有內容**那則（跳過空白）；疊裡只剩空白時復原鈕不亮 | 未測試 |
| T7 | 同 T1 | `~/Documents/TableTavern/prompt-cache.jsonl` 尾端 | 失敗那次仍記了一行——失敗一樣燒 token，額度分頁不能少算 | 未測試 |

測到哪項就把該列狀態改成「通過（日期）」或「紅：現象」。
