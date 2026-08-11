# Task
Task-ID: refactor-dispatch
Title: AI 重構提速省費：展開並行（時間）＋展開下放檔位（費用）
Status: todo
Created: 2026-08-08T03:35:00+08:00
Updated: 2026-08-08T03:35:00+08:00

## Summary
2026-08-08 討論立案。重構管線（盤點 1＋逐條展開 N＋收尾 1）現況全走 GM 檔且序列執行，西幻卡約 17 次呼叫、分鐘級等待。兩個獨立改動，全傳輸通用（CLI 與 API 直連皆受益）：

1. **並行展開省時間**：各條展開彼此獨立，序列 await 改有界並行（4–6 路），分鐘級壓到一分鐘上下。survey 先行已把 system 寫進 prompt cache（[refactor_ai.rs:6](../../src-tauri/src/refactor_ai.rs#L6) 三段共用同一份 system），並行展開全部命中，快取結構不用動。
2. **分檔位省費用**：survey 留 GM 檔（認人／分類是智力核心）；人物／介面／機制展開下放 balanced（單價 −40%）；finish 下放 fast（只判殘渣可否刪，−80%）。展開佔呼叫數九成且輸出是費用大頭，訂閱玩家的 rate-limit 視窗消耗同比例縮。混檔代價只有每檔位各多一次 cache write。

「模型內部自派子代理」評估後不採：子代理重建脈絡 token 更多、拆掉逐條解析與取消保底、CLI 須開工具讓不可信卡片內容接觸有工具的代理、且僅 claude CLI 支援。

## 設計要點
1. 並行保留既有語意：取消＝不發新的、在途跑完照樣進結果卡；單條失敗略過列名；進度字改「完成 x/n」。
2. 檔位下放點：`refactor_survey`／`refactor_expand`／`refactor_expand_person`／finish 目前都取 `transport::gm_tier`（[lib.rs:554](../../src-tauri/src/lib.rs#L554)、583、617、674），改為依呼叫類型給檔位；未設定的檔位退 GM 檔（同 opening-translation 慣例）。
3. 機制展開（EJS→JSON 契約）是下放品質最敏感點，實測不行單獨升回 balanced→best。
4. CLI 路徑並行＝同時多個 process，遇 429 退避；並行上限先取保守值。
5. **取消改成可中止在途呼叫**（2026-08-11 Dark Wolf 實測補立）：現行「在途跑完才停」對盤點不成立（第一條被取消＝整輪作廢，跑完純燒額度）；並行下取消＝殺全部 in-flight 子程序。順帶處理 app 退出的孤兒子程序（現況 Cmd-Q 後 CLI 呼叫繼續跑完）。

## Next action
- 未排程。**前置：[ai-card-refactor](ai-card-refactor.md)＋[person-promote](person-promote.md) A–E 實測結案**（2026-08-08 拍板）——先建立現行管線的品質與時間費用基線，下放檔位後才有得比。
- 開工點：前端 runAiRefactor 序列 await 改有界並行；後端四個 refactor command 的檔位參數化。

## Constraints
- survey／expand 共用 system 逐位元組相同的快取紅線照舊，不碰 system 組裝。
- 下放檔位後的展開品質須與實測基線 A/B 比對後才定案。
