# 桌子太長撞到指令長度上限：先修 Windows、上限前提醒換幕

公開前必做〔作者裁決 2026-10-04〕。實測數據、施工內容、範圍 3 草案在 [plans/long-prompt-scene-hint.md](../plans/long-prompt-scene-hint.md)。分支 `long-prompt-scene-hint`（未合併）。

## 範圍（立案原文）

1. 實測三個作業系統下 claude／grok／agy 撞上限的門檻與錯誤表現。
2. 撞得到就改傳遞方式（stdin 或暫存檔），不能只靠提示補救。
3. **上限前就提醒換幕**：換幕本身也要送 AI，所以必須在還送得出去時就提醒；門檻要把換幕那次呼叫的長度算進去〔作者裁決 2026-10-04〕。改走 stdin 後改看模型 context window。
4. 真的撞上時的錯誤走 `api_error_actionable`／`explain_ai_error` 既有的人話錯誤路線，不新增機制。
5. 文案十語系。

## 現況

- 範圍 1、2 完成，驗收通過（Sol PASS，主線重跑 verify 10 步綠），待結案。CLI 的 system／正文都不再進命令列：claude `--system-prompt-file`、grok agent profile＋`--prompt-file`＋`--verbatim`、agy 正文走 stdin；暫存檔在 `<config_root>/cli-prompts/`。
- claude haiku、agy gemini-3.6-flash-low 已在測試通道真 app 實送長桌（plans §1、§2.5）。
- 2026-10-06 拍板〔作者裁決 2026-10-06〕：發現 D 交給換幕提醒；提醒門檻＝（上限 − 換幕摘要呼叫所需空間）× 80%，同時看聊天與換幕呼叫；鎖只看換幕、只在再送一句換幕就送不出去時才鎖；拿不到上限的後端不鎖、自訂 base_url 不提醒；換幕送不出去時分段摘要再合併；範圍 4 加「太長了，請換幕」專屬錯誤附換幕鈕，十語系。細節見 plans §3、§4。
- 範圍 3／4 施工計畫已併入 Sol 第 1–3 輪意見，施工中。

## 下一步

1. 主線確認計畫後照 plans §3、§4 施工。
2. grok 真送（2026-10-06 22:00 額度恢復後）：正文自身 >100KB，確認 `--verbatim` 不搬檔、模型讀得到尾巴；grok 爆 context 時的錯誤長相與上限。結果回寫 plans §1.3、§3.4、§4。
3. 結案時刪暫時探針 `.github/workflows/argmax-probe.yml`、`.github/argmax-probe/`，再照 CLAUDE.md 壓縮合併。
4. 三家真 CLI 在 Windows 讀暫存檔已排進[實測佇列](../reference/verification-queue.md)梯 3。
