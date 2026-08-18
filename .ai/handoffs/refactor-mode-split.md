# Task handoff
Task-ID: refactor-mode-split
Updated: 2026-08-14T09:24:48.977395+00:00
Status: in-progress

## Goal
重構雙軌定向落地：介面優先 vs 角色優先（兩段式選擇＋模式專屬解析），四包完成（路由＋三態偵測＋二選一 UI／兩段 session／模式行為／穩定性驗收矩陣）。

## Current state
2026-08-14 GUI 回歸（TestCards/GUI-回歸交接.md）驗收 A1–E1 大多過、驗出四洞，已全修（d8f8f1c，經 Sol 確認修法）：①整條淘汰條目套用即停用（快照走 rewritten_entries，undo 覆寫可回；現場 AI 重構維持不記收據＝使用者拍板）；②mode 閘門提前到來源消耗判定前，characters 匯入不再套介面樹；③介面產物雙套路徑正規化（精確別名折疊、殼佔位符裁決正典、衝突拒套 preflight 零落檔）＋提示詞單一路徑鐵則；④世界書操作訊息移到按鈕列下。四件套綠，待使用者重打 release 包實測。舊測試桌（NorthHall、C2）使用者自行刪除不修復。

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
- 使用者重打 release 包重測四洞：A 桌（characters）套用後「格式」「COT」掛停用徽章、GM 出對話正文；C2 匯入後 state.json 無介面狀態樹、無 incremental；NorthHall 重跑重構後面板佔位符會更新；E1 擋下訊息出現在按鈕列正下方。
- 包 4 矩陣剩餘：同卡連跑三次皆可運行、取消反悔路、prompt-cache.jsonl 第二段 resume 命中。
- 驗完刪 lib.rs `[survey-persons]` eprintln（診斷水印，順路）。

## Next action
使用者重打 release 包（npm run tauri build）實測四洞修復；發現問題回主線修。

## Constraints
- 快取紅線：survey／expand 共用 system 逐位元組相同（既有測試把關）。
- 舊產物（無 mode）＝照 interface 行為；判官 drop 仍限 rule 1–4，rule 5 只由 app 構造。
- 正規化只認精確別名 W.p↔p 不採相似度；衝突一律拒套不猜；原始 refactor-outcome.json 永不改寫（救援證據）。
- 現場 AI 重構不記收據＝拍板結論（重開桌重匯卡等價於 undo），勿再議。
