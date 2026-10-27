# agy 線路形狀與 api-shared-lane 規格不符

Status: 已結案（2026-10-28，純文件）

查證結果：agy 續聊一角一線是刻意改的（2026-09-09，5d2177ad），之後 grok-shared-lane、worldbook-st-trigger-parity 都以此為前提，不是回歸；實測佇列第 14 項 cached 全 0 是 prompt 只有 8.7k–12.5k，低於 agy 觀測到的快取門檻（約 16k–20k，推論），與線路形狀無關。維持一角一線，只同步 [api-shared-lane 規格](../../plans/api-shared-lane.md)「現況」節、該案交接與實測佇列第 14 項。
