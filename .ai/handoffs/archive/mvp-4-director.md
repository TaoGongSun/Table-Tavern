# Task handoff
Task-ID: mvp-4-director
Updated: 2026-07-22T14:45:00+00:00
Status: completed

## Goal
完成 MVP 切片 4「簡易導演（GM）」：GM 上下文＝world.md（只進 GM）＋全部角色卡（含私有）＋公開 transcript；支援「建議下一位發言者」、插入旁白、每回合最大發言數（NewPlan §6.1／§7.0，KICKOFF §5.4）。

## Current state
已結案。2026-07-22 使用者在 dev App 內完成三項人手驗收，全通過。

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
- 2026-07-22 使用者 dev App 實測（CLI 模式，claude 訂閱）：
  - world.md 編輯器：加入獸人世界設定後儲存，重啟 App 仍在（world.md 尾段）
  - GM 旁白：置中旁白區塊正常串流，內容扣題，未說破鎮長狼人／狐狸通緝犯
  - GM 推進：transcript/0.jsonl 出現 3 組「system 點名＋dialogue 發言」（吟遊詩人→騎士→狐狸），達 max_round_speakers=3 即停
- 實測前置雷（已記）：呼叫 CLI 的 shell 若帶 ANTHROPIC_AUTH_TOKEN／ANTHROPIC_BASE_URL 等變數，claude CLI 會回 401 Invalid bearer token；需 unset 後再開 App

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main
- HEAD: 985ce42319e0f44d48d5dcb69a446f9d11dc0815
- Dirty: false
- Dirty fingerprint: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945

## Remaining
- 無。

## Next action
- 無，已結案。

## Constraints
- 不提前做完整 GM 模式（骰子、戰鬥、地圖，NewPlan §6.2／§12）
- 不加新依賴；模型 id 一律來自 config，不寫死
- 角色永不收到 world.md 與他人私有設定
