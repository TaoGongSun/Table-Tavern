import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { resolveEndpoints } from "./src/shared/endpoints/resolve";

const origin = (url: string) => new URL(url).origin;

// 宿主頁 CSP（計畫 2.4）：不載第三方 JS、只連 OpenRouter 與 GitHub API。只在建置時加——
// 開發伺服器的 HMR 需要行內腳本。包 9 改成 Cloudflare `_headers` 標頭（D1）。
// 端點與執行時同一份解析：只有 e2e 模式才換成本機假端點。
function hostCsp(mode: string): string {
  const endpoints = resolveEndpoints(mode, process.env);
  return [
    "default-src 'self'",
    "script-src 'self'",
    "style-src 'self'",
    // 第二層：外部圖片先在渲染時就換成替代文字（src/shared/ui/host-markdown.ts），CSP 再擋一次
    "img-src 'self' data:",
    "font-src 'self'",
    `connect-src ${origin(endpoints.openrouterApi)} ${origin(endpoints.githubApi)}`,
    "frame-src 'none'",
    "object-src 'none'",
    "base-uri 'none'",
    "form-action 'none'",
  ].join("; ");
}

function hostSecurityMeta(mode: string): Plugin {
  return {
    name: "tt-host-security-meta",
    apply: "build",
    transformIndexHtml: () => [
      {
        tag: "meta",
        attrs: { "http-equiv": "Content-Security-Policy", content: hostCsp(mode) },
        injectTo: "head-prepend",
      },
    ],
  };
}

export default defineConfig(({ mode }) => ({
  plugins: [react(), hostSecurityMeta(mode)],
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
