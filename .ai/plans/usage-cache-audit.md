# usage-cache-audit 規格

交接檔：[handoffs/archive/usage-cache-audit.md](../handoffs/archive/usage-cache-audit.md)。

## 邊界

| 相鄰案 | 本案做 | 留給該案 |
|---|---|---|
| non-claude-real-cache | 實跑量出 codex／agy／OpenRouter 在本次模型與樣本下的命中 | 尾巴重播等機制改動；看本案數字決定做不做 |
| vendor-prefix-floor | 記下各家首輪觀測到的白送底線 | 額度分頁扣底線的判定與顯示 |
| no-cache-model-optout | 記下有沒有「原始 usage 有欄位、值為 0」的零命中 | 零命中自動退回 |
| prompt-cache-optimization | 驗一次離開提醒 | undo 截尾等其餘尾巴 |
| api-cache-visibility（已結案） | 只讀它定的 `cache_reporting` 語意 | — |

本案只動抹寫失敗的落帳與測試包的量測掛點；「各狀態掛什麼標籤、顯示什麼」只寫建議，列成待使用者決定，不改 UI。實跑若發現解析把缺欄補成 0，只記證據、另立案修，不在本案修。

## 一、抹寫失敗原因落帳本

現況：[lanes/mod.rs](../../src-tauri/src/lanes/mod.rs) 有三處把錯誤吞掉、只寫 `reason`：回合後抹寫（`Err(_)`）、中止收尾 `settle_abort`（`rewrite.is_err()`）、保溫截尾 `truncate_ping`（`Err(_)`，reason=`ping-truncate-failed`）。三處一併補。

改法：

1. `apply_rewrite`／`truncate_ping` 改回 `Result<(), RewriteFailure>`，`RewriteFailure { stage: &'static str, detail: String }`。stage 固定六個：`load`（讀檔或逐行驗證失敗）、`find-segment`（含機密段的 user 行 0 行或 ≥2 行）、`erase-segment`（該行內片段不是恰好一次）、`prefix-assistant`、`truncate`、`write`（原子寫或回讀驗證）。各步驟用同一支小 helper 把 session_file 的錯誤字串標上 stage，session_file 的回傳型別不動。
2. `usage_log::append_event` 多帶 `Option<&RewriteFailure>`，落帳行多兩欄 `stage`、`detail`。detail 先遮路徑、再截斷：已知路徑（session 檔、其父目錄、claude_home、working_dir）的字串整段換成檔名或 `<dir>`——用已知字串比對，含空白與 Windows 反斜線都照樣命中；之後按 Unicode 字元截到 300 字。`reason` 值不變，舊帳本與 `usage/report.rs` 不受影響。
3. 刪檔／刪目錄失敗（claude `settle_abort` 的 abandon、grok 撤銷線刪 session 目錄）落 `reason: cleanup-failed`、`stage: cleanup`，detail 同樣遮路徑。不加重試、不改丟線策略。
3a. grok 角色共線（合併 main 後）：抹寫失敗 `reason: rewrite-failed`，stage 為 `rewrite`／`compacted`／`lock-timeout`（GROK_HOME 缺則 `locate`），detail 遮 session 目錄（群組名是編碼過的 cwd）與 GROK_HOME；中止、續聊失敗、開線失敗照記 `aborted`／`resume-failed`／`open-failed`；預定重開（換幕、改卡等）撤銷舊線但不記丟線。所有 `drop-lane` 的 `transport` 記實際 provider。
4. 測試矩陣：
   - load：session 檔不存在。
   - find-segment：機密段不在檔內；機密段出現在兩行 user。
   - erase-segment：同一 user 行內機密段出現兩次（find 命中一行、erase 判兩次）。
   - prefix-assistant：最後一則 assistant 第一段不是 text。
   - write、truncate：用 helper 的可控失敗注入驗 stage 映射，不改正式策略；真實 I/O 寫入失敗（父目錄唯讀）只在 Unix 平台另留一條。
   - 遮罩：含空白的 POSIX 路徑、`C:\Users\a b\...` 形式的 Windows 路徑、遮完才截斷（多位元組字元不被切半）。
   - lanes 既有抹寫失敗 e2e 改斷言帳本帶 `stage`。

已知可疑點（實跑時留意，不預先修）：`session_file::load` 逐行驗證要求 user 行 content 是字串，CLI 若改寫成陣列會整檔讀失敗＝`load`；CLI 若正規化換行或尾端空白，機密段逐字比對落空＝`find-segment`。

## 一之二、丟線修法：session 檔驗證接受 attachment 行

根因見「四、對帳結果」。修在 [lanes/session_file.rs](../../src-tauri/src/lanes/session_file.rs)。

1. **哪些行算鏈上節點**：user、assistant，加上 `type: attachment`（白名單）。本機 38 個 session 檔加 4 份留證掃過一遍，帶 uuid 的非對話行只有 attachment（250 行；子型 environment、model、date、session_context、prompt_snapshot、total_tokens_reminder、credential_org）。其餘沒有 uuid 的行（queue-operation、last-prompt、atis-latch、file-history-snapshot、cost-state、mode、ai-title）照舊不檢查、原樣保留。
2. **鏈怎麼接**：`parse` 維護「鏈尾」＝最後一個節點的 uuid。每個節點（含 attachment）的 `parentUuid` 必須等於鏈尾：第一個節點必須是 null，之後就換它當鏈尾。所以 user→attachment…→assistant 照樣驗證是單一直線，不會因為跨過 attachment 而放過分岔。所有節點的 uuid 必須全檔唯一（對話行與 attachment 之間也不得重複），否則 `u1→x→u1` 這種環也能過關，而抹寫按 uuid 找行會命中第一則。
3. **不認得的型就拒絕**：帶字串 uuid、卻不在白名單裡的行（例如將來的 `system`）直接驗證失敗，detail 寫出型別名稱。這樣仍走丟線，但帳本看得出是新型別；不採「凡是帶 uuid 都當節點」的寬鬆做法，因為不知道新型別會不會改變 resume 的語意。attachment 行只驗 uuid 與 parentUuid，內容不檢查。
4. **抹寫／截尾不動 attachment**：
   - `find_user_line_with_segment`、`erase_user_segment`、`prefix_last_assistant` 照舊只看 user／assistant。
   - CLI 會把 thinking 和 text 拆成兩則 assistant；`prefix_last_assistant` 找最後一則，也就是 text 那則。實測的檔都是這個順序。
   - 保溫截尾改成「記住保溫前的檔、事後還原」：送 ping 前讀下整份原文。讀不到就這條線不保溫。保溫後重讀，必須以保溫前原文為前綴（CLI 只追加），追加段裡恰有一則含保溫訊息的 user 行；然後把保溫前原文原子寫回並回讀比對。CLI 可能在 ping user 之前先追加 queue-operation 或 attachment，這些前置行也一併消失。前綴被改動＝`truncate` 失敗，丟線。`truncate_from` 隨之刪除。
5. **原樣寫回**：`SessionFile` 每行另存讀入時的原始文字，含行尾分隔符（`\n`／`\r\n`／末行無換行）；空白行也當一行保留。寫回時，沒改過的行（含 attachment 與所有雜項行）逐字照抄，只有被抹寫或補前綴的那一行重新序列化，並沿用它原本的行尾。現行做法是整檔重新序列化，每一行的鍵順序都會被改掉（serde_json 沒開 preserve_order）；改完後未改動的行逐位元組不變。回讀驗證改成逐位元組比對。鏈驗證用的 helper（含 attachment）與抹寫／補前綴用的 helper（只認 user／assistant）分開，改寫時不會誤選 attachment。
6. **回歸測試**：測試資料是手寫、照真實結構脫敏的 fixture，只留型別、uuid 鏈與欄位形狀，正式 session 內容不進 repo。
   - 2.1.287 形狀：兩組 queue-operation、user、四到八行 attachment 夾著 file-history-snapshot／atis-latch、thinking 和 text 兩則 assistant、assistant 後還有 attachment，再接 last-prompt／cost-state；第二輪的 user 的 parent 接在前一輪最後那行 attachment 上。
   - 2.1.227 形狀：user、一行 total_tokens_reminder attachment、ai-title、兩則 assistant。
   - 斷言：
     - 兩種形狀都能 parse。
     - 抹寫機密段＋補前綴後寫回：改動只落在目標兩行，其餘行逐位元組不變。
     - 截掉保溫問答後，檔案與保溫前逐位元組相同；含「ping user 之前 CLI 已先追加 queue-operation／attachment」的情況。保溫期間前段被改動要失敗。
     - uuid 重複（對話行之間、對話行與 attachment 之間）要失敗。
     - fixture 帶不同的鍵順序、多餘空白、CRLF、空白行、末行無換行，寫回後未改動行逐位元組不變。
     - 最後一則 assistant 不是 text 時補前綴失敗（保留）。
     - attachment 的 parent 沒接上鏈尾要失敗。
     - 未知型別帶 uuid 要失敗，且訊息含型別名。
     - 舊有的 user／assistant 驗證測試全部保留。
   - lanes e2e 的假 CLI session 檔也插入 attachment 行，走完「開線→抹寫→續聊→保溫截尾」不丟線。
7. 驗收：verify 綠；測試通道 claude 角色線連講 3 輪，帳本不再出現 `drop-lane`，第 2、3 輪 `mode: resume` 並讀到上一輪的量；GM 線保溫一次後截尾成功、線還在。
   - 2026-10-06 02:41–02:52 實跑（claude 2.1.287、haiku-4-5）：角色線第 1 輪開線；第 2 輪續聊讀 5,027／理論 5,037，第 3 輪讀 6,236／6,246。GM 推進開 GM 線。後端保溫同時打到兩條線（角色線距上次 283 秒讀 7,563，GM 線 206 秒讀 6,984），還原都成功。保溫後角色線再講一句，續聊讀 7,563／7,573。全程沒有 `drop-lane`、沒有留證。

## 二、測試包量測掛點（只在 `test-harness` feature）

正式包不含；沿用 AI log（`<root>/harness-ai.log`）的 dispatch id 串起呼叫。

1. **原始 usage**：各家解析收尾 usage 的位置多寫一行 `usage-raw`，帶同一 dispatch id、原始收尾事件的 usage 物件原樣（缺欄就是缺，不補 0）、實際回應模型；API 另帶 OpenRouter 回的 `model`／`provider`；agy 同時記原始累積值、上輪基準與算出的本輪差分。
2. **抹寫失敗留證**：抹寫或截尾失敗時、在丟線與 `abandon_session` 刪檔之前，把當下的 session 檔整份、要抹的機密段與名字前綴、stage／detail 寫進 `<root>/rewrite-failures/<時間>-<唯一 ID>-<安全化線名>/`（線名如 `chars:<model>`，模型可能含 `/`，非英數字元一律換 `_`），原線名寫在目錄內 metadata。留證失敗不阻止原本的清線／刪檔，但在 AI log 記一行 `rewrite-evidence-failed`。

留證是同步檔案 I/O，會讓清線稍晚一點完成；成功時在 AI log 記一行 `rewrite-evidence` 附耗時。session 檔複製失敗時照樣寫 metadata，同時記 `rewrite-evidence-failed`。

CLI 版本在實跑前用各家 `--version` 記下；claude 另以 session 檔逐筆 usage 對照帳本。回應沒給實際模型／provider 的欄位記「未回報」，不拿請求值冒充。

## 三、實跑對帳

### 環境

- `npm run harness:build`，`env -u ANTHROPIC_BASE_URL node scripts/harness.mjs launch --fresh --config-from <正式 config.json> --root <本案專屬 root>`。帳本落在 `<root>/data/prompt-cache.jsonl`，正式帳本只讀。
- 測試通道全機一把鎖（`harness/root.rs` identifier 鎖），同時只能一個實例；與其他子代理輪流，啟動前 `ps`／`status` 確認，跑完立刻 `quit`。
- Grok：用已登入 grok 的既有測試 root，不帶 `--fresh` 啟動，不複製任何登入檔。
- 保溫只在 `document.hasFocus()` 為真時發；測試包視窗不在前景，以 `js` 覆寫成恆真（只為量測，不改程式）。
- 測試 root 在報告寫完、`rewrite-failures/`、帳本、`harness-ai.log` 與 CLI 版本紀錄都複製到 scratchpad 之後才刪。

### 步驟

模型一律最低階、整段不換：claude `claude-haiku-4-5`、codex `gpt-5.6-luna`、agy `gemini-3.8-flash-low`、grok `grok-4.6`（三檔都是這支）、OpenRouter 固定一支 :free 模型（關掉智慧切換或釘住；若中途 fallback 或實際 provider 改變，另分一組記）。

1. 每家一張新桌，3–4 輪、輪距 1–2 分鐘（都在 5 分鐘內）。第 1 輪稱「新桌／新線」，不稱供應商冷快取。
2. claude 加跑雙角色桌連講 3 輪：每輪都走角色線抹寫（名字前綴＋機密段），看帳本有沒有 `drop-lane`，有就看 `stage`／`detail` 與 `rewrite-failures/` 留證。
3. 離開提醒：claude 桌最後一輪後不動。節奏是每 3.5 分鐘一次保溫週期、連三次成功後再過 3.5 分鐘才亮提醒＝最快約 14 分鐘加上呼叫耗時；紀錄要超過 8000 字元，不夠就先多講幾輪。截圖留證，帳本應有每週期每條線一筆 `ping`。
4. 每家結束各截一次額度分頁、`invoke usage_report` 存 JSON，對照帳本行與 `usage-raw`：帳本 vs 分頁是否一致、實際出現哪些 `mode`／`cache`／`cache_reason`、`cached_tokens` 是原始欄位的真值還是解析補出來的 0。

### 判讀規則

- 只對本次模型與樣本下結論；沒中不推論成「這家沒有快取」。
- `usage-raw` 缺快取欄位的輪次標「證據不足」，不算真零；發現解析把缺欄補成 0 就另立案。
- 白送底線先記首輪觀測值，不外推成定值。

### 預估花費

| 來源 | 呼叫數 | 量級 |
|---|---|---|
| claude haiku | 單角色 4 輪＋雙角色 3 輪，各輪約 2 通（旁白＋角色）≈ 14 通；保溫 3 週期 × GM＋角色兩條線 ≈ 6 通 | 訂閱額度；折 API 牌價約 $0.1–0.3 |
| codex luna | 4 輪 ≈ 8 通，每通約 1.5 萬 token（含 CLI 自帶前綴約 1 萬） | 訂閱額度，少量 |
| agy flash-low | ≈ 8 通 | 訂閱額度，少量 |
| grok 4.6 | ≈ 8 通 | 訂閱額度，少量 |
| OpenRouter | ≈ 8 通，免費模型 | $0 |

### 產出

對帳報告寫回本檔「四、對帳結果」：本次樣本下哪些線有命中、各狀態的實際數據、首輪底線、證據不足的輪次、抹寫是否重現及原因。標籤與顯示只寫建議，列成「待使用者決定」。

## 四、對帳結果

2026-10-06 01:24–02:12 實跑。CLI：claude 2.1.287、codex 0.160.0、agy 1.2.17；grok 1.0.46 於 2026-10-07 補跑。同一張單角色卡（塞拉菲·内藤），貼開場白 1 後講 3 句＋GM 推進。證據（帳本、AI log、抹寫失敗留證）在本機 `/private/tmp/claude-501/-Users-pachelo-GitHub-Table-Tavern/2f79d440-443a-471c-ab83-e159be2fd267/scratchpad/uca/evidence/`（暫存區，不進 repo），測試 root 已刪。以下只對本次模型與樣本成立。

| 通道 | 樣本 | 原始 usage 的快取欄 | 結果 |
|---|---|---|---|
| claude haiku-4-5 | 角色線 3 輪、GM 線 3 輪、保溫 1 次 | `cache_read_input_tokens`／`cache_creation_input_tokens` 都有 | 角色線 3 輪全丟線（見下），每輪都是新線、讀 0；GM 線續聊正常：第 2 輪讀 6,783／理論 6,793，第 3 輪隔 567 秒仍讀 11,866／11,876；保溫讀 14,159／15,302 |
| codex gpt-5.6-luna | 9 通（3 句 solo＋GM 推進 3 組 shared／solo） | `cached_input_tokens` 有 | 每通恰好讀 9,984＝首輪觀測到的 CLI 自帶底線，我方內容 0；帳本全標 `hit`，分頁頭條「已省 50%」 |
| agy gemini-3.8-flash-low | 角色線 3 輪、GM 線 1 輪 | `cache_read_tokens` 有，累積值也是 0 | 4 輪全 0。續聊確實接上（累積 input 15,313→31,387→48,282，差分＝每輪整段重送）；第 2、3 輪 `cache_reason` 標 `skipped` |
| OpenRouter dots-3-note-preview:free（provider AtlasCloud，全程同一支） | 5 通：3 通完成、2 通卡住 | `prompt_tokens_details.cached_tokens` 有 | 3 通全 0；另 2 通串流超過 5 分鐘沒動靜，app 沒有逾時，只能按停止 |
| grok grok-4.6（2026-10-07 補跑，見下） | 雙角色桌：角色共線 19 輪、GM 線 4 輪 | `modelUsage.*.cacheReadInputTokens` 有（`cacheCreationInputTokens` 恆 0） | 命中率 34%，續聊有一半輪次整線不命中（只讀 128／2688），見「grok 補跑」 |

原始 usage 都有快取欄位，本次沒有「缺欄被補 0」的輪次。

### 角色線抹寫丟線：根因

`session_file::load` 逐行驗證要求每則 user／assistant 的 `parentUuid` 等於前一則 user／assistant 的 uuid。claude CLI 會在兩者之間插入 `type: attachment` 行（2.1.287 每輪都有 environment、model、date、session_context 等多行；2.1.227 已偶發 total_tokens_reminder），這些行帶 uuid 且接在鏈上，第一則 assistant 就驗不過。結果：

- claude 角色線每輪都要補名字前綴，**每輪都丟線**（3／3，`stage: load`，detail「第 13 行 parentUuid 未連到前一條對話 uuid …」），下一輪重開全量。
- 保溫截尾同樣 `load` 失敗，丟 GM 線（`ping-truncate-failed`）。保溫在截尾前已經花掉。
- GM 線不抹寫，續聊不受影響。
- 本機正式 cli-workspace 14 個 session 檔有 5 個驗不過（8/22 起）。正式帳本只有一筆，是因為 9/06 之後沒有 claude 劇情輪。

雙角色桌沒跑：壞在第一步 `load`，機密段相關的步驟根本走不到，多跑只是重複同一個結果。

修法見「一之二」。

### 其他發現

1. **claude CLI 現在寫 1 小時快取**：每筆 `cache_creation.ephemeral_1h_input_tokens` 都是全額，`ephemeral_5m` 為 0；GM 線隔 567 秒仍幾乎全中。因此：app 以 300 秒判「過期」（`CACHE_TTL_SECS`）的前提不成立；保溫 ping 的前提（5 分鐘就過期）也不成立，這次一次 ping 花 $0.072，比一般劇情輪還貴；額度分頁估「省下多少」用 1.25 倍寫入係數，1 小時寫入是 2 倍，省下金額高估。
2. 換新 session 時，凍結 system 相同也不共用快取：角色線三次新開，讀到的都是 0。
3. codex 只中到 CLI 自帶的 9,984，分頁卻顯示綠字「已省 50%」（vendor-prefix-floor 描述的現象，本次重現）。
4. agy 的零命中被標成 `skipped`，但字典對這個值的說明是「claude CLI resume 已知毛病」，套在 agy 上講錯原因。
5. API 串流沒有停滯逾時，免費供應商卡住時一直轉圈（5 通裡有 2 通）。
6. 測試通道視窗不可見時 WebKit 計時器不跑，前端保溫計時器與離開提醒在通道裡驗不了；後端保溫改用 `invoke keepalive_lanes` 直接驗。在 `load` 修好之前，第一次保溫就會丟線，離開提醒本來就亮不起來。

### grok 補跑（2026-10-07 01:07–01:31）

合併 main 後（grok 角色共線＋每桌 lane 鎖）的測試包，新開一桌兩角色（塞拉菲·内藤、林教授｜經濟學），角色輪流講＋GM 推進，輪距 25–160 秒。證據（前後帳本、usage_report、分頁截圖）在 scratchpad `7fa4bcc0-…/scratchpad/uca/`。

- **帳本與分頁一致**：本桌 20 通（中止測試前）帳本加總＝`usage_report`＝分頁：輸入 275,808、讀到 94,464（34.2%）、輸出 7,619、$0.155；分組「該中沒中 ×9、有中 ×8、這次沒有快取 ×2、新桌／新線 ×1」。grok GM 線讀 0 顯示「這次沒有快取」無原因（第 2 項），GM 首輪「新桌／新線」（第 5 項）；舊帳本那筆 grok `skipped` 讀時已變 `zero`。
- **丟線落帳**：17 輪角色共線抹寫 0 次失敗。實測中止：帳本一筆 `drop-lane`、`transport: grok`、`reason: aborted`，下一輪新開線、不再多記。合併前 main 的 grok 丟線有三個錯（既有帳本 4 筆為證）：transport 寫死 `claude`、detail 黏在 reason（`rewrite-failed: 回覆與前綴對不上`，分頁認不得、路徑沒遮）、預定重開（`reopen: system-changed`）被記成丟線。本分支已修：reason 回固定鍵、stage／detail 另欄遮路徑、預定重開不記（原因在呼叫行的 reopen）。
- **角色線首輪讀 128 算成「有中」**：新線讀到的 128 是 xAI 端與內容無關的底線（GM 首輪則是 0、算新桌／新線），同 vendor-prefix-floor 的現象，證據留給該案。
- **整線不命中**：命中的續聊輪中位 93%；不命中的輪只讀 128 或 2,688，比凍結 system 還短。假說：落到沒有這段快取的伺服器（xAI 分流），未證實。本桌角色共線第 9–15 輪連 7 輪不中，第 16 輪沒介入就回到 16,128。

### grok 換 session 自救：本次樣本下暫不做〔模型判斷·未裁決〕

重算腳本 scratchpad `uca/recount.py`，輸入 `uca/ledger-after.jsonl`（10/06 迷霧酒館＋本次全部 grok 帳本）。規則：取 `transport: grok`、有 `lane`、非事件行，依（桌, lane）照帳本順序；讀到 >2,688 算中；帶 `reason`（first-turn、system-changed、scene-changed…）的是新線行。

| 指標 | 規則 | 結果 |
|---|---|---|
| 行數 | — | 新線 18 行中 1（5,248，其餘 ≤1,152）；續聊 59 行中 30 |
| 留原線，下一輪 | 續聊不中的行，下一列是續聊，數那一列 | 6/25 |
| 留原線，兩輪 | 續聊不中的行，之後兩列都是續聊（窗可重疊），數那兩列 | 14/40 |
| 開新線，全部 | 新線行＋下一列（下一列是續聊） | 10/30（下一列是新線也算則 10/32） |
| 開新線，同 lane 換線 | 同上，限非該 lane 第一列 | 6/20 |
| 連敗結局 | 續聊不中的連續段 | 10 段：自己恢復 6（1、4、2、2、1、7），被開新線打斷 3（4、2、3），資料結束 1（3） |

換規則分母就會變（例如窗內允許新線行），而且兩組不是受控比較：開新線多半是換幕、改卡、中止引起，不是挑在不命中時換。這組數字看不出換 session 比留原線好：新線那輪本身幾乎必定全額，同 lane 換線兩輪 30% 也沒高過留原線的 35%。本次只結論為暫不做，不宣稱換線必然無效；等 grok 帳本累積更多再重評，若要評估就做「不中時刻意換線」的受控對照。

### 顯示拍板〔作者裁決 2026-10-06〕

1. 只中到 CLI 自帶底線的輪次頭條不算「已省」，改標「只中到 CLI 自帶部分」——歸 [vendor-prefix-floor](../tasks/vendor-prefix-floor.md)。
2. 有回報、值是 0：顯示「這次沒有快取」，不推測原因；`skipped` 只留給 claude（含修 agy 被誤標 skipped）——本案做。
3. 丟線：分頁維持「丟線重來」一行，`stage`／`detail` 只留帳本——本案不動 UI。
4. claude 1 小時快取：停保溫、過期門檻改 1 小時、省額改 2 倍係數——歸 [claude-1h-cache](../tasks/claude-1h-cache.md)。
5. 首輪說法統一「新桌／新線，本來就沒有可中的」——本案做。

### 主線決定（2026-10-06）

- 角色線丟線在本案修，規格見「一之二」。
- API 串流停滯逾時、CLI 1 小時快取的影響由主線另立案（api-stream-stall-timeout、claude-1h-cache），本檔只留證據；agy `skipped` 誤標併在本案顯示第 2 項。
- grok 已於 2026-10-07 補跑（見上）。

## 五、顯示施工（拍板第 2、5 項）

### 判定規則（寫帳本，[usage/log.rs](../../src-tauri/src/usage/log.rs)）

`classify_cache` 多收 `transport`（`call_fields` 手上本來就有）：

- **續聊線、讀到 0、上輪有可中量**：
  - claude：照舊。讀寫皆 0＝`skipped`；有寫入＝`expired`／`below-expected`。
  - agy、grok：`cache: zero`，**不帶 `cache_reason`**。這兩家不回報寫入量（`created_tokens` 缺），舊碼 `unwrap_or(0)` 把它當「讀寫皆 0」而判成 `skipped`，就是誤標的根因。
- 其餘不變：首輪／重開讀 0＝`not-expected`；中了＝`hit`；中了但不足九成＝`partial`＋原因（數字事實，不屬「值為 0」）；無狀態路徑 0＝`zero` 不帶原因（現況就是）。

### 舊帳本回溯（讀帳本，[usage/report.rs](../../src-tauri/src/usage/report.rs) `classify`）

帳本不改寫，讀的時候套同一條規則：`transport` 不是 `claude` 的續聊線行，`cache` 為 `zero` 時丟掉 `cache_reason`（新行本來就沒有；舊 agy 行的 `skipped` 被濾掉）。效果：舊 agy 零命中從紅燈「該中沒中（claude CLI 已知毛病）」改成「這次沒有快取」，統計格也從 `missed` 移到 `zero`。加 `cache` 欄之前的舊續聊線行（只有 claude）推導照舊，同一條非 claude 濾除規則一併套上。前端 `chipState` 鏡像的是後端判定，不必改。

### 文案（十語系：zh-TW、zh-CN、en、ja、ko、de、fr、es、pt-BR、ru）

| key | 現在（zh-TW） | 改成（zh-TW） |
|---|---|---|
| `usageCacheZero` | 沒省到 | 這次沒有快取 |
| `usageCacheZeroWhy` | 供應商回報這輪一個 token 都沒重用，照全額計費。 | 供應商有回報快取數字，這輪是 0，照全額計費。 |
| `usageCacheNotExpected` | 本來就沒得中 | 新桌／新線 |
| `usageCacheNotExpectedWhy` | 這輪沒有可以命中的前文，不是故障。 | 本來就沒有可中的，不是故障。 |
| `usageReasonFirstTurn` | 這桌第一輪 | 第一次開這條線 |

- `usageCacheReasonSkipped`（「claude CLI 已知毛病」）文案不動，只是不再套到 agy／grok。
- `usageReasonFirstTurn` 一併改：`first-turn` 是「這條線（模型＋角色範圍）沒有舊狀態」，同一桌中途第一次開角色線也是它，「這桌第一輪」講錯；改完配上「新桌／新線」標籤不再互相打架。
- 模組頂註解（log.rs 標籤表、report.rs `chip_state`／`classify`、UsageTab 字典註解）同步改寫成新規則。

### 測試

- log.rs `cache_axis_covers_each_label_by_rule`：agy、grok 續聊線讀 0 且 `created_tokens` 缺 → `(Zero, None)`；agy 帶寫入量也一樣不帶原因；claude 讀寫皆 0 仍是 `skipped`；agy 首輪讀 0 仍是 `not-expected`。`call_fields` 落帳行斷言 agy 零命中沒有 `cache_reason` 欄。
- report.rs：舊 agy 行帶 `cache_reason: "skipped"` → latest 的 `cache_reason` 為 None、統計格是 `zero` 不是 `missed`；同樣的 claude 行仍是 `missed`；grok 行同 agy。
- 前端：新增 `UsageTab.test.tsx`，餵假 `usage_report`：latest＝agy `zero`、無原因 → 細項出現「這次沒有快取」、沒有 claude 毛病那句、不是紅燈；latest＝`not-expected`＋`first-turn` → 出現「新桌／新線」與「第一次開這條線」。
- `npm run verify` 全綠。

### 實測（測試通道）

1. 在獨立 root 的 `data/prompt-cache.jsonl` 預先放幾行舊格式帳本（agy `skipped`、claude `skipped`、claude 首輪 `not-expected`），開額度分頁截圖：舊 agy 行顯示「這次沒有快取」、claude 行照舊紅燈。不花額度。
2. agy `gemini-3.8-flash-low` 開一張新桌講 2 句：第 1 輪「新桌／新線」、第 2 輪「這次沒有快取」且無原因、帳本第 2 輪沒有 `cache_reason`。約 2 通，訂閱額度。
3. grok 見「四、grok 補跑」。

結果（2026-10-06，verify 10 步綠）：
- 舊帳本回溯：預放的 agy `skipped` 行在分頁顯示「這次沒有快取」、黃燈、無原因，統計格「新桌／新線 ×1、這次沒有快取 ×1」；claude `skipped` 行照舊紅燈「該中沒中（claude CLI 已知毛病）」。
- agy 即時：迷霧酒館 GM 線 2 句，第 1 輪 `not-expected`＋`first-turn`，第 2 輪（隔 25 秒）`zero`、帳本無 `cache_reason`；分頁最近一輪「這次沒有快取 續聊 — 供應商有回報快取數字，這輪是 0，照全額計費。」。證據在 scratchpad `uca/evidence-display/`。

### 拍板〔作者裁決 2026-10-06〕

1. claude 續聊線讀 0 保留 `expired`／`below-expected` 原因。
2. API、codex 首通讀 0 不推測新桌，顯示「這次沒有快取」。
