# 角色台詞照抄狀態標籤會原樣顯示

Status: done。已合併 main（2026-10-11）。規格、拍板與測試清單見 [plans/char-line-status-strip.md](../../plans/char-line-status-strip.md)。網頁版公開前門檻〔作者裁決 2026-10-10〕。

## 現況
實作與三方驗收完成，測試通道實測通過；四項拍板暫取 A〔模型判斷·未裁決〕，作者尚未確認。
- 收尾純函式 `transport/reply_cleanup.rs`（`finish_character_reply`、`final_reply_text`、自閉合移除、未閉合尾巴）；角色兩條路、GM 中止、續聊對帳、網頁存檔匯入都走它。
- Claude／Grok 共用角色線抹寫時把本輪回覆換成「前綴＋收尾台詞」；只有控制區塊的完成回合比照失控收尾後回 `AI_EMPTY_RESPONSE`。
- 前端串流截斷 `features/play/stream-display.ts`（旁白與台詞共用）；台詞落檔帶 raw。
- 對拍案例 `src/shared/contracts/reply-cleanup/cases.json`。
- 既有限制（非本案引入）：Claude 抹寫的「只補前綴」路徑要求最後一則 assistant 第一個分段是 text，thinking 在前就抹寫失敗丟線（`session_file.rs` `prefix_last_assistant`；真 CLI 把 thinking 與 text 拆成兩行，目前沒遇到）。

## 測試通道實測（2026-10-11，api 傳輸＋本機假 OpenAI 相容端點，新桌兩張自製角色卡）
- 兔子回「台詞＋換行＋`<UpdateVariable>`」：畫面只見台詞；逐字稿 text 乾淨、raw 是原文。
- 狐狸回「`<UpdateVariable>`＋換行＋狐狸：台詞」：畫面與 text 都是「台詞」，raw 保留標籤、不帶「狐狸：」。
- 狐狸那輪與下一輪 GM 推進送出的提示詞都含兩句台詞、沒有任何 `UpdateVariable`。
- 只有前綴＋標籤的回覆：跳「這一輪沒能完成」（`AI_EMPTY_RESPONSE`），逐字稿沒落東西；關掉再請發言即正常落檔。
- 卡片介面讀 raw：這張桌沒有介面卡，沒測到（`card-shell-route` 的 `floorText` 讀 raw 已有單元測試）。
- CLI 續聊線的共用 session 換寫只有假 CLI 的整合測試，沒用真 CLI 跑。

## 下一步
作者確認四項拍板（plans 的「待作者拍板」）；改選 B 的項目另立案。
