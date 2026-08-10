# Task
Task-ID: refactor-stream-progress
Title: 重構進度小框：modal 固定 3–4 行 tail 播放 AI 字流
Created: 2026-08-10T23:59:00+08:00
Updated: 2026-08-10T23:59:00+08:00
Status: todo

## Summary
重構 modal 目前只有一行靜態字（「盤點中…」「整理『X』i/n」），盤點大卡（orc-cave 55K 字元級）全程黑箱，玩家不知道 AI 是否還活著。拍板做法：modal 加一個固定 3–4 行高的小框，tail 式滾動播放 AI 當下輸出的字流——玩家只需要知道 AI 在工作，不求逐字可讀（盤點／機制段輸出是標記格式與 JSON，半可讀沒關係）。

## 拍板結論（2026-08-10）
1. **只做小框 tail**：固定 3–4 行、顯示最新輸出、自動滾動。不做展開階段累積清單、不做聊天室訊息流（重構是偶發操作，訊息留歷史反而要清）。
2. **時程**：前置＝orc-cave 卡重構實測完成（使用者通知後開工）；實作外包代理。

## 技術入口（發包時展開成規格）
- 底層已是串流：`refactor_survey`／`refactor_expand`（lib.rs）走 `stream_via_transport`，與 GM 對話同一條傳輸路；缺的只是把 delta 經 Channel 轉發前端。
- 前端接點：`runAiRefactor`（[App.tsx:2082](../../src/App.tsx#L2082)）與進度 modal（[App.tsx:2545](../../src/App.tsx#L2545)）；Channel 先例照 `gm_narrate` 呼叫端。

## Next action
等使用者通知（orc-cave 重構實測完成）→主線出規格發包代理→四項自驗（cargo／vitest／build／i18n）→實機看盤點階段字流有動。

## Constraints
- 取消語意不變：擋下一條、當前條跑完。
- 小框只是顯示，不落檔、不進對話歷史。
