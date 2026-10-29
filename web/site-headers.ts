// 站點標頭（計畫 2.4、D1）：建置時寫出 Cloudflare Pages 的 `_headers`，宿主頁 CSP 只由這裡給（不另放 meta）。
// 端點與執行時同一份解析：只有 e2e 模式才換成本機假端點。
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";
import { resolveEndpoints } from "./src/shared/endpoints/resolve";

const origin = (url: string) => new URL(url).origin;

export function hostCsp(mode: string, env: Record<string, string | undefined>): string {
  const endpoints = resolveEndpoints(mode, env);
  return [
    "default-src 'self'",
    "script-src 'self'",
    "style-src 'self'",
    // 訊息的行內 style 已由消毒器限成字色、字型類且不含函式（src/shared/ui/story-inline-style.ts）；<style> 元素照擋
    "style-src-attr 'unsafe-inline'",
    // 第二層：外部圖片先在渲染時就換成替代文字（src/shared/ui/host-markdown.ts），CSP 再擋一次
    "img-src 'self' data:",
    "font-src 'self'",
    `connect-src ${origin(endpoints.openrouterApi)} ${origin(endpoints.githubApi)}`,
    // 卡片介面只掛同站的 sandbox.html（沙盒 iframe、不透明來源）
    "frame-src 'self'",
    "frame-ancestors 'none'",
    "object-src 'none'",
    "base-uri 'none'",
    "form-action 'none'",
  ].join("; ");
}

/** 沙盒文件只准本站嵌入（外站一般 iframe 嵌進來時來源是本站）；寬政策由 sandbox.html 自帶的 meta 給。 */
export const SANDBOX_CSP = "frame-ancestors 'self'";

/** zod 卡隔離宿主頁（public/mvu-zod.html），部署與開發伺服器讀同一份 */
export const ZOD_FRAME_FILE = fileURLToPath(new URL("./public/mvu-zod.html", import.meta.url));
export const readZodFrame = () => readFileSync(ZOD_FRAME_FILE, "utf8");

/**
 * zod 卡隔離宿主頁的 CSP（mvu-zod-semantics 規格 4 節）：只准那段行內開機碼（sha256 對實際輸出的文件內容算）與
 * blob:（引擎是在不透明來源裡建的 blob Worker），`'wasm-unsafe-eval'` 只為 Worker 編譯 QuickJS 的 wasm（JS 的
 * eval 照擋；卡片在 QuickJS 裡，看不到 WebAssembly）；不准連線、不准巢狀框；`sandbox allow-scripts` 讓這份文件
 * 就算被直接打開也永遠是不透明來源。Worker 由 blob 建立，沿用這份政策。
 */
export function zodFrameCsp(html: string): string {
  const scripts = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)];
  if (scripts.length !== 1) throw new Error("mvu-zod.html 必須剛好一段行內開機碼");
  const hash = createHash("sha256").update(scripts[0][1], "utf8").digest("base64");
  return [
    "default-src 'none'",
    `script-src 'sha256-${hash}' blob: 'wasm-unsafe-eval'`,
    "worker-src blob:",
    "connect-src 'none'",
    "frame-ancestors 'self'",
    "base-uri 'none'",
    "form-action 'none'",
    "sandbox allow-scripts",
  ].join("; ");
}

/**
 * Cloudflare Pages `_headers`：同一標頭被多條規則命中會用逗號併成兩份政策（只會更嚴），所以 CSP 不掛 `/*`，
 * 宿主 CSP 只給 `/`（站內只用 `#download` hash 路由，宿主頁只有這一個路徑；有 404.html，Pages 不做 SPA 退回），
 * 沙盒與 zod 隔離宿主頁各給一條。Pages 會把 `/x.html` 308 轉到 `/x`，兩個路徑都列。
 * index.html 用 Pages 預設的 `max-age=0, must-revalidate`；帶 hash 的資產長快取。
 */
export function siteHeaders(mode: string, env: Record<string, string | undefined>, zodFrame = readZodFrame()): string {
  const zodCsp = zodFrameCsp(zodFrame);
  return [
    "# 由 web/site-headers.ts 建置時產生，不要手改",
    "/*",
    "  X-Content-Type-Options: nosniff",
    "  Referrer-Policy: no-referrer",
    "/",
    `  Content-Security-Policy: ${hostCsp(mode, env)}`,
    "/sandbox",
    `  Content-Security-Policy: ${SANDBOX_CSP}`,
    "/sandbox.html",
    `  Content-Security-Policy: ${SANDBOX_CSP}`,
    "/mvu-zod",
    `  Content-Security-Policy: ${zodCsp}`,
    "/mvu-zod.html",
    `  Content-Security-Policy: ${zodCsp}`,
    "/assets/*",
    "  Cache-Control: public, max-age=31536000, immutable",
    "",
  ].join("\n");
}

export function siteHeadersPlugin(mode: string): Plugin {
  return {
    name: "tt-site-headers",
    apply: "build",
    generateBundle() {
      this.emitFile({ type: "asset", fileName: "_headers", source: siteHeaders(mode, process.env) });
    },
  };
}

/** 開發伺服器：zod 隔離宿主頁照樣帶部署時那份 CSP（兩種網址都是；每次讀檔重算 hash）。開發時宿主頁本身沒有 CSP */
export function zodFrameDevPlugin(): Plugin {
  return {
    name: "tt-zod-frame-dev",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const path = (request.url ?? "").split("?")[0];
        if (path !== "/mvu-zod" && path !== "/mvu-zod.html") return next();
        const html = readZodFrame();
        response.setHeader("Content-Security-Policy", zodFrameCsp(html));
        response.setHeader("Content-Type", "text/html; charset=utf-8");
        response.end(html);
      });
    },
  };
}
