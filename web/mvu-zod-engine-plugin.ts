// `virtual:mvu-zod-engine`：把 zod 卡的隔離 Worker（src/features/mvu/zod/worker-entry.ts：共用核心＋zod＋值解析）
// 打成一支 classic IIFE，以文字匯出。主頁把這段文字交給不透明來源的 iframe 建 blob Worker（mvu-zod-semantics 規格
// 4 節）：那份 CSP 只准 blob:、不准同站資源，所以要整支自足、不能拆 chunk。宿主頁的 connect-src 不必放行本站。
import { fileURLToPath } from "node:url";
import { build, type Plugin, type Rollup } from "vite";
import { mvuEnginePlugin } from "./mvu-engine-plugin";

const ID = "virtual:mvu-zod-engine";
const RESOLVED = `\0${ID}`;
const ENTRY = fileURLToPath(new URL("./src/features/mvu/zod/worker-entry.ts", import.meta.url));

export async function buildZodEngineSource(): Promise<string> {
  const output = (await build({
    configFile: false,
    logLevel: "warn",
    plugins: [mvuEnginePlugin()],
    resolve: { alias: { "@desktop": fileURLToPath(new URL("../src", import.meta.url)) } },
    define: { "process.env.NODE_ENV": JSON.stringify("production") },
    build: {
      write: false,
      minify: true,
      lib: { entry: ENTRY, formats: ["iife"], name: "TTMvuZod" },
      rollupOptions: { output: { inlineDynamicImports: true } },
    },
  })) as Rollup.RollupOutput[];
  const chunks = output.flatMap((result) => result.output).filter((item) => item.type === "chunk");
  if (chunks.length !== 1) throw new Error(`zod 引擎應打成一支檔，實得 ${chunks.length} 支`);
  return chunks[0].code;
}

export function mvuZodEnginePlugin(): Plugin {
  let source: Promise<string> | null = null;
  return {
    name: "tt-mvu-zod-engine",
    resolveId: (id) => (id === ID ? RESOLVED : null),
    async load(id) {
      if (id !== RESOLVED) return null;
      source ??= buildZodEngineSource();
      return `export default ${JSON.stringify(await source)};`;
    },
  };
}
