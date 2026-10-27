# initvar-nonzod-overwrite — 非 zod 桌開場白 initvar 不覆寫初始值

## Summary
非 zod 桌的開場白 `<initvar>` 不會覆寫初始值，目前只在 zod 路徑補了這段。

## 已知線索
- 相關：[initvar-entry-yaml-semantics](initvar-entry-yaml-semantics.md)、mvu-zod-semantics。其餘未查。

## Next action
未排程；排在 mvu-zod-semantics 合併之後。

## 驗收方向
非 zod 桌開場白帶 `<initvar>` 時，初始值被覆寫，與 zod 路徑行為一致。
