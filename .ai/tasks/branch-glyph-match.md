# branch-glyph-match — 分支綁定的字形洞

## Summary
`transport/state_view.rs` 的 resolve_branch／auto_match_branch 仍是 `key == card_name` 精確同名，不做字形折疊；卡名與分支鍵字形（簡繁）不同時，要玩家在面板手動指認〔模型判斷·未裁決〕。來源：ai-card-refactor 結案時的已知限制 7（[交接檔](../handoffs/archive/ai-card-refactor.md)）。

## 已知線索
- ai-card-refactor 已在在場比對、名字池、狀態路徑做了 OpenCC TSCharacters 的簡繁折疊，分支綁定沒套。

## Next action
2026-10-29 輕防重審：只記已知限制、暫不動工〔模型判斷·未裁決〕——卡名與分支鍵剛好只差簡繁字形很少見，代價是玩家在面板手動指認一次；已知限制已寫在 [ai-card-refactor 交接檔](../handoffs/archive/ai-card-refactor.md)已知限制 7。真遇到實卡再做：重用同一套折疊，歧義時拒收、不猜。

## 驗收方向
卡名與分支鍵只差簡繁字形時自動綁定；折疊歧義時不綁、維持手動指認。
