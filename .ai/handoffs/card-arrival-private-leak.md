# 角色卡回歸事件把私設漏給同桌其他角色

Status: awaiting-verification。規格與拍板見 [plans/card-arrival-private-leak.md](../plans/card-arrival-private-leak.md)。

## 現況
已進 main（2026-10-02，Sol 兩輪審查與驗收通過；verify 全綠 vitest 421、cargo 711）。
- 回歸時私設另成一則 `gm_only` 的「（角色私設）〈名〉」事件（先寫），公開回歸事件後寫。
- 角色側單一入口 `transport::character_visible_text`／`character_events`：角色線、api／codex 共線、換幕摘要（所有 gm_only 只留第一行）、角色側 keyword 觸發都走它；GM 一律原文。
- 角色線 `LaneState.redaction`：已送段含舊合併事件的三家角色線重開一次（額度分頁原因「舊回歸事件遮掉私設」）。
- 已知限制：已經生成的換幕摘要若寫進了私設，無法辨認修復。

## 下一步
實機已過（測試通道 2026-10-27，claude CLI Haiku 5.5）：NorthHall-structure 重構桌，秦岚加私設後設 auto_hidden，GM 回覆 present 帶到她→逐字稿先私設（gm_only）再公開回歸兩則；周掌柜、贺铮角色回合後角色線 session 檔零私設字樣、只見「（角色回歸）〈秦岚〉」前綴，GM 線 session 有私設且下一輪續用同一條線。可結案。
