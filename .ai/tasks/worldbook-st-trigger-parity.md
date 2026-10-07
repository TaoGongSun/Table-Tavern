# 桌面版世界書觸發補齊到 SillyTavern 行為

Status: todo

## Summary
桌面版觸發只做 constant＋最近 4 則子字串比對（`src-tauri/src/transport/context.rs:8`）；已結案的 [worldbook-st-format](worldbook-st-format.md) 當時明訂 token budget／機率／遞迴不做、需要時另開。web-version 計畫的網頁版照 ST 完整行為做，網頁存檔帶 sticky／cooldown／delay 等觸發狀態，桌面版匯入後只保存不消費（[plans/web-version.md](../plans/web-version.md)「二之二、桌檔契約」）。本案把桌面版補到 ST 行為（次要鍵邏輯、掃描深度、正則鍵、機率、遞迴、插入位置、預算、sticky／cooldown／delay、inclusion group），並開始消費網頁存檔的觸發狀態。2026-10-07 web-version 第 1 輪審查衍生立案。

## Next action
未排程。開工先對照 ST 釘版本的 World Info 行為與桌面版 GM／角色兩條注入路，決定可見性（`extensions.table_tavern.visibility`）與 ST 插入位置怎麼共存。
