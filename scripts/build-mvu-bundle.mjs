// 桌面 QuickJS 宿主的 MVU bundle（mvu-zod-semantics 規格 2.3 節）：`npm run build:mvu-bundle`。
// src/features/mvu-engine/quickjs-entry.ts 連同 zod、lodash、值解析函式庫打成單一 IIFE（全域名 TTMvu），
// 輸出 src-tauri/gen/mvu-engine.js（gitignore），Rust 以 include_str! 嵌入；缺檔時 src-tauri/build.rs 會擋。
import { fileURLToPath } from "node:url";
import { build } from "vite";

const ROOT = fileURLToPath(new URL("..", import.meta.url));

await build({
  root: ROOT,
  configFile: false,
  publicDir: false,
  logLevel: "warn",
  // 函式庫裡的開發分支：QuickJS 沒有 process，照正式版取值
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  build: {
    lib: {
      entry: fileURLToPath(new URL("../src/features/mvu-engine/quickjs-entry.ts", import.meta.url)),
      formats: ["iife"],
      name: "TTMvu",
      fileName: () => "mvu-engine.js",
    },
    outDir: fileURLToPath(new URL("../src-tauri/gen/", import.meta.url)),
    // gen/ 底下還有 Tauri 產生的 schemas，不能清空
    emptyOutDir: false,
    target: "es2020",
    minify: "esbuild",
    reportCompressedSize: false,
    rollupOptions: {
      // zod 原始碼裡 @__PURE__ 註解的位置 Rollup 看不懂、會自動拿掉，不影響輸出
      onwarn(warning, warn) {
        if (warning.code !== "INVALID_ANNOTATION") warn(warning);
      },
    },
  },
});
