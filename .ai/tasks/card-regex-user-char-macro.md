# card-regex-user-char-macro — 卡片 regex 替換字串不代換 {{user}}／{{char}}

## Summary
`src/features/card-interface/interface-card.ts` 的 `substitute()` 只處理 `{{match}}` 與 `$1..$9`，卡片 regex 殼裡寫的 `{{user}}`／`{{char}}` 在面板上顯示成字面。ST 在代換 `$n` 之後對整段結果再換 `{{user}}`／`{{char}}`（`public/scripts/extensions/regex/engine.js:444`）。實測佇列第 19 項⑤發現。

## Next action
未排程；立案時要定：照 ST 對含抓取文字的整段代換，或只換替換字串本身（當初刻意避免同一段文字被代換兩次）。
