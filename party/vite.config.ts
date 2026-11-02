import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

// 手機客戶端：純靜態，建置到 dist/，由 Workers Static Assets 跟房間伺服器一起部署（server/workers/wrangler.toml 的 [assets]）。
// 開發時 /api 轉給本機 wrangler dev（npm run dev:server，預設 8787）；Origin 改成目標，否則成員連線被來源檢查擋掉。
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@desktop": fileURLToPath(new URL("../src", import.meta.url)),
      "@web": fileURLToPath(new URL("../web/src", import.meta.url)),
    },
  },
  server: {
    port: 1440,
    fs: { allow: [".."] },
    proxy: { "/api": { target: "http://127.0.0.1:8787", ws: true, headers: { origin: "http://127.0.0.1:8787" } } },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
