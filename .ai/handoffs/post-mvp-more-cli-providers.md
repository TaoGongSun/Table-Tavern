# Task handoff
Task-ID: post-mvp-more-cli-providers
Updated: 2026-07-24T05:51:57.855260+00:00
Status: in-progress

## Goal
CLI 訂閱模式接入 gemini（官方 gemini-cli）端到端：偵測、headless 串流、檔位模型、設定頁選項＋一鍵安裝 UX（可見終端機、OAuth 回跳 CLI、app 不碰 token）；grok 留偵測介面後補。

## Current state
程式全部完成入庫：agy 供應商接入（b5a50f5）＋一鍵安裝（84e6f55），cargo 64/64、npm build 綠、主線親讀複驗。剩本機實測與使用者驗收。

## Completed
- 拍板：目標是官方 gemini-cli（機上 agy 實為 Antigravity CLI）；一鍵安裝取代舊「不代裝」約束；終端機開頭「正在自動安裝 Gemini CLI，請勿關閉此視窗」、結尾「驗證成功，已連結，可以關閉終端機視窗」，登入輪詢在腳本內完成。
- 測試機備妥：~/.local/bin/agy → agy.bak、brew uninstall gemini-cli；~/.gemini/oauth_creds.json 保留（測全新登入時暫搬）。
- 研究（haiku 外包＋主線原始碼抽查，v0.52.0）：settings 欄位 security.auth.selectedType="oauth-personal"（settings.ts:684、contentGenerator.ts:64）；憑證檔 ~/.gemini/oauth_creds.json（storage.ts:22）；headless `-p -m --output-format stream-json`，JSONL 事件 message(delta)/error/result；無 system 旗標；OAuth localhost 回跳；NO_BROWSER 替代流程。原文：scratchpad/gemini-cli-research.md＋gemini-cli-src/ clone。
- R1 交辦檔：scratchpad/task-gemini-r1.md（含事件格式與驗收清單）。

## Verification
研究三關鍵事實主線 grep 原始碼驗證（見 Completed 行號）；R1 尚無產出，回來須 cargo test＋npm build＋親讀 diff。

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main
- HEAD: 84e6f551c5e47fa3ecfc6927f983839742d69d4f
- Dirty: false
- Dirty fingerprint: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945

## Remaining
本機實測一鍵安裝與 agy 實聊（見 Next action）；測「全新帳號登入」需暫搬 ~/.gemini 相關資料（含 antigravity-cli/implicit/*.pb——實測發現搬走 oauth_creds.json 仍能通過驗證，真憑證在 implicit 快取），動手前先告知使用者。grok 供應商未做（無額度，另開）。agy 關鍵實測事實：-p 必帶值、stdin 不進上下文、純文字輸出 EOF 收尾、`agy models` 動態清單、登入訊號只信功能探針 `agy -p ok` rc=0（勿輪詢 ~/.gemini 內部檔）。安裝：官方 curl -fsSL https://antigravity.google/cli/install.sh | bash（無互動）。

## Next action
使用者在場時本機實測：1) mv ~/.local/bin/agy agy.bak 後 npm run tauri dev，設定→AI 連線→agy 列按「一鍵安裝」，看終端機腳本跑完印成功訊息（已登入帳號會秒過探針）；2) 還原或重裝後在桌上把某角色檔位綁 agy 模型實聊一輪。全過即可結案。

## Constraints
app 不碰帳密／token；安裝過程可見（終端機）；不傳 --yolo；模型用 CLI 穩定別名 pro/flash/flash-lite；風險告知前置；grok 本輪不做。
