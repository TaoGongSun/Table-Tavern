// zod 卡隔離引擎的建置外掛（mvu-zod-semantics 規格 4 節）：
// - `virtual:mvu-quickjs-assets`：桌面同一支 MVU bundle（scripts/build-mvu-bundle.mjs 同一份設定，在記憶體裡打）與
//   QuickJS wasm（release-sync，base64 內嵌：主頁 CSP 的 connect-src 不放行本站，不另外抓檔）。
// - `virtual:mvu-zod-engine`：隔離 Worker（src/features/mvu/zod/worker-entry.ts）連同上面兩樣打成一支 classic IIFE，
//   以文字匯出。主頁把這段文字交給不透明來源的 iframe 建 blob Worker：那份 CSP 只准 blob:、不准連線，所以要整支自足、
//   不能拆 chunk。wasm 在 Worker 裡編譯（隔離頁 CSP 有 'wasm-unsafe-eval'，主頁沒有）。
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { build, type Plugin, type Rollup } from "vite";
// 純 JS 建置腳本（root 的 scripts/），桌面與網頁共用同一份設定
import { buildMvuBundleSource } from "../scripts/build-mvu-bundle.mjs";

const ASSETS = "virtual:mvu-quickjs-assets";
const ENGINE = "virtual:mvu-zod-engine";
const ENTRY = fileURLToPath(new URL("./src/features/mvu/zod/worker-entry.ts", import.meta.url));
const require = createRequire(import.meta.url);

interface Assets {
  bundle: string;
  wasm: string;
}

let assets: Promise<Assets> | null = null;
/** 一個行程只打一次 */
function loadAssets(): Promise<Assets> {
  assets ??= (async () => ({
    bundle: (await buildMvuBundleSource()) as string,
    wasm: readFileSync(require.resolve("@jitl/quickjs-ng-wasmfile-release-sync/wasm")).toString("base64"),
  }))();
  return assets;
}

const assetsPlugin = (load: () => Promise<Assets>): Plugin => ({
  name: "tt-mvu-quickjs-assets",
  resolveId: (id) => (id === ASSETS ? `\0${ASSETS}` : null),
  async load(id) {
    if (id !== `\0${ASSETS}`) return null;
    const { bundle, wasm } = await load();
    return `export const bundle = ${JSON.stringify(bundle)};\nexport const wasm = ${JSON.stringify(wasm)};\n`;
  },
});

export const mvuQuickJsAssetsPlugin = (): Plugin => assetsPlugin(loadAssets);

/** Worker 先以佔位字串建置，建完再換成資產：大字串不進 Worker 的建置與壓縮（vite 的 IIFE 收尾會在程式碼裡找
 *  `(function(` 插入輔助碼，bundle 原文裡也有這個樣子，會被插錯位置） */
const PLACEHOLDER = { bundle: "__TT_MVU_BUNDLE__", wasm: "__TT_QJS_WASM__" };

export async function buildZodEngineSource(): Promise<string> {
  const output = (await build({
    configFile: false,
    logLevel: "warn",
    plugins: [assetsPlugin(async () => PLACEHOLDER)],
    // emscripten 載入器會拿 import.meta.url 拼 wasm 網址（wasm 位元組已直接給，不會真的抓）；IIFE 沒有 import.meta
    define: { "process.env.NODE_ENV": JSON.stringify("production"), "import.meta.url": JSON.stringify("https://mvu-zod.invalid/") },
    build: {
      write: false,
      minify: true,
      lib: { entry: ENTRY, formats: ["iife"], name: "TTMvuZod" },
      rollupOptions: { output: { inlineDynamicImports: true } },
    },
  })) as Rollup.RollupOutput[];
  const chunks = output.flatMap((result) => result.output).filter((item) => item.type === "chunk");
  if (chunks.length !== 1) throw new Error(`zod 引擎應打成一支檔，實得 ${chunks.length} 支`);
  let code = chunks[0].code;
  const real = await loadAssets();
  for (const key of ["bundle", "wasm"] as const) {
    const pieces = code.split(JSON.stringify(PLACEHOLDER[key]));
    if (pieces.length !== 2) throw new Error(`zod 引擎裡的 ${key} 佔位應剛好一處，實得 ${pieces.length - 1} 處`);
    code = pieces.join(JSON.stringify(real[key]));
  }
  return code;
}

export function mvuZodEnginePlugin(): Plugin {
  let source: Promise<string> | null = null;
  return {
    name: "tt-mvu-zod-engine",
    resolveId: (id) => (id === ENGINE ? `\0${ENGINE}` : null),
    async load(id) {
      if (id !== `\0${ENGINE}`) return null;
      source ??= buildZodEngineSource();
      return `export default ${JSON.stringify(await source)};`;
    },
  };
}
