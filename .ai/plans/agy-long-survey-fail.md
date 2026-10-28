# agy-long-survey-fail — 根因與修法

## 根因（agy 1.4.8，Gemini 3.8 Flash Medium，測試 HOME 實測 2026-10-28）

三種獨立失敗，與舞台卡指示無關（拿掉 STAGE 段照樣重現）：

1. **模型自己叫工具**：agy 預設把約 60 個工具和約 6.8k token 的代理系統提示一起給模型。盤點時模型會先 `run_command ls -la`「看工作區」，無頭模式 soft-deny 後整輪結束、正文空白。
2. **思考期間 stdout 完全沒輸出**：print 模式的 stream-json 只有 init／step_update／result，沒有思考事件或保活。預設代理下盤點思考 15k–49k token、靜默 68–358 秒，撞上「120 秒無輸出＝卡住」。
3. **印出 result 後不退出**：關閉時等背景的對話標題產生（`maybeGenerateConversationTitle`，goroutine dump 為證），某些內容的標題請求永不返回，程序閒置不退、不理 SIGTERM。stdin、`-p`、`--input-format stream-json` 三種輸入都會產生標題（1.4.8 實測，與舊 changelog「-p 跳過標題」不符），也找不到關閉的設定或旗標——每開一條新對話都多一次標題模型呼叫，無法避免〔已知限制〕。

`--model gemini-3.8-flash-medium --effort low` 直接報衝突：思考量由玩家選的型號決定。

## 做法（三方共識 2026-10-28；停滯門檻選 A〔作者裁決 2026-10-28〕）

- **精簡工具集代理**：`agy_profile` 每次確保 `agy-home/.gemini/config/agents/table-tavern-text.md`（`tools: []`、`excludeDefaultComponents: true`、`mainAgent: true`、`subagent: false`，本文一句中性說明；內容相同不動，不同就臨時檔＋rename，原位被非檔案佔住或寫失敗一律回錯）。聊天 lane、旁白、卡重構帶 `--agent table-tavern-text`；生圖（要 generate_image）不帶。模型仍看得到內建的 manage_task。
- **版本門檻 1.2.1**（`agy_version_supported`，取代原本 1.1.8 的 stream-json 門檻）：以下回 AgyTooOld、不照跑；版本字串認不得時放行並照帶 `--agent`。
- **收尾寬限**：各家以官方結束事件判完成（claude `result`、codex `turn.completed`、grok `end`、agy `result`）；只在成功收尾時起算 10 秒、從收到那刻定下不重算，寬限期間照常解析（claude 計費事件可能晚到），期限分支排在讀管線前；到期殺程序後走正常完成路徑。agy SUCCESS 但正文空白照舊回 cli_no_reply（附 stderr 尾巴）；寬限照樣起算，免得「工具被拒又不退出」退回等斷流。agy 是單一程序、沒有子程序，殺 pid 即可。
- **grok `end` 分類**（照官方 headless 文件 streaming-json 節，Grok 1.0.70 自己答不出、帶文件追問後同意）：end_turn＝完成；max_tokens、max_turn_requests＝`AI_INCOMPLETE_RESPONSE`；refusal、cancelled、缺欄或文件沒列的值＝失敗。大小寫與底線不拘（1.0.70 實測寫 `end_turn`，舊版寫 `EndTurn`）。
- **首個進展保底**〔模型判斷·未裁決〕：「每出一行就續 120 秒」不變，另加第一個正文或思考事件出現前的 300 秒保底——期限取兩者較晚者，不是「最多 300 秒」（init 與 stderr 不算進展，但照樣續命）。斷流錯誤不觸發 lane 的丟線重開重試，收尾比照輸出失控（留 pending_rewrite／撤線，下一輪重開）。不設整輪絕對上限。文案分兩種：還沒出字＝「等待回應逾時，可能仍在思考」（cli_stalled），出過字＝「回應中斷：超過 120 秒沒有新內容」（cli_stalled_after_output）。

## 實測（測試通道真 app，2026-10-28）

- 盤點 5 張卡各 3 次（林教授、灰岩要塞、瑟拉、冒險者公會、西幻）：15/15 成功，最長 245 秒；西幻卡 3/3 印完不退、10 秒後被殺、答案照收；全程 0 次叫工具，結束後無殘留 agy 程序。舞台判定與回歸集一致。
- 西幻桌 GM 線開線一輪、續聊一輪：第二輪問只有常駐世界書才有的公會等級，答對（鐵到傳奇六級、新人從銅級起），續聊那則 stdin 不含這段設定、開線那則有，兩輪同一 conversation。這次開線標題順利產生、沒有卡住（卡不卡看標題請求內容，無法主動觸發）。
- 卡住後續聊（真 agy、與 app 相同的旗標與代理）：西幻盤點 prompt 開對話 → 印完卡住、10 秒後殺 → `--conversation` 續問卡片裡的公會等級，同一 conversation、答對。回答原文未留存（腳本只印到終端機）；agy 對話資料庫留在 scratchpad `agyask-home/.gemini/antigravity-cli/conversations/044d1f9b-de57-42df-90ad-32ad6c0a47e3.db`（測試 HOME，scratchpad 清掉就不在）。
- 林教授桌 GM 與角色各開線一輪、續聊一輪：四輪都成功，續聊沿用同一 conversation（agy log：conversation 屬於 table-tavern-text），旁白與角色格式正常。同一份 GM 開線正文：預設代理 input 8,503 → 精簡代理 1,712 token。

## 業界對照
- 關工具：Claude Code `--tools`、Gemini CLI `excludeTools`、Codex `--sandbox read-only`；agy 是自訂代理的 `tools:`＋`excludeDefaultComponents`。
- 逾時：首字與字間分開計時，app 的 API 路徑（`transport/stall.rs`）與 CLI 路徑現在一致。
- 子程序：以結束事件為完成，不退出的子程序寬限後強殺。
