# Task handoff
Task-ID: mvp-6-onboarding
Updated: 2026-07-22T15:10:00+00:00
Status: completed

## Goal
完成 MVP 切片 6「Onboarding（BYOK 引導）」：首開不問任何問題直接落在內建範例桌（附範例角色卡＋開場旁白）；僅在 transport=api 且缺 OpenRouter key 時顯示 BYOK 引導（開官方註冊頁、儲值說明、費用直覺化文案、貼 key 即玩）。除 API key 外零必填（NewPlan §4.1／§9.3）。

## Current state
已結案。2026-07-22 使用者 UI 實測兩項全通過，實測中發現的三個問題已一併修掉（見 Completed 末三項）。

## Completed
- data.rs：create_sample_world「迷霧酒館（範例）」＝world.md（含 GM 隱藏真相＋導演方針）＋3 角色卡（狐狸🦊／騎士🛡️／吟遊詩人🪕，含私有設定）＋1 則開場旁白＋ready-to-play 測試
- lib.rs：create_sample_world 指令＋註冊
- App.tsx：首開分支改走 create_sample_world；Onboarding 面板（transport=api 且無 key 才顯示，openUrl 開 OpenRouter 註冊頁／key 頁，貼 key 即存）
- 實作外包 codex（gpt-5.6-sol，workspace-write），主線寫規格＋逐條驗收；提交 4cfcc82
- 2026-07-22 實測後補修（未提交）：data.rs create_sample_world 改冪等（世界目錄已存在就回傳名稱，dev StrictMode 雙跑不再噴 File exists）＋測試補重複呼叫檢查
- 2026-07-22 文案修正：$5 改「最低 5 美元」「5 美元約可玩 3 小時」（原「$5 玩 5 個晚上」台幣美金同符號易誤解，且時數過度樂觀）；標題「費用有多低」改「費用有多高」（誠實揭露成本）
- 2026-07-22 樣式修正：App.css 加 .onboarding li／li button 間距，修掉步驟按鈕壓字

## Verification
- cargo test：27 綠（codex 沙盒禁 loopback 造成 mock server 測試誤報，本機重跑通過）
- npm run build（tsc＋vite）：過
- 主線親讀三檔 diff：文案逐字照規格、Onboarding 渲染位置與條件正確、無新依賴、無範圍外改動（git status 僅 3 檔）
- 2026-07-22 使用者 UI 實測（搬走 ~/Documents/TableTavern 後開 dev App）：
  - 首開零精靈：無任何問答直接進「迷霧酒館（範例）」，三張角色卡＋開場旁白齊全；磁碟驗到 worlds/迷霧酒館（範例）/transcript/0.jsonl 開場旁白
  - BYOK 引導：設定切 API 直連＋key 留空並按「儲存設定」後，面板出現在標題與角色列之間，三步驟＋儲值說明＋費用文案＋貼 key 即玩＋CLI 進階提示齊全
- 實測前置雷（已記）：Onboarding 讀的是已儲存的 config，只點 radio 不按「儲存設定」面板不會出現（App.tsx:83）
- 補修後 cargo test 30 綠、npm run build 過

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main
- HEAD: 4cfcc82194146209c9ce994be8187d76376c3fb5
- Dirty: false
- Dirty fingerprint: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945

## Remaining
- 無。

## Next action
- 無，已結案。

## Constraints
- 簡易模式只顯示能力檔位（NewPlan §3.1）；除 API key 外零必填（§9.3）
- 不加新依賴；不動 mvp-4 GM 邏輯
