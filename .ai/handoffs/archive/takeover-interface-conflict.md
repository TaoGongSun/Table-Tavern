> 結案 2026-10-11：佔位容器讓位給定義容器進 main，三方驗收 PASS；畫面驗收（多行值縮排）待 takeover-text-multiline。

# 重構接管：多來源產出同欄位就整組介面不套

Status: done

## Summary
RPGImmortal「格式增强」（佔位容器）與「美化状态栏」（定義容器）都產出行動選項，合併判衝突整組丟。改成佔位容器讓位給定義容器（`src/features/refactor/refactor-slot.ts`），規格、驗證、實測與已知限制見 [plans/takeover-interface-conflict.md](../../plans/takeover-interface-conflict.md)。
