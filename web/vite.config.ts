import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { mvuEnginePlugin } from "./mvu-engine-plugin";
import { siteHeadersPlugin } from "./site-headers";

export default defineConfig(({ mode }) => ({
  // 宿主 CSP 走建置產生的 `_headers`（site-headers.ts）；開發伺服器不加（HMR 需要行內腳本）
  plugins: [react(), siteHeadersPlugin(mode), mvuEnginePlugin()],
  resolve: {
    // D2：直接共用桌面版的純 TS 模組
    alias: { "@desktop": fileURLToPath(new URL("../src", import.meta.url)) },
  },
  server: {
    port: 1430,
    // 開發伺服器要能讀到 repo 根的 src/（@desktop）
    fs: { allow: [".."] },
  },
}));
