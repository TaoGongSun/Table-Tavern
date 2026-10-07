// 正式建置的部署前 smoke（npm run smoke:dist）：`vite build` 出 dist/（官方端點），照 Cloudflare Pages 的行為
// 託管（static-server.mjs 套 `_headers`），用 WebKit 確認：宿主 CSP 只放行 OpenRouter 與 GitHub API、
// 不擋宿主頁本身、外連會被擋；沙盒那條路徑不被宿主 CSP 罩，卡片文件的行內腳本與 eval 跑得動。
// 外部請求一律在瀏覽器裡攔下（GitHub 回 404、其餘中止），不連任何外部服務。
import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";
import { webkit } from "playwright";
import { startStaticServer } from "./static-server.mjs";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const step = (name) => console.log(`• ${name}`);

await build({ root: ROOT, logLevel: "warn" });
const server = await startStaticServer(join(ROOT, "dist"));
const browser = await webkit.launch();

try {
  step("宿主頁的 CSP 標頭用正式端點");
  const csp = (await fetch(server.url)).headers.get("content-security-policy");
  assert.match(csp, /connect-src https:\/\/openrouter\.ai https:\/\/api\.github\.com;/);
  assert.match(csp, /script-src 'self';/);
  assert.equal((await fetch(new URL("sandbox", server.url))).headers.get("content-security-policy"), "frame-ancestors 'self'");

  step("宿主頁在 CSP 底下正常開站（只有攔下的外部請求，沒有 CSP 擋下的資源）");
  const page = await browser.newPage({ locale: "zh-TW" });
  const consoleErrors = [];
  page.on("console", (message) => message.type() === "error" && consoleErrors.push(message.text()));
  const external = [];
  await page.route(/^https?:\/\/(?!127\.0\.0\.1)/, (route) => {
    external.push(route.request().url());
    if (route.request().url().startsWith("https://api.github.com/")) {
      return route.fulfill({ status: 404, contentType: "application/json", headers: { "access-control-allow-origin": "*" }, body: "{}" });
    }
    return route.abort();
  });
  await page.goto(server.url);
  await page.getByText("連接 OpenRouter 就能開始玩").waitFor();
  await page.getByTestId("download-link").click();
  await page.getByText("桌面版還沒有正式版").waitFor();
  assert.ok(external.some((url) => url.startsWith("https://api.github.com/repos/TaoGongSun/Table-Tavern/releases/latest")));
  assert.deepEqual(consoleErrors.filter((text) => /Content Security Policy|Refused to/i.test(text)), []);

  step("CSP 真的生效：宿主頁連不在名單上的網址會被擋");
  const blocked = await page.evaluate(
    () =>
      new Promise((resolve) => {
        setTimeout(() => resolve("（10 秒內沒有 CSP 攔截）"), 10_000);
        document.addEventListener("securitypolicyviolation", (event) => resolve(event.blockedURI), { once: true });
        fetch("https://example.com/probe").catch(() => {});
      }),
  );
  assert.match(blocked, /example\.com/);

  step("沙盒：同站 sandbox.html 掛進 iframe，卡片文件的行內腳本與 eval 跑得動");
  const reported = await page.evaluate(
    () =>
      new Promise((resolve, reject) => {
        const frame = document.createElement("iframe");
        frame.setAttribute("sandbox", "allow-scripts");
        frame.src = "/sandbox.html";
        const timer = setTimeout(() => reject(new Error("沙盒沒有回報")), 10_000);
        window.addEventListener("message", (event) => {
          if (event.source !== frame.contentWindow || event.data?.source !== "table-tavern-card") return;
          if (event.data.kind === "probe") {
            clearTimeout(timer);
            resolve(event.data.value);
          }
        });
        frame.addEventListener(
          "load",
          () => {
            const html =
              "<!doctype html><html><body><p>smoke</p><script>" +
              "parent.postMessage({source:'table-tavern-card',kind:'probe',value:eval('1+1')},'*')" +
              "</script></body></html>";
            frame.contentWindow.postMessage({ source: "table-tavern-host", kind: "load", token: "smoke", html }, "*");
          },
          { once: true },
        );
        document.body.append(frame);
      }),
  );
  assert.equal(reported, 2);

  step("反例：沒掛 sandbox 的 iframe（來源是本站）收到卡片也不寫，頁面留空");
  const plain = await page.evaluate(
    () =>
      new Promise((resolve) => {
        const frame = document.createElement("iframe");
        frame.src = "/sandbox.html";
        let probed = false;
        window.addEventListener("message", (event) => {
          if (event.source === frame.contentWindow && event.data?.kind === "probe") probed = true;
        });
        frame.addEventListener(
          "load",
          () => {
            const html = "<!doctype html><html><body><p>leak</p><script>parent.postMessage({source:'table-tavern-card',kind:'probe',value:1},'*')</script></body></html>";
            frame.contentWindow.postMessage({ source: "table-tavern-host", kind: "load", token: "smoke", html }, "*");
            setTimeout(() => resolve({ probed, body: frame.contentDocument.body.innerHTML.trim() }), 1500);
          },
          { once: true },
        );
        document.body.append(frame);
      }),
  );
  assert.deepEqual(plain, { probed: false, body: "" });

  step("反例：外站嵌不進沙盒頁（frame-ancestors 'self'）");
  const foreign = await browser.newPage();
  await foreign.route("http://foreign.test/", (route) =>
    route.fulfill({ contentType: "text/html", body: `<iframe id="f" src="${server.url}sandbox"></iframe>` }),
  );
  await foreign.goto("http://foreign.test/");
  await foreign.waitForTimeout(1500);
  const embedded = foreign.frames().find((frame) => frame !== foreign.mainFrame());
  const loaded = embedded ? await embedded.evaluate(() => !!document.querySelector("meta[http-equiv]")).catch(() => false) : false;
  assert.equal(loaded, false, "外站 iframe 不該載到沙盒文件");
  await foreign.close();
  console.log("✓ smoke:dist 全部通過");
} finally {
  await browser.close();
  await server.close();
}
