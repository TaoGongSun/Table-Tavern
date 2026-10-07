// 照 Cloudflare Pages 的靜態託管行為起一個本機伺服器，讓 e2e／smoke 跑在部署時會拿到的標頭底下：
// 讀產物裡的 `_headers`（路徑規則、`*` 萬用、`:名稱` 佔位、`!` 拆標頭、同名標頭以逗號合併），
// `/x.html` 308 轉 `/x`、`/x` 送 x.html，找不到的路徑有 404.html 就回它（404），沒有才退回 index.html（SPA），
// `_headers` 沒給 Cache-Control 時補 Pages 預設值。與真 Pages 的差異（站上實測）：
// - 308 轉址回應這裡不帶標頭，Pages 會帶 `/*` 的 nosniff／Referrer-Policy；404 回應這裡也不套 `_headers`。
// - `//` 這裡回 404，Pages 會正規化回首頁 200 並帶宿主 CSP。
// - 沒有 ETag／壓縮／邊緣快取，只認本專案用到的副檔名。
import { createServer } from "node:http";
import { existsSync, readFileSync, statSync } from "node:fs";
import { extname, join, normalize } from "node:path";

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".woff2": "font/woff2",
  ".wasm": "application/wasm",
};

// https://developers.cloudflare.com/pages/configuration/serving-pages/
const PAGES_DEFAULT_CACHE = "public, max-age=0, must-revalidate";
const INDEX_ALIASES = new Set(["/index", "/index/", "/index.html"]);

export function parseHeadersFile(text) {
  const rules = [];
  for (const raw of text.split("\n")) {
    const line = raw.trimEnd();
    if (!line.trim() || line.trim().startsWith("#")) continue;
    if (!/^\s/.test(line)) {
      rules.push({ pattern: line.trim(), set: [], detach: [] });
      continue;
    }
    const rule = rules.at(-1);
    if (!rule) throw new Error(`_headers：標頭行前面沒有路徑：${line}`);
    const body = line.trim();
    if (body.startsWith("!")) {
      rule.detach.push(body.slice(1).trim().toLowerCase());
    } else {
      const colon = body.indexOf(":");
      if (colon <= 0) throw new Error(`_headers：看不懂這行：${line}`);
      rule.set.push([body.slice(0, colon).trim(), body.slice(colon + 1).trim()]);
    }
  }
  return rules;
}

function matches(pattern, pathname) {
  const source = pattern
    .split("*")
    .map((part) => part.replace(/[.+?^${}()|[\]\\]/g, "\\$&").replace(/:[A-Za-z_]\w*/g, "[^/]+"))
    .join(".*");
  return new RegExp(`^${source}$`).test(pathname);
}

/** 照規則算出這個路徑的標頭（鍵為小寫）。拆標頭作用在所有規則套完之後。 */
export function headersFor(rules, pathname) {
  const out = new Map();
  const detached = new Set();
  for (const rule of rules) {
    if (!matches(rule.pattern, pathname)) continue;
    for (const [name, value] of rule.set) {
      const key = name.toLowerCase();
      out.set(key, out.has(key) ? `${out.get(key)}, ${value}` : value);
    }
    for (const name of rule.detach) detached.add(name);
  }
  for (const name of detached) out.delete(name);
  return out;
}

export async function startStaticServer(dir, port = 0) {
  const headersPath = join(dir, "_headers");
  const rules = existsSync(headersPath) ? parseHeadersFile(readFileSync(headersPath, "utf8")) : [];
  const isFile = (path) => existsSync(path) && statSync(path).isFile();
  const server = createServer((request, response) => {
    // 不用 new URL 解析：`//x` 會被當成主機名
    const queryAt = request.url.indexOf("?");
    const url = { pathname: queryAt < 0 ? request.url : request.url.slice(0, queryAt), search: queryAt < 0 ? "" : request.url.slice(queryAt) };
    const pathname = decodeURIComponent(url.pathname);
    if (pathname.includes("\0") || normalize(pathname).includes("..")) {
      response.writeHead(400).end();
      return;
    }
    // 首頁的別名（/index、/index/、/index.html）一律轉回 /，其餘 .html 轉成無副檔名
    const redirect = INDEX_ALIASES.has(pathname) ? "/" : pathname.endsWith(".html") ? pathname.slice(0, -".html".length) : null;
    if (redirect !== null) {
      response.writeHead(308, { location: `${redirect}${url.search}` }).end();
      return;
    }
    // 空路段（例：//）不對應任何檔案
    let file = pathname.includes("//") ? "" : join(dir, pathname);
    if (file && pathname.endsWith("/")) file = join(file, "index.html");
    else if (file && !isFile(file) && isFile(`${file}.html`)) file = `${file}.html`;
    let status = 200;
    let headers = {};
    if (!isFile(file) || pathname === "/_headers") {
      if (isFile(join(dir, "404.html"))) {
        status = 404;
        file = join(dir, "404.html");
      } else {
        file = join(dir, "index.html");
        headers = Object.fromEntries(headersFor(rules, pathname));
      }
    } else {
      headers = Object.fromEntries(headersFor(rules, pathname));
    }
    headers["cache-control"] ??= PAGES_DEFAULT_CACHE;
    headers["content-type"] = TYPES[extname(file)] ?? "application/octet-stream";
    response.writeHead(status, headers).end(request.method === "HEAD" ? undefined : readFileSync(file));
  });
  await new Promise((resolve) => server.listen(port, "127.0.0.1", resolve));
  const { port: bound } = server.address();
  return {
    url: `http://127.0.0.1:${bound}/`,
    close: () =>
      new Promise((resolve) => {
        server.close(resolve);
        server.closeAllConnections();
      }),
  };
}
