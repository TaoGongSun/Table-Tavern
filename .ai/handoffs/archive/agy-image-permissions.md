> 結案 2026-11-04：進 main，Sol 驗收 PASS（無必改）。

# agy-image-permissions — agy 生圖收緊執行權限

Status: done

## Summary
`agy_args` 一律帶 app 專用代理：文字用 `table-tavern-text`，生圖用只給 generate_image／view_file 的 `table-tavern-image`。不再帶 `--dangerously-skip-permissions`，並用 `--settings` 鎖成 request-review、清空放行規則，蓋過 agy-home 裡可能存在的設定檔。`agy_home::ensure_agents` 維護兩份代理檔。VN 參考圖的提示寫執行時的絕對路徑。規格、實測與作者裁決見[方案檔](../../plans/agy-image-permissions.md)。

驗證：verify 16 步綠。agy 1.4.12 夾具全情境重錄；誘導格用 app 參數實跑，沒有標記檔。測試通道角色圖庫與 VN 立繪定稿（卡圖參考）各一張 agy 出圖。

## 已知限制
- `view_file` 讀得到 agy-home 的 `.gemini/`，裡面有同一玩家其他桌的 agy 對話；讀到的內容可能進入回覆或生圖提示詞，未阻止外流。作者知情並選擇保留 `view_file`〔作者裁決 2026-11-02〕。
- `--settings` 能否蓋過 project／shared scope 的權限規則未證實；白名單在 schema 層沒有直接證據；代理檔被竄改退回預設工具時 `search_web` 可用。Windows 未實測。詳見方案 §5。
