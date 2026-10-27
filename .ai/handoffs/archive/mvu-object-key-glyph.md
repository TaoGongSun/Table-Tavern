> 結案 2026-10-28：Sol、Grok 驗收 DONE，squash 進 main。

# mvu-object-key-glyph — 物件值 replace／insert 的巢狀鍵做簡繁折疊

## Summary
GM 以物件值 replace／insert（例 `{"行動": …}`）時巢狀鍵不折疊，樹與 stat_data 長出繁體鍵，原卡腳本讀不到。takeover-yaml-key-glyph 送審時拆出。

## 做法（Sol、Grok 送審，主線裁決 2026-10-28）
`src-tauri/src/mechanism/value_keys.rs`：不改樹的純函式，掛在 `apply.rs` 的 `canonicalize` 之後、寫樹與帶型別視圖之前改 patch 的值（兩邊吃同一份）；同批逐筆依當下的樹解析。
- 只處理 replace／insert 的物件值；底線開頭鍵不參與；陣列不往下走、路徑進到陣列（中途是葉子）也不插手；落在集合裡或集合祖先的路徑交給集合（`collection::claims`），不插手。
- 每層沿配對後的舊節點：精確 → 該節點同層唯一只差字形 → 對不上時拿同層兄弟物件鍵聯集當候補，仍須唯一才折；都對不上是新鍵照收。配對後子節點不存在時（新項目、既有項目新增的分支），下一層候補是舊節點底下其他分支加上兄弟底下同名分支。
- 碰撞：寫入物件裡折完撞成同一鍵、或新引入只差字形的雙鍵 → 這一筆拒收回饋，同批其他筆照常；舊值本來就有的雙鍵照原字回寫放行。
- 歧義：寫入鍵精確命中其一照收；對不上又像兩個以上 → 這一筆拒收回饋。

## 已知限制（不在本案）
- insert 路徑末段維持不折（`fold_last=false`）：`insert /人物/賀錚` 旁邊已有 `贺铮` 仍會長出第二個。
- 陣列裡的物件（`/背包/0`）與 zod 桌另案。

## 驗收方向
GM 用不同字形寫同層既有鍵時落回原鍵；合法新鍵照收；碰撞不產生重複鍵。測試 9 項 `mechanism/object_key_glyph_tests.rs`（帶 stat_data 的 `apply_block_typed`）。
