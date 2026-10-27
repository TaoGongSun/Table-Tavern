import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { mvuEnginePlugin } from "./mvu-engine-plugin";

// zod 隔離引擎的原始碼（virtual:mvu-zod-engine）只在真瀏覽器用得到：happy-dom 沒有不透明來源的 iframe 與 Worker，
// 單元測試以 zod/test-runner.ts 代替，這裡給空字串免得每次跑測試都另打一包
const zodEngineStub = {
  name: "tt-mvu-zod-engine-stub",
  resolveId: (id: string) => (id === "virtual:mvu-zod-engine" ? "\0virtual:mvu-zod-engine" : null),
  load: (id: string) => (id === "\0virtual:mvu-zod-engine" ? 'export default "";' : null),
};

export default defineConfig({
  plugins: [react(), mvuEnginePlugin(), zodEngineStub],
  resolve: {
    alias: { "@desktop": fileURLToPath(new URL("../src", import.meta.url)) },
  },
  test: {
    // web/ 根的建置外掛（site-headers.ts 等）測試同住在根層
    include: ["src/**/*.test.{ts,tsx}", "*.test.ts"],
  },
});
