# agy 跑長篇重構盤點多半失敗

Status: 已結案（2026-10-28 合併 main）

agy 長篇盤點失敗的三個根因（模型叫工具被無頭模式擋、思考期間無輸出撞 120 秒斷流、印完 result 後卡在標題產生不退出）已修：精簡工具集代理（`--agent table-tavern-text`）、成功收尾後 10 秒寬限強殺、首個進展前 300 秒保底〔作者裁決 2026-10-28：選 A〕、grok `end` 依官方 stopReason 分類。根因、做法、實測與已知限制（agy 每條新對話多一次標題呼叫、關不掉）見 [plans/agy-long-survey-fail.md](../../plans/agy-long-survey-fail.md)。
