# card-arrival-private-leak — 角色卡回歸事件不再漏私設

## 拍板〔作者裁決 2026-10-02〕
- 回歸事件拆兩則：公開回歸事件給所有線；私設另成一則 GM 專屬事件。
- 舊桌已寫進逐字稿的合併事件：讀取時遮掉私設段，不改檔。
- grok（與 agy）維持一角一線＋私設提進該角色凍結 system：grok 對話檔沒有回合後抹寫路徑，共線會讓私設永留共用歷史。

## 做法
1. `data` 新增 `CARD_PRIVATE_PREFIX = "（角色私設）"`。`transport::card_arrival_text` 只產「（角色回歸）〈名〉＋公開設定」；新增 `card_private_text(card, user) -> Option<String>`（私設空就 None）。
2. `record_card_arrivals` 每張卡先 append 公開事件（`gm_only: false`），有私設再 append 私設事件（`gm_only: true`）。`appeared_titles` 只認 `CARD_ARRIVAL_PREFIX`，私設事件不影響回歸判定與換幕結算。
3. 角色側渲染單一入口：`system_event_text(event, redact)` 在 redact 時
   - `gm_only` 且以 `CARD_PRIVATE_PREFIX` 起首 → 整則不出現（`lane_event_line`／`assemble_shared_messages` 跳過該事件，不留空行）；
   - 其餘 `gm_only` → 照舊只留第一行；
   - 以 `CARD_ARRIVAL_PREFIX` 起首且含 `\n私有設定：` 的舊合併事件 → 截掉該行以後。
   GM 線一律全文不變。
4. 已開著的 claude 角色線：舊合併事件可能已經在 session 歷史裡。`LaneState` 加 `#[serde(default)] redaction: u32`；Chars 線 `redaction < 1` 且已送段（`events[..base]`）含舊合併事件 → `Reopen{HistoryRedacted}` 一次，重開後寫入 `redaction = 1`。不含舊事件的線不重開，不白燒快取。
5. 測試：拆兩則落檔、各線渲染（chars 不見私設與私設事件、GM 全文）、舊事件遮罩、`appeared_card_names` 不重複計、lane 遷移只在含舊事件時重開一次。

## 驗收
- `npm run verify` 全綠。
- 實機（併入實測批）：自動隱藏一張有私設的卡→GM 回覆 present 帶到牠→逐字稿出現公開回歸＋私設兩則；切別的角色發言，`prompt-cache` 診斷或 session 檔裡看不到私設；GM 下一輪仍能用到私設。
