import { configDefaults, defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// 一般單元測試（npm test／verify）：*.webkit.test.tsx 要真瀏覽器，只由 vitest.webkit.config.ts 收
export default defineConfig({
  plugins: [react()],
  test: {
    exclude: [...configDefaults.exclude, "**/*.webkit.test.tsx"],
  },
});
