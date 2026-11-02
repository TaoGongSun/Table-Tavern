import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

// 房間伺服器核心用 Node 跑（不用 Workers 測試池）；殼以薄的假 DurableObjectState／WebSocket 測關鍵路徑。
// 手機客戶端（src/）的測試各自標 happy-dom，拿伺服器核心當假伺服器做整合測試。
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@desktop": fileURLToPath(new URL("../src", import.meta.url)),
      "@web": fileURLToPath(new URL("../web/src", import.meta.url)),
    },
  },
  test: { include: ["server/**/*.test.ts", "src/**/*.test.{ts,tsx}"] },
});
