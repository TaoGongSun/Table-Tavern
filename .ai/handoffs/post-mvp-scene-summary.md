# Task handoff
Task-ID: post-mvp-scene-summary
Updated: 2026-07-24T12:30:00+08:00
Status: done

## Goal
依 NewPlan §8／§8.1：「換場」動作——把當前場景公開紀錄用單發 LLM 呼叫壓成摘要，存入本機正典並帶進新場景上下文。不依賴供應商 session 或自動壓縮。

## Current state
程式碼完成（Opus subagent 實作、主線驗收）。設計拍板：摘要以 GM 旁白事件（【前情提要】／Previously: 前綴）寫入新場景 transcript 開頭——角色與 GM 上下文、匯出、下次換場的鏈式壓縮全部自然沿用，不另做注入管線；摘要檔位固定 GM 檔位（不做設定項，YAGNI）。cargo test 43 綠、npm build 綠。2026-07-24 使用者實測換場通過（附截圖），結案。

## Completed
- transport.rs `summary_messages(events, lang)`（transport.rs:140-172）：system 指示（條列地點時間／人物狀態／關鍵事件／關係變化／未解懸念，zh 約 300 字／en 約 200 words）＋公開 transcript 逐行；不含 world.md 與角色卡（摘要只壓公開事件，避免私有資訊外洩到公開摘要）
- data.rs `begin_next_scene(root, world, summary_text, lang)`（data.rs:677-704）：摘要包成 GM Narration 事件 append 到 scene+1、current_scene +1 寫回 state、回傳新場景號；ts 用 local_timestamp()（已確認 ts 全程只存不解析）
- lib.rs `advance_scene` command（lib.rs:309-334，generate_handler 已註冊）：空場景擋下（「沒東西可以換場」）；摘要走既有 stream_via_transport＋gm_tier
- App.tsx `advanceScene()`（App.tsx:827-841）＋chat-header「換場」鈕（App.tsx:1186-1196）：generating 鎖 UI、空場景 disabled、完成後 enterTable 重載
- i18n sceneAdvance／sceneAdvanceHint（zh＋en）

## Verification
- 主線親跑 `cargo test`：**43 passed; 0 failed**（新增 begin_next_scene zh/en 前綴＋場景推進測試、summary_messages 內容測試）
- 主線親跑 `npm run build`：rc=0
- 主線抽查行號：advance_scene 空場景防護＋GM 檔位（lib.rs:319-327）、雙語前綴（data.rs:685-689）、前端 disabled 條件（App.tsx:1192）

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main

## Remaining
- 無

## Next action
- 無（任務結案；場景歷史瀏覽／單場匯出／換場提醒見 scene-history-browser）

## Constraints
- 摘要只讀公開 transcript（不讀 world.md／角色私有）；不依賴供應商 session；摘要檔位固定 GM 檔位
