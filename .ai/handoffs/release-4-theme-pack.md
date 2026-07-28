# Handoff: release-4-theme-pack

## Current state
贊助解鎖檔案（提前段）實作完成，使用者實機驗收匯入通過。剩 Ko-fi 商品頁連結替換（等開店）；主題引擎等原任務 scope 未動工。

## Completed
- 贊助包檔案格式定案：單一 JSON 檔、副檔名 `.ttpack`，必要欄位 `type: "table-tavern-sponsor-pack"` 與 `format`（正整數），其他欄位一律忽略（向前相容；未來要裝主題資產再升容器格式，匯入口先認格式）。榮譽制已拍板，不做簽章。
- 後端：`validate_sponsor_pack`／`sponsor_pack_active`（掃資料根目錄第一層任何 `*.ttpack`，手動丟檔即生效）／`install_sponsor_pack`（寫入 `sponsor-pack.ttpack`），data.rs:1508 起；lib.rs 兩個 command `sponsor_status`／`import_sponsor_pack`（lib.rs:124）。
- 前端：解鎖狀態改為啟動時問後端的推導值（App.tsx 主元件 `sponsorUnlocked` state），`preferences.sponsor_unlocked` 手改旗標全數移除（grep 0 命中）；作者頁 Ko-fi 鈕下方加「匯入贊助包」（選檔→匯入→原地顯示感謝字樣；失敗顯示原因小字）；resolveTheme／五套配色閘門／AI 生圖次數閘門改吃新狀態。i18n zh/en 各 +3 鍵。
- 販售檔案本體已產出（不進 repo）：`~/Desktop/TableTavern-SponsorPack.ttpack`，上架 Ko-fi Shop 用；內容照上述格式，含中英文使用說明欄位。

## Verification
- `cargo test` 114 綠（基線 110，+4：合法包通過、type 錯拒絕、format 缺拒絕、install 後 active／空目錄 inactive）；`cargo clippy --all-targets` 0 error
- `npm run build` exit 0；`grep -rn "sponsor_unlocked" src src-tauri/src` 0 命中
- 實作由 codex（gpt-5.6-terra）完成，主線逐行審過 diff 並本機重跑全部驗證
- 2026-07-28 使用者實機驗收通過：匯入 `.ttpack` 成功解鎖

## Remaining
- 作者頁與各處 Ko-fi 連結（App.tsx `KOFI_URL`）目前指作者主頁；等 Ko-fi Shop 商品上架（release-3-kofi，使用者操作）拿到商品頁網址後換掉，一行改動（使用者：屆時交給 opus 做即可）。
- 之後才是原任務 scope：主題檔格式與載入引擎＋基礎白色主題 → 五套贊助包資產與自選桌布 → AI 產生主題（v1 可不上）；主題檔格式定案時預留「元件裝飾」schema（theme-pack-component-skins）。

## Next action
使用者走 release-3-kofi 開店上架 `~/Desktop/TableTavern-SponsorPack.ttpack`，拿到商品頁網址後換 `KOFI_URL`。
