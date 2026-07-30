# Task
Task-ID: release-4-theme-pack
Title: 發佈 4：佈景主題引擎＋贊助包（回禮內容）
Status: in_progress
Created: 2026-07-22T22:35:12.291556+08:00
Updated: 2026-07-30T14:45:00+08:00

## Summary
主題功能與贊助回禮（NewPlan §16.1）：開源碼含完整主題載入引擎；贊助回禮不進 repo、不用解鎖碼。贊助解鎖檔案（`.ttpack`）已實作：丟進資料目錄或作者頁匯入即解鎖，取代舊的手改旗標。

## Next action
- 匯入流程 2026-07-28 實機驗收通過；`KOFI_URL` 已於 2026-07-30 換成商品頁連結，細節見 [handoffs/release-4-theme-pack.md](../handoffs/release-4-theme-pack.md)。
- 接著是原 scope：主題檔格式與載入引擎＋基礎白色主題 → 五套贊助包資產與自選桌布 → AI 產生主題（v1 可不上）
- 相依：主題檔格式定案時要順便預留「元件裝飾」schema（見 theme-pack-component-skins）

## Constraints
付費資產不進 repo，不受 AGPL 涵蓋；開源下閘門可繞過為已拍板接受（榮譽制，`.ttpack` 只驗格式不驗簽章）；免費版＝開源 build，基礎白色一種（NewPlan §16.1）。
