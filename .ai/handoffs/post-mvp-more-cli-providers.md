# Task handoff
Task-ID: post-mvp-more-cli-providers
Updated: 2026-07-24T05:28:19.078310+00:00
Status: in-progress

## Goal
CLI 訂閱模式接入 gemini（官方 gemini-cli）端到端：偵測、headless 串流、檔位模型、設定頁選項＋一鍵安裝 UX（可見終端機、OAuth 回跳 CLI、app 不碰 token）；grok 留偵測介面後補。

## Current state
研究完成且原始碼抽查驗證；R1（供應商接入）Codex gpt-5.6-sol 背景執行中（Bash task id：bjj3fz38u）；R2（一鍵安裝）規格待 R1 落地後定稿。

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
- HEAD: 2dcbb4359486e434563367f6d548c45535698600
- Dirty: false
- Dirty fingerprint: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945

## Remaining
R1 複驗 → R2 規格定稿（安裝腳本＋Rust 開終端機＋輪詢＋設定頁按鈕）→ R2 實作與複驗 → 本機實測一鍵安裝（先用既有憑證 smoke，再暫搬憑證測全新登入）→ 使用者驗收。

## Next action
讀 /private/tmp/claude-501/-Users-pachelo-GitHub-Table-Tavern/dbb6d324-9b0b-4b7e-8f34-657fe0286962/tasks/bjj3fz38u.output 確認 R1 結果並複驗。

## Constraints
app 不碰帳密／token；安裝過程可見（終端機）；不傳 --yolo；模型用 CLI 穩定別名 pro/flash/flash-lite；風險告知前置；grok 本輪不做。
