import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { mvuEnginePlugin } from "./mvu-engine-plugin";

export default defineConfig({
  plugins: [react(), mvuEnginePlugin()],
  resolve: {
    alias: { "@desktop": fileURLToPath(new URL("../src", import.meta.url)) },
  },
  test: {
    // web/ 根的建置外掛（site-headers.ts 等）測試同住在根層
    include: ["src/**/*.test.{ts,tsx}", "*.test.ts"],
  },
});
