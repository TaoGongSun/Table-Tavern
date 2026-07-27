# Task
Task-ID: release-4-theme-pack
Title: 發佈 4：佈景主題引擎＋贊助包（回禮內容）
Status: todo
Created: 2026-07-22T22:35:12.291556+08:00
Updated: 2026-07-29T01:30:00+08:00

## Summary
主題功能與贊助回禮（NewPlan §16.1）：開源碼含完整主題載入引擎；贊助包是純資料檔（五套主題色票＋圖片、自選桌布開關），不進 repo、不用解鎖碼，丟進資料目錄即生效。

## Next action
- **先做「贊助解鎖檔案」**（2026-07-28 使用者要求提前，不再等 release-1／release-2）：定贊助包檔案長什麼樣、丟進哪裡、app 怎麼認得它。現況是 sponsor-features 用 `preferences.sponsor_unlocked === true` 這個手改旗標暫代（[handoffs/sponsor-features.md](../handoffs/sponsor-features.md)），格式定案後要換掉它，並補上 sponsor-features 缺的「匯入贊助包」入口（預計放作者頁）。
- 之後才是：主題檔格式與載入引擎＋基礎白色主題 → 五套贊助包資產與自選桌布 → AI 產生主題（v1 可不上）
- 相依：主題檔格式定案時要順便預留「元件裝飾」schema（見 theme-pack-component-skins）

## Constraints
付費資產不進 repo，不受 AGPL 涵蓋；開源下閘門可繞過為已拍板接受（榮譽制）；免費版＝開源 build，基礎白色一種（NewPlan §16.1）。
