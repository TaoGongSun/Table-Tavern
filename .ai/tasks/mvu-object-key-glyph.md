# mvu-object-key-glyph — 物件值 replace 的巢狀鍵不做簡繁折疊

## Summary
未重構 MVU 卡 GM 以物件值 replace（例如 `{"行動": …}`）時，巢狀鍵不經字形折疊（`mechanism/tree.rs` json_to_node Object 分支；帶型別的 `upstream_set` 在 `apply.rs` 約 513–524 行也走這條），樹上會長出繁體鍵，原卡腳本讀不到。takeover-yaml-key-glyph 送審時拆出，Sol／Grok 都主張另案：物件鍵是 JSON 鍵、參考是樹上既有鍵，新鍵可合法存在，與清單字串不同條寫入。

## Next action
未排程；開工先重現，再定同層比對、新鍵、碰撞與視圖同步的規則。

## 驗收方向
GM 用不同字形寫同層既有鍵時落回原鍵；合法新鍵照收；碰撞不產生重複鍵。
