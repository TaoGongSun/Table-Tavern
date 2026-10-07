import { describe, expect, it } from "vitest";
import { siteHeaders } from "./site-headers";
// 與 e2e／smoke 同一份 Pages 標頭規則模擬（純 JS）
import { headersFor, parseHeadersFile } from "./e2e/static-server.mjs";

const PROD_CSP =
  "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; " +
  "connect-src https://openrouter.ai https://api.github.com; frame-src 'self'; frame-ancestors 'none'; " +
  "object-src 'none'; base-uri 'none'; form-action 'none'";
const SANDBOX = "frame-ancestors 'self'";

describe("site _headers", () => {
  it("writes the exact production file", () => {
    expect(siteHeaders("production", {}).split("\n")).toEqual([
      "# 由 web/site-headers.ts 建置時產生，不要手改",
      "/*",
      "  X-Content-Type-Options: nosniff",
      "  Referrer-Policy: no-referrer",
      "/",
      `  Content-Security-Policy: ${PROD_CSP}`,
      "/sandbox",
      `  Content-Security-Policy: ${SANDBOX}`,
      "/sandbox.html",
      `  Content-Security-Policy: ${SANDBOX}`,
      "/assets/*",
      "  Cache-Control: public, max-age=31536000, immutable",
      "",
    ]);
  });

  it("ignores endpoint overrides outside e2e mode", () => {
    const env = { VITE_TT_OPENROUTER_API: "http://127.0.0.1:9/api/v1", VITE_TT_GITHUB_API: "http://127.0.0.1:9" };
    expect(siteHeaders("production", env)).toContain("connect-src https://openrouter.ai https://api.github.com;");
    expect(siteHeaders("e2e", env)).toContain("connect-src http://127.0.0.1:9 http://127.0.0.1:9;");
  });

  it("puts the host CSP on the host page only and a frame-ancestors-only policy on the sandbox", () => {
    const rules = parseHeadersFile(siteHeaders("production", {}));
    expect(headersFor(rules, "/").get("content-security-policy")).toBe(PROD_CSP);
    for (const path of ["/sandbox", "/sandbox.html"]) {
      expect(headersFor(rules, path).get("content-security-policy")).toBe(SANDBOX);
      expect(headersFor(rules, path).get("referrer-policy")).toBe("no-referrer");
    }
    expect(headersFor(rules, "/assets/index-abc.js").get("cache-control")).toBe("public, max-age=31536000, immutable");
    expect(headersFor(rules, "/").has("cache-control")).toBe(false);
  });

  it("would stack the host CSP onto the sandbox if it were set on /*", () => {
    // 反例：Pages 把多條規則的同名標頭用逗號併起來，宿主 CSP 掛 /* 就會罩住沙盒（行內腳本被擋）
    const hostOnSplat = siteHeaders("production", {}).replace("/*\n", `/*\n  Content-Security-Policy: ${PROD_CSP}\n`);
    const rules = parseHeadersFile(hostOnSplat);
    expect(headersFor(rules, "/sandbox").get("content-security-policy")).toBe(`${PROD_CSP}, ${SANDBOX}`);
  });

  it("joins a header set by two rules with a comma, like Pages", () => {
    const rules = parseHeadersFile("/*\n  X-A: one\n/a\n  X-A: two\n");
    expect(headersFor(rules, "/a").get("x-a")).toBe("one, two");
  });
});
