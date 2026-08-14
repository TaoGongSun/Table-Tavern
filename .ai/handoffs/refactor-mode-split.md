# Task handoff
Task-ID: refactor-mode-split
Updated: 2026-08-14T05:36:56.599616+00:00
Status: in-progress

## Goal
重構雙軌定向落地：照 .ai/plans/refactor-mode-split.md 四包完成（路由＋三態偵測＋二選一 UI／兩段 session／模式行為／驗收矩陣）。

## Current state
2026-08-14 開工輪被使用者中止：主線花 15 分鐘推實作規格未發包（違規，已記 lessons.md）。補救已落檔——底稿新增「實作定案」段（mode 欄位鏈、三態判定、rule 5、fallback 抑制、UI 兩層互動全部定案）與「開工（兩分鐘程序）」段：下次說「開工」＝checkpoint＋直接發包，禁再推規格。

## Completed
- 底稿補「實作定案」與「兩分鐘開工程序」段（.ai/plans/refactor-mode-split.md）。
- 任務檔轉 in-progress。

## Verification
- 尚無程式改動。各包驗證＝cargo test／npm test／npm run build／npm run check:i18n 四件套＋主線逐條對驗收。

## Plan files
- .ai/plans/refactor-mode-split.md

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main
- HEAD: 76218d8cd1bc792244e11a54b46723cd0d18efa4
- Dirty: false
- Dirty fingerprint: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945

## Remaining
- 包 1–3 發包（照底稿開工段程序）；包 3 前主線出判官提示詞正文；包 4 實跑歸使用者。

## Next action
下次開工：handoff checkpoint 後兩分鐘內照底稿「開工」段發包 1——先解決環境陷阱：`claude -p --dangerously-skip-permissions` 被會話 classifier 擋，需使用者加 Bash 允許規則或當場核可。

## Constraints
- 開工輪禁止規格推導、禁重讀 codebase（2026-08-14 使用者裁決）；實作細節歸執行者，主線只發包＋收貨。
- 有介面卡一律問玩家；none 直通角色線；unsupported 擋下。
- 兩段判官同檔位、獨立短命 session；resume 失敗降級重送全卡。
- 模式持久化；角色優先停用介面 fallback；匯出／匯入保存 mode；稽核 mode-aware。
- 快取紅線：survey／expand 共用 system 逐位元組相同零觸碰。
