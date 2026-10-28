# grok-image-original-leftover — grok 生圖原始檔留在 app 的 grok-home

## Summary
2026-10-28 實測第 8／21② 項：grok 生圖流程本身正確（工作目錄清空、圖庫是 PNG），但 grok 把原始 JPEG 存在 `grok-home/sessions/<workspace>/<session>/images/`（一張約 238 KB），每生一次圖多一份，app 不清。

## Next action
未排程。生圖完成並進圖庫後，清掉該次 session 的 images（或整個生圖 session），比照工作目錄清理；先確認 grok 是否還需要它續用。

## Constraints
- 只清 app 自己的 grok-home。
