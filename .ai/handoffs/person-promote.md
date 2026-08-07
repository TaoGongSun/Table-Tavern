# Handoff: person-promote

## Current state
實作完成、主線審查通過、四項自驗全綠（見 Verification）。剩實機驗收——與 [ai-card-refactor](ai-card-refactor.md) 的待實測清單 A–E 合併跑（2026-08-07 拍板）。

## Completed
- 盤點認人（refactor_ai.rs SURVEY_BODY:132）：單人條目也列、同一人跨條歸組（輸出 name＋uids＋player 標記）、排除勢力／組織／職階、多語言版本挑接近玩家語言的一版；解析成 RefactorSurveyPerson{name, uids, is_player}。
- 展開一人一次呼叫（person_expand_messages:319）：帶該人全部來源條目全文，只挑他的段落。
- 收尾階段（FINISH_BODY:344、command refactor_finish_shared）：共用合集條目「整條刪或整條留」二元判定——比規格要點 7 的「移除段落再判」更保守，不產生任何 AI 改寫內容；殘渣才刪、沒把握原樣留（規格保底的常態化）。
- 套用（refactor.rs apply():119）：勾選的人合併升格成單卡；專屬條目刪除走 receipts（deleted_entries 無條件復原）；玩家卡指定沿用一桌一張限制，undo 退回 player_card_assigned（receipts.rs:92、290）。
- 前端：勾選畫面玩家單選列（可不選）；預設全勾維持；手動「轉成角色卡」的 `{{user}}` 對話框移除（App.tsx:2235，一律 asPlayer:false）；i18n 十語系同步（刪 convertEntryPersonaAsk/Ok/Cancel，增 refactorFinishing 等 5 鍵）。
- 快取契約：system 逐字元相同測試擴增涵蓋 person_expand／finish（refactor_ai.rs:1068）。

## Verification
- 主線複驗（2026-08-08 00:45）：cargo test **422 綠**（基線 411）；vitest **71 綠**（基線 55）；npm run build 成功；check:i18n 十語系 OK。
- 主線抽查：SURVEY_BODY 對規格六項全中；convertEntryToCharacter 已無對話框；receipts 含玩家卡回退與整條復原。
- 分工：opus subagent 實作（194 tool uses）＋主線 Fable 5 審查複驗。

## Remaining
- 實機驗收（等使用者）：匯入三張測試卡實跑——HeroTrainingUnderSide_stats（15 人合併、概览不再拆薄卡）、NorthHall（霍玄一人一卡＋玩家指定只問一次）、orc-cave-copy（Rigurd 中英只出一人）；undo 一鍵倒退含刪除條目與玩家卡。
- 與 ai-card-refactor 交接檔「待實測清單」A–E 合併驗收，全過兩案一起結案搬 DONE。

## Next action
使用者實機驗收上列項目；期間發現問題開新對話帶本檔即可接手。

## Constraints
- 盤點 system 提示詞逐字元相同才吃得到快取，改輸出格式只動 user 訊息那半。
- 刪條目一律走 receipts。
