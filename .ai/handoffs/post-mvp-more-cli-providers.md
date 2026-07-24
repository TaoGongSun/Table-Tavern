# Task handoff
Task-ID: post-mvp-more-cli-providers
Updated: 2026-07-24T05:47:37.454424+00:00
Status: in-progress

## Goal
CLI 訂閱模式接入 gemini（官方 gemini-cli）端到端：偵測、headless 串流、檔位模型、設定頁選項＋一鍵安裝 UX（可見終端機、OAuth 回跳 CLI、app 不碰 token）；grok 留偵測介面後補。

## Current state
重大轉向已完成：gemini-cli 個人帳號遭 Google 停用（主線實裝實測 IneligibleTierError），目標改 Antigravity CLI（agy）。agy 接入已實作、複驗、commit（b5a50f5）；R2 一鍵安裝由 Codex gpt-5.6-terra 背景實作中（Bash task id：bhe89y3zu，交辦檔 scratchpad/task-agy-r2-install.md）。

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
- HEAD: b5a50f50d31b7f5739a845d3800c174aa7b54010
- Dirty: false
- Dirty fingerprint: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945

## Remaining
R2 複驗入庫 → 本機實測一鍵安裝（agy 先移除，實跑終端機腳本；測全新登入需暫搬整個 ~/.gemini 相關資料，動手前先告知使用者）→ 使用者驗收。agy 關鍵實測事實：-p 必帶值、stdin 不進上下文、純文字輸出 EOF 收尾、`agy models` 動態清單、登入訊號只信功能探針 `agy -p ok` rc=0（勿輪詢 ~/.gemini 內部檔）。安裝：官方 curl -fsSL https://antigravity.google/cli/install.sh | bash（無互動）。

## Next action
讀 /private/tmp/claude-501/-Users-pachelo-GitHub-Table-Tavern/dbb6d324-9b0b-4b7e-8f34-657fe0286962/tasks/bhe89y3zu.output 確認 R2 一鍵安裝實作結果並複驗（cargo test＋npm build＋親讀腳本產生邏輯）。

## Constraints
app 不碰帳密／token；安裝過程可見（終端機）；不傳 --yolo；模型用 CLI 穩定別名 pro/flash/flash-lite；風險告知前置；grok 本輪不做。
