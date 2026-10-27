# takeover-yaml-key-glyph — 接管桌續聊 GM 把清單欄 YAML 鍵寫成繁體

## Summary
接管桌續聊時，GM 把清單欄的 YAML 鍵寫成繁體，原卡腳本只認簡體鍵，人物卡片少欄。這是 ai-card-refactor 驗收項 B5「附品質問題」的重現；該案已結案，沒有對應待辦，故立此案。

## 已知線索
- 出處：[ai-card-refactor 交接檔](../handoffs/archive/ai-card-refactor.md) B5。
- 重現證據：`/private/tmp/claude-501/-Users-<user>--claude-spares-spare-yfWgps/853b218d-0b3d-4f00-b3b6-0b81bbbd6996/scratchpad/sd/shots/06c-old-lane-interface.png`（暫存檔，可能已被清掉）。
- 與 present-name-interface-lang 同型（輸出一律繁體 vs 原卡字形），但清單欄鍵是腳本讀取的資料鍵，修法未必同。

## Next action
未排程；開工先重現，確認提示詞裡鍵名的呈現方式，再決定是提示、折疊鍵名，或寫入時校正。

## 驗收方向
接管桌續聊多輪，清單欄的鍵與原卡一致，介面不少欄。
