# Task
Task-ID: ai-connection-provider-panels
Title: AI 連線設定重整：OpenRouter 免費推薦＋供應商專屬面板
Status: todo
Created: 2026-09-03T17:53:09+08:00
Updated: 2026-09-03T17:53:09+08:00

## Summary
重整「設定 → AI 連線」的呈現方式：所有可用連線方式繼續平等顯示在同一頁，不新增會嚇退一般玩家的「進階設定」總入口；玩家選 OpenRouter、Claude、Codex、Antigravity 或 Grok 後，下方只顯示該供應商真正需要的設定。

OpenRouter 改成「預設一個 Table Tavern 推薦免費模型即可開玩」；既有高／中／低模型分級保留，但收進「顯示其他模型」的展開區，供需要細分角色品質／成本的玩家使用。推薦來源採本機 fallback＋OpenRouter 公開模型目錄＋GitHub repo 內可遠端更新的靜態推薦清單，不先架自營伺服器。

規格細節與分階段驗收見 [plans/ai-connection-provider-panels.md](../plans/ai-connection-provider-panels.md)。本案更新 2026-08 的舊方向：CLI／BYOK 不再因為較複雜就整體收進「進階」摺疊。

## Progress
- 2026-09-03 立案；已拍板同頁顯示所有 provider、provider-specific 下半頁、OpenRouter 預設推薦免費模型、tier 保留但預設隱藏、第一階段以 GitHub 靜態資料解決遠端推薦更新。

## Next action
- 開工時先做 UI／資料契約切分：把目前 SettingsForm 內共用的 tier UI 改成 provider-specific 區塊；同時定義 OpenRouter 推薦 manifest 與本機 fallback 格式，再接現有 `/api/v1/models` 清單。

## Constraints
- 不新增「進階」總頁面，也不把 CLI／BYOK 整體藏起來；所有連線方式仍在同一頁可見。
- OpenRouter 新手路徑必須能在不理解模型分級的前提下，用單一推薦免費模型開始玩。
- `best`／`balanced`／`fast` 既有資料與角色卡語意先保留；本案只改預設呈現與路由方式，不為了 UI 簡化破壞相容性。
- CLI 不強迫套用 OpenRouter 的高／中／低模型配置；每家只顯示自身實際支援且有意義的模型／登入設定。
- 第一階段不建自營伺服器、不需要資料庫；遠端推薦以公開 GitHub 靜態檔為基礎，斷網或 GitHub 失效時仍須能靠內建 fallback／既有快取使用。
- OpenRouter 不是被鎖成「只能免費」：預設推薦免費，但展開後仍保留其他免費模型、付費模型與自訂模型出口。
