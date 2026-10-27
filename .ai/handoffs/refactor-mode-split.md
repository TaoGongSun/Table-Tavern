# Task handoff
Task-ID: refactor-mode-split
Updated: 2026-10-27
Status: ready-to-close

## Goal
重構雙軌定向落地：介面優先 vs 角色優先（兩段式選擇＋模式專屬解析），四包完成（路由＋三態偵測＋二選一 UI／兩段 session／模式行為／穩定性驗收矩陣）。

## Current state
程式面四包＋四洞修復（d8f8f1c）都在 main，實測全數通過，只剩下方兩件小事即可結案。
- 2026-10-02 測試包五卡矩陣（Claude CLI claude-sonnet-5-5）：WestFantsy 連跑 3 次皆可套用；Transfur 接管；bcd368、NorthHall-structure 判 playable: no 不建殼（沒殼的桌不給面板，見 [refactor-noshell-panel](archive/refactor-noshell-panel.md)）；TrainEmperor 零派送擋下；二選一取消無產物；第二段 resume 快取命中 83～89%。
- 2026-10-27 測試包重測（獨立 root，Sonnet 5.5 重構／Haiku 5.5 回合，13 次呼叫）：
  - 洞①過：Transfur-safe 選多角色，「格式」(rule 5)「COT」(rule 1) 進已淘汰；套用後兩條掛「停用中」、`worldbook.json` 為 `disable: true`；無「卡片介面」鈕；GM 一回合出對話正文，無 `<mainPage>`／`prop|`／`skill|`。
  - 洞②過：匯入 `C2-mode-Characters.json` 套用，摘要「新增 18 個角色・新增 34 條世界書條目」無「介面已套用」；`state.json` 只有 `state.table: {}`、無 mechanism／介面狀態樹，`refactor_mode: "characters"`，無殼檔。
  - 洞④過：TrainEmperor 擋下訊息 top 366px，在按鈕列（297–327）正下方、第一條條目（431）之上，零派送。
  - 洞③由「重跑重構」定案取代（[refactor-statusbar-skeleton](archive/refactor-statusbar-skeleton.md) 定案結論 1），改驗新路徑並通過：NorthHall-structure 零額度匯入舊產物→面板有值→按重構跳「會用原卡重新重構」→確認後殼、產物、mechanism-log、mode 全清，世界書回 23 條→選保留原卡玩法重構→狀態樹單套、殼的佔位符全對得上樹路徑→Haiku 一回合 `<UpdateVariable>` 更新時間／地點／天氣／行動選項，面板跟著變。
  - 重構中取消過：WestFantsy 盤點串流中按取消，該呼叫無完成紀錄、無殘留子程序、無 refactor-outcome、世界書 38 條不動；重按重構可重新初判。
- 證據截圖在當次子代理 scratchpad（未入庫）。

## Completed
- 包 1：三態偵測＋二選一對話框＋unsupported 擋下＋refactor_recommend／survey 帶 mode＋i18n。
- 包 2：refactor_session.rs 短命 session（開線／resume＋指紋核對＋降級單發）。
- 包 3：模式專屬提示詞＋MODE 回聲核對＋mode-aware 稽核＋refactor_mode 持久化＋介面 fallback 抑制＋匯出入保 mode。
- GUI 回歸四洞修復（d8f8f1c）：apply 停用整條 dropped＋rewritten_entries 快照；effective apply_interface 閘門（含來源消耗判定）；normalize_interface_paths 正規化（鏡像分支折疊／值合併／rules remap／dangling 剔除，preflight 在任何寫入前）；INTERFACE_STATE_RULES 加單一路徑鐵則；worldbookMessage 置頂。

## Verification
- cargo test 500；vitest 137；npm run build ✓；check:i18n 十語系 OK；GUI 實測見上。

## Plan files
- .ai/plans/refactor-mode-split.md

## Working context
- Repo: /Users/<user>/GitHub/Table-Tavern
- Branch: main
- HEAD: d8f8f1c7a615dcde35912417b1882834e90e6c87
- Dirty: false
- Dirty fingerprint: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945

## Remaining
- 刪 `src-tauri/src/commands/refactor.rs:218` 的 `[survey-persons]` eprintln（診斷水印）。
- 輕：匯入的 characters 重構卡若帶介面產物，結果框仍顯示並預勾「遊戲介面搬進 app」（`src/features/refactor/refactor-review.ts:692`、`:710` 只看 `outcome.interface`，沒看 mode）；後端閘門已擋、套用不會落介面，只是預覽說法不實。只有匯入異常產物會遇到〔模型判斷·未裁決〕。

## Next action
處理上面兩件後結案。

## Constraints
- 快取紅線：survey／expand 共用 system 逐位元組相同（既有測試把關）。
- 舊產物（無 mode）＝照 interface 行為；判官 drop 仍限 rule 1–4，rule 5 只由 app 構造。
- 正規化只認精確別名 W.p↔p 不採相似度；衝突一律拒套不猜；原始 refactor-outcome.json 永不改寫（救援證據）。
- 現場 AI 重構不記收據＝拍板結論（重開桌重匯卡等價於 undo），勿再議。
