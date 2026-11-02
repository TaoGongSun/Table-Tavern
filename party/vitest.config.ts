import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";

// 房間伺服器核心用 Node 跑（不用 Workers 測試池）；殼以薄的假 DurableObjectState／WebSocket 測關鍵路徑
export default defineConfig({
  resolve: {
    alias: {
      "@desktop": fileURLToPath(new URL("../src", import.meta.url)),
      "@web": fileURLToPath(new URL("../web/src", import.meta.url)),
    },
  },
  test: { include: ["server/**/*.test.ts"] },
});
