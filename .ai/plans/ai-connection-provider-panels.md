# AI 連線設定重整：供應商專屬面板（延後）

本檔存放 [ai-connection-provider-panels](../tasks/ai-connection-provider-panels.md) 的剩餘規格。2026-09-09 已把 OpenRouter OAuth、推薦免費模型、限時免費模型、遠端推薦 manifest 全部移交 [free-player-onboarding](../tasks/free-player-onboarding.md)。

## 為什麼延後

原案同時想解決兩個不同問題：

1. 新玩家不知道怎麼拿 key、選模型，所以開始門檻太高。
2. AI 設定頁把 OpenRouter 與各 CLI 都塞進同一套高／中／低抽象，長期不夠自然。

現在優先只處理第 1 類，而且拆成「OAuth 免 key」→「推薦／限時免費模型」兩步。第 2 類只是設定頁整潔與進階 UX，不應阻擋免費玩家先能玩。

## 未來若重啟，本案剩餘範圍

### 1. 所有 provider 仍在同一頁可見

保留 OpenRouter、Claude、Codex、Antigravity、Grok 的平等入口；不新增一個把 CLI 整組藏起來的「進階設定」總頁。

### 2. 下半頁 provider-specific

玩家選哪個 provider，下方只顯示該 provider 真正需要的控制：

- OpenRouter：自己的連線與模型相關設定。
- CLI：各自的安裝、登入、重新驗證、換帳號與真正可用的模型控制。
- 跨 provider 的遊戲行為設定另外放共同區，不和模型選擇混在一起。

### 3. CLI 不必硬套高／中／低

未來可以依各 CLI 現況決定：

- 值得選單一模型 → 顯示單一模型控制。
- CLI 有清楚穩定的模型選項 → 顯示實際支援項目。
- Table Tavern 不值得指定 → 直接使用 CLI 預設模型。

舊 `claude:best`、`codex:best` 等 config 即使 UI 不再強迫顯示，也應先保留相容，不為清設定增加 migration 風險。

## 明確不屬本案

以下已移到 [free-player-onboarding](../tasks/free-player-onboarding.md)：

- OpenRouter OAuth PKCE。
- 免 key onboarding。
- `openrouter/free` bootstrap。
- Table Tavern 推薦免費模型。
- 限時免費模型。
- OpenRouter catalog metadata 與推薦 manifest。

因此 `free-player-onboarding` 施工時**不要順手改本案的 CLI／provider panel**。

## 重啟條件

只有在免費玩家 onboarding 兩階段完成後，實測仍顯示設定頁本身造成明顯困惑，才把本案拉回優先序。開工時重新盤 `SettingsForm`、各 CLI catalog 與現有設定資料，不沿用 2026-09-03 的 UI 細節當硬規格。
