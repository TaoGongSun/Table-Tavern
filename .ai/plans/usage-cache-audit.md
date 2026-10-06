# usage-cache-audit 規格

交接檔：[handoffs/usage-cache-audit.md](../handoffs/usage-cache-audit.md)。

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
3. 刪檔失敗那筆（`settle_abort` 的 abandon 錯誤）維持現狀。不加重試、不改丟線策略。
4. 測試矩陣：
   - load：session 檔不存在。
   - find-segment：機密段不在檔內；機密段出現在兩行 user。
   - erase-segment：同一 user 行內機密段出現兩次（find 命中一行、erase 判兩次）。
   - prefix-assistant：最後一則 assistant 第一段不是 text。
   - write、truncate：用 helper 的可控失敗注入驗 stage 映射，不改正式策略；真實 I/O 寫入失敗（父目錄唯讀）只在 Unix 平台另留一條。
   - 遮罩：含空白的 POSIX 路徑、`C:\Users\a b\...` 形式的 Windows 路徑、遮完才截斷（多位元組字元不被切半）。
   - lanes 既有抹寫失敗 e2e 改斷言帳本帶 `stage`。

已知可疑點（實跑時留意，不預先修）：`session_file::load` 逐行驗證要求 user 行 content 是字串，CLI 若改寫成陣列會整檔讀失敗＝`load`；CLI 若正規化換行或尾端空白，機密段逐字比對落空＝`find-segment`。

## 二、測試包量測掛點（只在 `test-harness` feature）

正式包不含；沿用 AI log（`<root>/harness-ai.log`）的 dispatch id 串起呼叫。

1. **原始 usage**：各家解析收尾 usage 的位置多寫一行 `usage-raw`，帶同一 dispatch id、原始收尾事件的 usage 物件原樣（缺欄就是缺，不補 0）、實際回應模型；API 另帶 OpenRouter 回的 `model`／`provider`；agy 同時記原始累積值、上輪基準與算出的本輪差分。
2. **抹寫失敗留證**：抹寫或截尾失敗時、在丟線與 `abandon_session` 刪檔之前，把當下的 session 檔整份、要抹的機密段與名字前綴、stage／detail 寫進 `<root>/rewrite-failures/<時間>-<唯一 ID>-<安全化線名>/`（線名如 `chars:<model>`，模型可能含 `/`，非英數字元一律換 `_`），原線名寫在目錄內 metadata。留證失敗不阻止原本的清線／刪檔，但在 AI log 記一行 `rewrite-evidence-failed`。

CLI 版本在實跑前用各家 `--version` 記下；claude 另以 session 檔逐筆 usage 對照帳本。回應沒給實際模型／provider 的欄位記「未回報」，不拿請求值冒充。

## 三、實跑對帳

### 環境

- `npm run harness:build`，`env -u ANTHROPIC_BASE_URL node scripts/harness.mjs launch --fresh --config-from <正式 config.json> --root <本案專屬 root>`。帳本落在 `<root>/data/prompt-cache.jsonl`，正式帳本只讀。
- 測試通道全機一把鎖（`harness/root.rs` identifier 鎖），同時只能一個實例；與其他子代理輪流，啟動前 `ps`／`status` 確認，跑完立刻 `quit`。
- Grok 登入：正式 config 目錄的 `grok-home`、`cli-home` 複製進 `<root>/config/`，跑完刪〔作者裁決 2026-10-06〕。**Grok 額度 2026-10-06 22:00 才恢復，之前不對 grok 送任何請求**〔作者裁決 2026-10-06〕。
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
| grok 4.6 | ≈ 8 通（22:00 後） | 訂閱額度，少量 |
| OpenRouter | ≈ 8 通，免費模型 | $0 |

### 產出

對帳報告寫回本檔「四、對帳結果」：本次樣本下哪些線有命中、各狀態的實際數據、首輪底線、證據不足的輪次、抹寫是否重現及原因。標籤與顯示只寫建議，列成「待使用者決定」。

## 四、對帳結果

（實跑後填）
