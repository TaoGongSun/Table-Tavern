# Task handoff
Task-ID: mvp-6-onboarding
Updated: 2026-07-19T17:55:34.084953+00:00
Status: in-progress

## Goal
完成 MVP 切片 6「Onboarding（BYOK 引導）」：首開不問任何問題直接落在內建範例桌（附範例角色卡＋開場旁白）；僅在 transport=api 且缺 OpenRouter key 時顯示 BYOK 引導（開官方註冊頁、儲值說明、費用直覺化文案、貼 key 即玩）。除 API key 外零必填（NewPlan §4.1／§9.3）。

## Current state
實作完成並提交（4cfcc82），自動化驗證全過；剩使用者 UI 實測（與 mvp-4 同批驗收即可）。

## Completed
- data.rs：create_sample_world「迷霧酒館（範例）」＝world.md（含 GM 隱藏真相＋導演方針）＋3 角色卡（狐狸🦊／騎士🛡️／吟遊詩人🪕，含私有設定）＋1 則開場旁白＋ready-to-play 測試
- lib.rs：create_sample_world 指令＋註冊
- App.tsx：首開分支改走 create_sample_world；Onboarding 面板（transport=api 且無 key 才顯示，openUrl 開 OpenRouter 註冊頁／key 頁，貼 key 即存）
- 實作外包 codex（gpt-5.6-sol，workspace-write），主線寫規格＋逐條驗收；提交 4cfcc82

## Verification
- cargo test：27 綠（codex 沙盒禁 loopback 造成 mock server 測試誤報，本機重跑通過）
- npm run build（tsc＋vite）：過
- 主線親讀三檔 diff：文案逐字照規格、Onboarding 渲染位置與條件正確、無新依賴、無範圍外改動（git status 僅 3 檔）
- 未驗：UI 實測——首開範例桌需在「零桌」狀態觸發（暫時把 ~/Documents/TableTavern 搬走再開 App 即可重現）；Onboarding 面板需把設定切成 API 直連且 key 留空才會出現（現用 CLI 模式所以平常看不到）

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main
- HEAD: 4cfcc82194146209c9ce994be8187d76376c3fb5
- Dirty: false
- Dirty fingerprint: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945

## Remaining
- 使用者 UI 實測上述兩點，通過後 handoff complete＋task complete

## Next action
- 使用者實測：搬走 ~/Documents/TableTavern 開 App 看範例桌；設定切 API 直連（key 空）看 BYOK 引導

## Constraints
- 簡易模式只顯示能力檔位（NewPlan §3.1）；除 API key 外零必填（§9.3）
- 不加新依賴；不動 mvp-4 GM 邏輯
