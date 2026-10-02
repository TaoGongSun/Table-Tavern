# 角色卡回歸事件把私設漏給同桌其他角色

Status: awaiting-verification。規格與拍板見 [plans/card-arrival-private-leak.md](../plans/card-arrival-private-leak.md)。

## 現況
已進 main（2026-10-02，Sol 兩輪審查與驗收通過；verify 全綠 vitest 421、cargo 711）。
- 回歸時私設另成一則 `gm_only` 的「（角色私設）〈名〉」事件（先寫），公開回歸事件後寫。
- 角色側單一入口 `transport::character_visible_text`／`character_events`：角色線、api／codex 共線、換幕摘要（所有 gm_only 只留第一行）、角色側 keyword 觸發都走它；GM 一律原文。
- 角色線 `LaneState.redaction`：已送段含舊合併事件的三家角色線重開一次（額度分頁原因「舊回歸事件遮掉私設」）。
- 已知限制：已經生成的換幕摘要若寫進了私設，無法辨認修復。

## 下一步
實機（排在[實測佇列](../reference/verification-queue.md)梯 2）：自動隱藏一張有私設的卡→GM 回覆讓牠回來→逐字稿出現私設＋公開兩則；換別的角色發言，診斷或 session 檔看不到私設；GM 下一輪仍用得到私設。過了就結案。
