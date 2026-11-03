import { configDefaults, defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

// 一般單元測試（npm test／verify）：*.webkit.test.tsx 要真瀏覽器，只由 vitest.webkit.config.ts 收；
// .claude/ 底下是子代理工作樹，不算本 repo 的測試；web/ 是網頁版、party/ 是多人房間，各用自己的 vitest 設定跑
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@party": fileURLToPath(new URL("./party/src", import.meta.url)),
      "@desktop": fileURLToPath(new URL("./src", import.meta.url)),
    },
    dedupe: ["react", "react-dom"],
  },
  test: {
    exclude: [...configDefaults.exclude, "**/*.webkit.test.tsx", "**/.claude/**", "web/**", "party/**"],
  },
});
