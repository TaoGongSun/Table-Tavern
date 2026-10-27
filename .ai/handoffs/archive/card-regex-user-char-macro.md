# card-regex-user-char-macro — 卡片 regex 殼代換 {{user}}／{{char}}

## 狀態
已結案進 main（2026-10-28）。桌面版卡片 regex 照 ST runRegexScript：抓取引用（`{{match}}`、`$N` 任意位數、`$0`、`$<name>`）只掃一輪，再對整段代換一次 `{{user}}`／`{{char}}`；trim_strings 先代換名字。重構骨架填值時就把 `{{user}}`（只有一張畫得出東西的卡時連 `{{char}}`）換成名字並照位置做 YAML 跳脫。網頁版原本就照 ST，沒動。規格、已知限制見 [plans/card-regex-user-char-macro.md](../../plans/card-regex-user-char-macro.md)。

## 分歧紀錄
玩家名本身含 `{{char}}`／`$1` 字面時，骨架插入後會被 regex 階段再換一次：Sol 主張要防，主線裁決列已知限制不防（罕見邊際狀況輕防），Sol 接受。

## 下一步
無。
