# card-regex-user-char-macro 規格

拍板（Sol、Grok 審過，主線裁決 2026-10-28）：

1. 每次命中：先做抓取代換（`$N` 任意位數、`$0`、`$<name>`、`{{match}}` 不分大小寫），再對整段只掃一次 user／char，名字以回呼字面插入；第二輪不認 `$N`。取值用已切好的 captures，越界／未命中／不存在群組回空字串。`$&`、`$(` 維持字面。
2. trim_strings 先代換名字再挖（完整命中、位置與具名群組都挖）；代換後是空字串就跳過。
3. 殼用自己的 `/\{\{\s*(user|char)\s*\}\}/gi`；card-mvu-shim 與沙盒字串碼不放寬內側空白（已知限制）。
4. 玩家名沿用 App 的玩家名與翻譯後備值；char＝當次套用的卡名，空的保留字面（ST 換空字串）。userName 進 cardShell 快取鍵。
5. 重構骨架填值時先把 `{{user}}`／`{{char}}` 換成名字再做 YAML 跳脫，regex 看不到 token；regex 輸出的 HTML／JS 不跳脫。骨架的 `{{char}}` 只在桌上只有一張畫得出東西的卡時就地換；多張時保留字面，交給命中那張卡的 regex 換成它的名字。
6. 已知限制：多卡桌骨架的 `{{char}}` 在 regex 階段才換，卡名含引號或換行時 YAML 可能壞，交給卡片的那一樓文字也留著 `{{char}}`；玩家名本身含 `{{char}}`／`$1` 這類字面時，骨架插入後會被 regex 階段再換一次（Sol 驗收重現，罕見不防）；僅 user／char；`{{random}}`、`{{getvar}}`／`{{setvar}}`、`<USER>`／`<CHAR>`／`<BOT>`、`{{trim}}`、`{{user::…}}`、找式 substituteRegex 不做。
