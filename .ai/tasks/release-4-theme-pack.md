# Task
Task-ID: release-4-theme-pack
Title: 發佈 4：佈景主題引擎＋贊助包（回禮內容）
Status: completed
Created: 2026-07-22T22:35:12.291556+08:00
Updated: 2026-08-03T00:00:00+08:00

## Summary
主題功能與贊助回禮（NewPlan §16.1）：開源碼含完整主題載入引擎；贊助回禮不進 repo、不用解鎖碼。贊助解鎖檔案（`.ttpack`）已實作：丟進資料目錄或作者頁匯入即解鎖，取代舊的手改旗標。

## Next action
- 結案（2026-08-03 使用者拍板）。已交付並實機驗收：贊助解鎖檔 `.ttpack`（丟資料目錄或作者頁匯入即解鎖）、`KOFI_URL` 換成 Ko-fi 商品頁。
- **不做**：原 scope 的主題檔格式與載入引擎、五套贊助包資產外部化、自選桌布、AI 產生主題。理由＝五套配色已寫死在 `App.css` 並靠 `.ttpack` 閘門解鎖，玩家可見價值已全數上線；引擎只解「不改程式就能加主題」的未來問題，目前沒有需求驅動。
- 連帶關閉 [theme-pack-component-skins](theme-pack-component-skins.md)（元件裝飾 schema 沒有容器可掛）。NewPlan §16.1 已同步縮減 scope。
- 若日後要重啟：ui-overhaul 已把 `App.css` 全面 token 化，主題檔＝一組 token 覆寫值，引擎只需驗格式＋注入 CSS 變數，成本比原規劃低很多。交接現場見 [handoffs/archive/release-4-theme-pack.md](../handoffs/archive/release-4-theme-pack.md)。

## Constraints
付費資產不進 repo，不受 AGPL 涵蓋；開源下閘門可繞過為已拍板接受（榮譽制，`.ttpack` 只驗格式不驗簽章）；免費版＝開源 build，基礎白色一種（NewPlan §16.1）。
