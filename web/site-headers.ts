// 站點標頭（計畫 2.4、D1）：建置時寫出 Cloudflare Pages 的 `_headers`，宿主頁 CSP 只由這裡給（不另放 meta）。
// 端點與執行時同一份解析：只有 e2e 模式才換成本機假端點。
import type { Plugin } from "vite";
import { resolveEndpoints } from "./src/shared/endpoints/resolve";

const origin = (url: string) => new URL(url).origin;

export function hostCsp(mode: string, env: Record<string, string | undefined>): string {
  const endpoints = resolveEndpoints(mode, env);
  return [
    "default-src 'self'",
    "script-src 'self'",
    "style-src 'self'",
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

/**
 * Cloudflare Pages `_headers`：同一標頭被多條規則命中會用逗號併成兩份政策（只會更嚴），所以 CSP 不掛 `/*`，
 * 宿主 CSP 只給 `/`（站內只用 `#download` hash 路由，宿主頁只有這一個路徑；有 404.html，Pages 不做 SPA 退回），
 * 沙盒路徑另給一條。Pages 會把 `/sandbox.html` 308 轉到 `/sandbox`，兩個路徑都列。
 * index.html 用 Pages 預設的 `max-age=0, must-revalidate`；帶 hash 的資產長快取。
 */
export function siteHeaders(mode: string, env: Record<string, string | undefined>): string {
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
