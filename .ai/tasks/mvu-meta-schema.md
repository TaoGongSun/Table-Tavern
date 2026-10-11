# 非 zod MVU 卡的 schema 檢查（可擴充、必填鍵、模板）

Status: todo

## Summary
上游 MVU 對舊式（`$meta`／`$__META_EXTENSIBLE__$`／`$arrayMeta`）卡會由 generateSchema 產生 schema：陣列與物件預設不可擴充（insert 拒收）、鍵預設必填（remove 拒收）、insert 進陣列套 `$arrayMeta.$meta.template`。zod 版卡（registerMvuSchema）schema 為 any，全部不檢查。本地目前兩種都不檢查。照酒館行為補上非 zod 卡的這套檢查：匯入時算出 schema 資訊（注意巨集改鍵名、撞名、重複匯入、撤銷還原、落檔讀回），套用 insert／remove 時檢查並回饋 GM。mvu-array-index-path 審查時拆出。

## Next action
未排程；mvu-array-index-path 結案後再排。開工先找有 `$meta` 標記的非 zod 實卡確認。
