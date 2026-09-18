# Task
Task-ID: ai-connection-provider-panels
Title: AI 連線設定重整：供應商專屬面板（延後）
Status: todo
Created: 2026-09-03T17:53:09+08:00
Updated: 2026-09-09T16:21:41+08:00

## Summary
本案只保留「設定 → AI 連線」的架構整理：讓 OpenRouter、Claude、Codex、Antigravity、Grok 各自顯示真正有意義的設定，而不是所有 transport 都硬套同一組高／中／低表單。

2026-09-09 起，本案**不再負責 OpenRouter OAuth、免費新手路徑、推薦免費模型、限時免費模型或推薦 manifest**；那些全部移到 [free-player-onboarding](free-player-onboarding.md)，而且要先完成。

## Progress
- 2026-09-03：原案同時包含 OpenRouter 免費推薦與 provider-specific panel。
- 2026-09-09：拆案。免費玩家 onboarding 成為優先路線；本案降為後續設定頁整理，不得阻擋或擴張前者。

## Next action
- **暫不開工。** 等 [free-player-onboarding](free-player-onboarding.md) 的「免 key」與「推薦／限時免費模型」兩階段完成後，再看真實玩家是否仍被設定頁困擾，再決定是否值得動 CLI／高中低 UI。

## Constraints
- 不搶先修改 `free-player-onboarding` 正在依賴的 OpenRouter onboarding／模型發現流程。
- 若未來開工，既有 `tier_models`、角色卡 tier 語意與舊 config 仍須相容。
- CLI 是否顯示單一模型、實際模型選項或完全沿用 CLI 預設，屆時依各 CLI 現況重新盤點；不要在本案尚未排程時先改。
