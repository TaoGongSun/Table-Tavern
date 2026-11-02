import { defineConfig } from "vite";

// Node 自架殼打包（核心是無副檔名 import，Node 型別剝除跑不了）：產 dist-node/server.mjs，node_modules 依賴（ws）外部化。
export default defineConfig({
  build: {
    ssr: "server/node/main.ts",
    outDir: "dist-node",
    emptyOutDir: true,
    target: "node22",
    rollupOptions: { output: { entryFileNames: "server.mjs" } },
  },
});
