import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { playwright } from "@vitest/browser-playwright";

// 真實 WebKit 回歸（npm run test:webkit，不進 verify）：焦點可見性這類 happy-dom 算不出來的行為。
// 首次要先 npx playwright install webkit
export default defineConfig({
  plugins: [react()],
  test: {
    include: ["src/**/*.webkit.test.tsx"],
    browser: {
      enabled: true,
      headless: true,
      provider: playwright(),
      instances: [{ browser: "webkit" }],
    },
  },
});
