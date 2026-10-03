# Task handoff
Task-ID: refactor-mode-split
Updated: 2026-08-14T09:24:48.977395+00:00
Status: awaiting-verification

## Goal
重構雙軌定向落地：介面優先 vs 角色優先（兩段式選擇＋模式專屬解析），四包完成（路由＋三態偵測＋二選一 UI／兩段 session／模式行為／穩定性驗收矩陣）。

## Current state
程式面四包＋2026-08-14 GUI 回歸四洞修復（d8f8f1c）都在 main。2026-10-02 用 test-harness 測試包（Claude CLI claude-sonnet-5-5，正式資料零差異）跑完五卡矩陣：
- WestFantsy 連跑 3 次：皆完成、可套用、可載入，產物一致（11×7、23 佔位符、五棵狀態子樹、mode=interface）；第 3 次殼多包一層外框，屬變異。
- Transfur：完成，接管（有殼）。
- bcd368：完成，判 playable: no，沒建殼。
- TrainEmperor：送 AI 前擋下，零派送。
- NorthHall：Sonnet 5.5 以內容理由在初判拒答，已撤出測試流程、不再使用；改用 `TestCards/NorthHall-structure.png`（同結構、成年非情色內容），判 playable: no，沒建殼。
- 取消路：初判後在二選一框取消，無產物落地；重構進行中取消未測。
- 第二段 resume：同一 CLI session，盤點快取命中 83～89%（小卡 59%），展開 69～97%。
- 沒建殼的桌（bcd368、NorthHall-structure）改成不給面板，已由 [refactor-noshell-panel](archive/refactor-noshell-panel.md) 結案。

## Completed
- 包 1：三態偵測＋二選一對話框＋unsupported 擋下＋refactor_recommend／survey 帶 mode＋i18n。
- 包 2：refactor_session.rs 短命 session（開線／resume＋指紋核對＋降級單發）。
- 包 3：模式專屬提示詞＋MODE 回聲核對＋mode-aware 稽核＋refactor_mode 持久化＋介面 fallback 抑制＋匯出入保 mode。
- GUI 回歸四洞修復（d8f8f1c）：apply 停用整條 dropped＋rewritten_entries 快照；effective apply_interface 閘門（含來源消耗判定）；normalize_interface_paths 正規化（鏡像分支折疊／值合併／rules remap／dangling 剔除，preflight 在任何寫入前）；INTERFACE_STATE_RULES 加單一路徑鐵則；worldbookMessage 置頂。

## Verification
- cargo test 500（+6：characters 跳介面留來源、整條 dropped 停用＋undo、NorthHall 縮小版折疊、衝突與雙綁拒套、無殼鏡像判定、preflight 零落檔）；vitest 137；npm run build ✓；check:i18n 十語系 OK。
- 未實機：四洞修復後的 GUI 重測＋包 4 矩陣剩餘項（實跑歸使用者）。

## Plan files
- .ai/plans/refactor-mode-split.md

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main
- HEAD: d8f8f1c7a615dcde35912417b1882834e90e6c87
- Dirty: false
- Dirty fingerprint: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945

## Remaining
- 四洞①②④ GUI 重測：A 桌（characters）套用後「格式」「COT」掛停用徽章、GM 出對話正文；C2 匯入後 state.json 無介面狀態樹、無 incremental；E1 擋下訊息出現在按鈕列正下方。
- 四洞③「重跑重構後面板佔位符更新」：2026-10-03 在 refactor-statusbar-skeleton 測試包驗——NorthHall-structure 第一次重構已產殼（狀態欄型），同桌第二次重構因介面來源條目在第一次套用時已被消耗，產物沒有介面，套用後殼被清掉、面板消失（現場重構沒有收據、無法 undo）。③ 的原始情境（同桌重跑時還找得到介面來源）在現行「消耗即刪除」規則下已不會發生；重跑清殼的處理待作者裁決，見 [refactor-statusbar-skeleton](refactor-statusbar-skeleton.md) 待問 1。
- 重構進行中取消。
- 驗完刪 lib.rs `[survey-persons]` eprintln（診斷水印，順路）。

## Next action
用測試包重測四洞①②④與重構中取消；③ 等 refactor-statusbar-skeleton 待問 1 裁決後再定還要不要驗。

## Constraints
- 快取紅線：survey／expand 共用 system 逐位元組相同（既有測試把關）。
- 舊產物（無 mode）＝照 interface 行為；判官 drop 仍限 rule 1–4，rule 5 只由 app 構造。
- 正規化只認精確別名 W.p↔p 不採相似度；衝突一律拒套不猜；原始 refactor-outcome.json 永不改寫（救援證據）。
- 現場 AI 重構不記收據＝拍板結論（重開桌重匯卡等價於 undo），勿再議。
