// QuickJS 宿主的 MVU bundle（mvu-zod-semantics 規格 2.3、4 節）：`npm run build:mvu-bundle`。
// src/features/mvu-engine/quickjs-entry.ts 連同 zod、lodash、值解析函式庫打成單一 IIFE（全域名 TTMvu），
// 輸出 src-tauri/gen/mvu-engine.js（gitignore），Rust 以 include_str! 嵌入；缺檔時 src-tauri/build.rs 會擋。
// 網頁版（web/mvu-zod-engine-plugin.ts）用同一份設定在記憶體裡打同一支，交給 QuickJS-wasm 跑。
import { realpathSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build } from "vite";

const ROOT = fileURLToPath(new URL("..", import.meta.url));

/** 兩邊共用的建置設定；`overrides` 只放輸出位置（寫檔或留在記憶體） */
export function mvuBundleOptions(overrides = {}) {
  return {
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
      target: "es2020",
      minify: "esbuild",
      reportCompressedSize: false,
      rollupOptions: {
        // zod 原始碼裡 @__PURE__ 註解的位置 Rollup 看不懂、會自動拿掉，不影響輸出
        onwarn(warning, warn) {
          if (warning.code !== "INVALID_ANNOTATION") warn(warning);
        },
      },
      ...overrides,
    },
  };
}

/** 打成文字（不寫檔） */
export async function buildMvuBundleSource() {
  const output = await build(mvuBundleOptions({ write: false }));
  const chunks = [output].flat().flatMap((result) => result.output).filter((item) => item.type === "chunk");
  if (chunks.length !== 1) throw new Error(`MVU bundle 應打成一支檔，實得 ${chunks.length} 支`);
  return chunks[0].code;
}

// 直接執行（npm run build:mvu-bundle）才寫檔；被網頁外掛 import 時只提供設定
if (process.argv[1] && import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href) {
  // gen/ 底下還有 Tauri 產生的 schemas，不能清空
  await build(mvuBundleOptions({ outDir: fileURLToPath(new URL("../src-tauri/gen/", import.meta.url)), emptyOutDir: false }));
}
