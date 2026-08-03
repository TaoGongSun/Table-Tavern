# Task
Task-ID: ui-i18n-switch
Title: UI 語系切換（zh-TW／en）
Status: completed
Created: 2026-07-23T00:15:00+08:00
Updated: 2026-07-23T00:15:00+08:00

## Summary
緣起：需要英文介面。前端新增 `src/i18n.ts` 字典（zh-TW 為正典，en 逐鍵對應，缺鍵會被 TypeScript 擋下），App.tsx 全部 UI 字串改走 `t()`；側欄底部加「語言 Language」下拉，切換即寫入 `config.preferences.language` 立即生效。後端 transport.rs 的 LANGUAGE_RULE 改為 `language_rule(lang)` 依語系注入：zh 系維持繁中台灣用語規範，en 注入英文輸出規範（提示詞模板仍是中文，需明講輸出語言才不會被帶成中文）。資料層識別字（`玩家` 哨兵、transcript 內容、config key）不受語系影響。

驗證：`npm run build` rc=0；`cargo test` 31 passed（含新增 `language_rule_follows_ui_language`）。

## Next action
- 無，已結案。後續已知缺口（未做，量小時再開任務）：後端錯誤訊息（如「尚未設定 API key」）與範例桌內容仍為中文。

## Constraints
資料層識別字維持中文常數（`玩家` 哨兵、保留名檢查）；語系只影響顯示與 system prompt 語言規範。
