# ui-text-polish — 介面文字三處小毛病

## Summary
2026-10-02 實測梯 1 順手發現：
- (a) en／de 未儲存計數單數錯誤：「1 unsaved changes」「1 ungespeicherte Änderungen」。
- (b) 新手連線說明各語系長短不一：zh-TW 是短版三句，en／de 是長版。
- (c) de 刪除確認窗角色名後的右引號被擠到下一行（„亚瑟·晨光 換行 “）。

## Next action
(a) 改用複數規則；(b) 先定以哪版為準再對齊十語系；(c) 查確認窗換行規則，讓引號跟著名字。
