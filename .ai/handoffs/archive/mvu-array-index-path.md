> 結案 2026-10-12：verify 12 步綠；三方驗收 PASS。

# 未重構 MVU 卡陣列索引路徑

Status: done

## Summary
沒重構 MVU 卡的陣列索引路徑（`/HP/0`、`insert /清單/-`、`remove /清單/1`）改在帶型別視圖的 JSON 上照上游 MagVarUpdate 套用、整列寫回葉子；未改元素保留原始 JSON，非有限數標記隨 splice 位移。前端墊片同步三項刻意偏離（VWD 新值陣列整換、頂層 insert 放行、splice 標記位移），parity 17 例兩端一致。規格、偏離與已知限制見 [plans/mvu-array-index-path.md](../../plans/mvu-array-index-path.md)。後續：`mvu-meta-schema`（非 zod 卡 schema 檢查）、`mvu-zod-semantics`（zod 卡實際套用語意），由主線立案。
