import { configDefaults, defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// 一般單元測試（npm test／verify）：*.webkit.test.tsx 要真瀏覽器，只由 vitest.webkit.config.ts 收；
// .claude/ 底下是子代理工作樹，不算本 repo 的測試
export default defineConfig({
  plugins: [react()],
  test: {
    exclude: [...configDefaults.exclude, "**/*.webkit.test.tsx", "**/.claude/**"],
  },
});
