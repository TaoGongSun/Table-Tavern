# non-claude-real-cache — 無續聊路的快取

## 結論（2026-10-28，三方共識）

**目前沒有適用的端點，撤銷原施工方案**（`CacheStrategy::StrictExtension`、尾巴重播進歷史、chain epoch 三包）。api 路徑組裝不動。

- 唯一驗證過的嚴格延續型 `stealth/ox-alpha` 已從 OpenRouter 下架（2026-10-28 模型清單 464 個，無 `stealth/`）。
- 現行組裝已是一般前綴型的標準形狀：靜態在前、動態在尾、每輪 append。離線量測相鄰兩輪斷點全落在上一輪的尾巴；免費模型裡會快取的兩家都照樣中到共同前綴（下節）。
- 尾巴重播與共線隱私衝突：多角色桌角色共線的尾巴含本輪角色私設，重播進歷史會讓下一輪別的角色讀到。**日後再遇嚴格延續型端點，也要先重審隔離方式（例如一角一線或只重播公開段），不可直接復活原方案。**
- agy 自 2026-09-09 走 lane 續聊，不屬每輪重送的路。codex 單發的我方內容零命中與排版無關，解法是續聊（另案 codex lane resume），數據見下。

## 驗證證據與範圍

### 離線 byte-LCP（測試通道＋本機錄製端點）
- 迷霧酒館範例桌（3 卡，GM system 891 字）＋3 條 keyword 世界書，假回覆不含狀態區塊；只錄 api 路徑 body（模型字串 `deepseek/deepseek-v4-pro`，非 anthropic 分支）。
- GM 線相鄰 7 對、角色共線 4 對（含換角色）：斷點全落在上一輪尾巴那則（動態塊／導演指示／私設），之前逐位元組一致；連續旁白經 `push_merged` 併成同一則，位元組前綴仍延續。
- 未涵蓋：換幕摘要後那輪、undo／收回後那輪、真實大桌、長間隔；樣本小、間隔短。

### OpenRouter 免費模型（2026-10-28，未儲值 key，約 32 次）
- 16 個 `:free` 裡能打的 6 個；只有 `nvidia/nemotron-3.5-super-vl`（Nvidia，4,336 一格）與 `dots-studio/dots-3-note-preview`（AtlasCloud，4,096 一格；作者帳本另見 2,048 命中，未定）會快取，兩家都是**一般前綴型**：
  - A＝一輪；B＝同歷史、尾巴換掉（app 現形態）；C＝B 的嚴格延伸。Nvidia A 0／B 8,672（10,145 中）／C 8,672；AtlasCloud A 0／B 8,192（8,611 中）／C 0。
- Nvidia ultra、Poolside、Cohere 回 `cached_tokens: 0`：OpenRouter 對不快取的端點也回 0，「回報 0」不代表該家有快取。
- 測試通道實跑 nemotron 6 輪：帳本 `cache_reporting: reported`、`cached_tokens` 解析正確，7k–8k 輸入中 4,336。
- **不外推**：只代表這兩個免費端點；Alibaba 等走顯式 `cache_control` 的、Gemini 隱式快取都沒測。

### 歷史證據：ox-alpha（2026-08-22，保留）
`stealth/ox-alpha` 只在「上一次請求是這一次的逐字前綴」時給快取。真實桌 body 重放、間隔 60 秒：尾端動態塊逐輪換掉 A1 7,453／64 → A2 8,124／64（1%）；嚴格前綴 B1 9,380／64 → B2 9,522／8,000（84%）。當時排除過的假說：缺 `session_id`、請求形狀與串流、前綴被打散（LCP 97.8–99.0%）、傳輸層、單輪長輸出。措辭只宣稱「該端點呈現嚴格延續行為」。

## codex 數據（供 codex lane resume 新案引用）

codex-cli 0.164.0、CLI 預設模型、範例桌：
- **單發 `exec --ephemeral`**（現行）：測試通道 5 通、直接重放（含同一份 stdin 連打兩次，每通只有一次模型請求）`cached_input_tokens` 全是 12,288，冷熱一樣——我方內容從未命中。
- **`exec resume` 一條線 4 輪**（逐輪值取自 session 檔 `last_token_usage`）：

| 輪 | 輸入 | 讀到快取 |
|---|---|---|
| 1（開線） | 16,161 | 12,288 |
| 2 | 17,465 | 16,000 |
| 3 | 18,108 | 12,288 |
| 4 | 23,214 | 18,304 |

- resume 時 `turn.completed` 的 usage 是整條線**累計**，逐輪要減上一輪（同 agy 的 usage base）。
- 第 3 輪掉回底線，兩個假說未分辨：路由沒對上，或上一輪快取還沒寫好（第 2 輪結束到第 3 輪送出只隔約 2.5 秒）。
