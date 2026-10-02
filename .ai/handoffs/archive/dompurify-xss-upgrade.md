# Handoff: dompurify-xss-upgrade

2026-10-03 結案：已進 main。

dompurify 3.4.12 → 3.4.16（`^3.4.16`），修 GHSA-55q2-fjhq-7xh7（IN_PLACE 模式 hook 移除節點留下可執行分離子樹）；修補版本依 GitHub advisory 為 3.4.13。`npm audit --omit=dev` 歸零。唯一使用點 `src/shared/ui/story-markdown.ts` 沒用 IN_PLACE／addHook，設定不變，屬防禦性升級。

happy-dom 跑 DOMPurify 不正確（3.4.12–3.4.16 皆拆掉第一個元素、放過後面的 on* 屬性），`story-markdown.test.ts` 只驗得到 renderer 轉義那層；DOMPurify 出口改由 `story-markdown.webkit.test.tsx` 在真 WebKit 驗（`npm run test:webkit`，不進 verify）。
