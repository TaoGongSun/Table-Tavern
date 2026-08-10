# Task
Task-ID: refactor-outcome-export
Title: 重構產物匯出：重構一次，之後重玩同一張卡免再燒額度
Created: 2026-08-10T22:30:00+08:00
Updated: 2026-08-10T23:30:00+08:00
Status: in_progress

## Summary
玩家花額度重構一張卡後，產物只活在當下結果卡的 state 裡（[App.tsx:1794](../../src/App.tsx#L1794)），套用或關掉就丟了；重玩同一張卡得再燒一次 AI。本功能把產物（RefactorOutcome JSON）存檔可匯出，重玩時匯入直接進人審面板。

匯入端已是正式功能：世界書分頁「匯入重構卡」（[App.tsx:2419](../../src/App.tsx#L2419)）＋逐欄驗證（[refactor-review.ts:198](../../src/refactor-review.ts#L198)），測完不刪。玩家面用詞統一叫「重構卡」（2026-08-10 拍板）。

## 拍板結論（2026-08-10）
1. **本任務先做、實測順序反轉**：手工捏假產物等於用人力重造 app 按一顆鈕就會產的東西，成本高又測不出真實情況。[ai-card-refactor](ai-card-refactor.md) 實測改成先跑 B1 真 AI、匯出真產物，再用它跑 A 段；重測不必再燒額度。
2. **兩個匯出入口都做**（一次做完省暖機）：結果卡摘要頁「匯出」鈕＝套用前存檔；套用時另自動把產物落檔 `worlds/<id>/refactor-outcome.json`，世界書分頁「匯入重構卡」旁「匯出重構卡」鈕隨時可掏。
3. **落檔存全量**（含沒勾的人）：重玩改主意才有得選；與結果卡匯出檔同格式零分叉。
4. **undo 不刪產物檔**：「復原上次匯入」只回退套用效果，產物檔留著供重勾再套；收據不記檔。二次套用覆寫不留上一版（同殼檔慣例）。
5. **含 HTML 殼**：殼是 `interface.shell` 欄位，序列化自然帶上，匯入端已會讀；不含反而要多寫剝除碼。
6. **uid 對不上 v1 不處理**：同桌重匯必中、同 PNG 重建桌 uid 由卡帶入通常一致；現行為查不到顯 uid 兜底、套用靜默略過，不炸。跨桌防呆等功能對外再談。

## 待辦（2026-08-10 B1 實測回饋）
- **匯出檔名前置桌名**：兩個入口的 saveDialog defaultPath 目前固定 `refactor-outcome.json`，存多張卡認不出來；改成「{桌名}-重構卡.json」（桌名含檔名非法字元時的處理實作時定）。等使用者通知與 [refactor-stream-progress](refactor-stream-progress.md)／[refactor-review-detail](refactor-review-detail.md) 同批開工。

## Next action
實作完成、四項驗證全綠（2026-08-10）；B1 真跑 orc-cave 已完成、產物已匯出。剩：[ai-card-refactor](ai-card-refactor.md) A 段用真產物實測（A4／A7 驗本功能）＋上方待辦一項。

## Constraints
- 匯入是信任邊界：玩家自己選的檔一律逐欄驗證後才進面板。
- 產物一律人審後套用（沿用重構紅線），匯入不得跳過勾選畫面直接落檔。
