# Handoff: person-promote

Status: 已結案（2026-10-27 合併 main，與 ai-card-refactor 一起；梯 2 第 6 項真桌實測全過）

## Current state
實作完成、主線審查通過。2026-10-27 實機驗收（test-harness，Sonnet 5.5，與 [ai-card-refactor](ai-card-refactor.md) A–E 合跑）本案項目全過；整體結案仍等 ai-card-refactor 的三個紅修完。

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
實機驗收結果（2026-10-27；露骨卡改用作者裁決另做的乾淨卡，NorthHall.png 已撤出改用 NorthHall-structure）：
- [x] 15 人合併（`TestCards/clean-hero-roster.json` 代 HeroTrainingUnderSide_stats）：16 張＝15 人＋{{user}}；角色概览 uid35 沒拆薄卡，只在概览出現的名字沒建卡；英雄分支自動指認 15 支。
- [x] 一人一卡＋玩家只問一次（`NorthHall-structure.png`）：8 張，每人兩條來源併一張，只有林远有「這是我的角色」。
- [x] 跨語只出一人（`clean-orc-cave.json` 的托伦代 Rigurd）：中文＋行為模式兩條併一張；EN 版在卡內是停用條目，進淘汰、沒另成一人。
- [x] undo 一鍵倒退（含刪除條目與玩家卡）：見 ai-card-refactor A4。
- 觀察：同一張卡兩次重構，人名字形不一致（吉爾／吉尔），與 ai-card-refactor 紅 1 同源風險。

## Next action
等 ai-card-refactor 紅 1／2／3 修完、補跑過後，兩案一起結案。

## Constraints
- 盤點 system 提示詞逐字元相同才吃得到快取，改輸出格式只動 user 訊息那半。
- 刪條目一律走 receipts。
