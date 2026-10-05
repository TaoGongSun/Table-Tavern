# claude CLI 改寫 1 小時快取的連帶影響

## Summary
usage-cache-audit 實跑（2026-10-06，claude 2.1.287）：CLI 寫的是 `ephemeral_1h`（5m 為 0）。連帶三件前提不成立：以 300 秒判定過期、保溫 ping 的必要性（一次 $0.072，比劇情輪貴）、額度分頁估省下金額用 1.25 倍寫入係數（1h 寫入是 2 倍，省額被高估）。證據在 usage-cache-audit 計畫檔「四、對帳結果」。顯示面怎麼改與 usage-cache-audit 五項顯示建議一起由使用者拍板。

## Next action
先確認 1h 寫入是 CLI 固定行為還是可設定，再提停保溫／改門檻／改係數的方案給使用者拍板。
