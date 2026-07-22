# Task handoff
Task-ID: mvp-4-director
Updated: 2026-07-19T17:44:04.787115+00:00
Status: in-progress

## Goal
完成 MVP 切片 4「簡易導演（GM）」：GM 上下文＝world.md（只進 GM）＋全部角色卡（含私有）＋公開 transcript；支援「建議下一位發言者」、插入旁白、每回合最大發言數（NewPlan §6.1／§7.0，KICKOFF §5.4）。

## Current state
實作全部完成並提交（985ce42），自動化驗證與真實 CLI 冒煙全過；只剩使用者在 App 內點按 GM 兩鈕的人手驗收（原生視窗無法自動點擊）。

## Completed
- transport.rs：assemble_gm_messages（world.md＋全卡含私有＋公開 transcript，GM 旁白→assistant）、suggest／narrate 導演指示、pick_speaker（含「玩家」哨兵）、gm_tier（preferences.gm_tier 預設 best）＋3 個新測試
- lib.rs：抽 stream_via_transport 共用 API／CLI 分流；新增 gm_narrate／gm_suggest_speaker 指令並註冊
- cli.rs：flatten_messages 收尾指示參數化，GM 與角色共用攤平路徑
- App.tsx：world.md 編輯器、GM 旁白／GM 推進按鈕（至「玩家」或 preferences.max_round_speakers 上限停）、旁白串流顯示、設定加 GM 檔位與每回合上限、保留名稱 GM／玩家擋建卡
- 提交 985ce42

## Verification
- cargo test：26 綠（含新增 GM 組裝／點名解析／GM 檔位測試）
- npm run build（tsc＋vite）：過
- 真實 claude CLI 冒煙（暫時 #[ignore] 測試，跑畢已移除；需清掉 ANTHROPIC_AUTH_TOKEN／ANTHROPIC_BASE_URL 等 session 環境變數才能用訂閱登入）：GM 點名回「騎士」解析正確，旁白扣題且未說破未揭露設定
- 未驗：App 內實際點按「GM 旁白」「GM 推進」（invoke 參數映射與 chat_with_character 同型，風險低）

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main
- HEAD: 985ce42319e0f44d48d5dcb69a446f9d11dc0815
- Dirty: false
- Dirty fingerprint: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945

## Remaining
- 使用者開 App（npm run tauri dev）實測：world.md 編輯器儲存、「GM 旁白」出現置中旁白區塊、「GM 推進」自動點名接力並在上限或「玩家」停下
- 實測通過後 handoff complete＋handoff task complete

## Next action
- 請使用者開 App 實測 GM 旁白與 GM 推進兩鈕，通過即結案

## Constraints
- 不提前做完整 GM 模式（骰子、戰鬥、地圖，NewPlan §6.2／§12）
- 不加新依賴；模型 id 一律來自 config，不寫死
- 角色永不收到 world.md 與他人私有設定
