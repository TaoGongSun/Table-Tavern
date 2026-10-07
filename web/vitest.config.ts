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
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
