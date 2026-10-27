// zod 卡隔離的真瀏覽器驗收（npm run e2e:zod，P5 必跑、不進 verify；mvu-zod-semantics 規格 4 節）：正式建置照
// Cloudflare Pages 的標頭託管（static-server.mjs），用 WebKit（本機有 Google Chrome 就再跑一次）確認：
// - 隔離宿主頁 /mvu-zod 的 CSP hash 對得上實際送出的開機碼、只有它帶 'wasm-unsafe-eval'；開發伺服器兩種網址也帶同一份。
// - 主頁的 wasm 編譯與 JS eval 都被擋；隔離頁的 blob Worker 能編 wasm、JS eval 照擋。
// - 同一份引擎（wasm 版 QuickJS 跑桌面同一支 bundle）跑共用 41 案與真卡，與桌面基準逐項相同。
// - 第一道防線（不透明來源＋CSP，探測引擎直接在 Worker 跑）：存檔讀／寫／刪、網路、WebRTC、eval、非 blob 模組、二層 blob
//   偷載都失敗。
// - 卡片在 QuickJS 裡：巢狀 Worker、存檔、Blob、WebRTC、網路、假結果、WebAssembly 都拿不到（QuickJS 自己的 eval 不受
//   瀏覽器 CSP 管，不在此列）。
// - loader 白名單：白名單網址的靜態、動態（含非字面量，同桌面）import 照樣載入；白名單外的整次失敗，不連網。
// - 上限：transform 期與載入期的記憶體炸彈（字串、物件、ArrayBuffer、TypedArray）整次失敗、線性記憶體封頂 128 MB；
//   遞迴（堆疊）、丟 null、無窮迴圈與 Promise 無窮迴圈整次失敗；炸彈進行中主頁期限到了照樣收掉；之後都能重建。
// - 開機訊息只收父頁、本站：外站直接開頁或嵌入後送開機訊息，惡意引擎不會跑。
// 判定一律看可觀測的結果（另開的探測伺服器有沒有收到請求、卡片自己回報的例外、存檔哨兵還在不在），不靠攔截請求。
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { build, createServer as createViteServer } from "vite";
import YAML from "yaml";
import { chromium, webkit } from "playwright";
import { startStaticServer } from "./static-server.mjs";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const FIXTURES = fileURLToPath(new URL("../../src/features/mvu-engine/fixtures/", import.meta.url));
const fixture = (name) => readFileSync(join(FIXTURES, name), "utf8");
const MVU_ZOD = "https://testingcf.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js";
const step = (name) => console.log(`• ${name}`);
// TestCards/（gitignore、本機限定）有真卡基準就一起比（由 quickjs-entry.test.ts 在 Node 產生，cargo 拿同一份比 QuickJS）
const REAL_GOLDEN = fileURLToPath(new URL("../../TestCards/zod-cards-golden.json", import.meta.url));

/** 探測伺服器（另一個來源）：記下所有請求與 WebSocket 升級；收到任何一筆＝那條路沒擋住 */
async function startProbe() {
  const hits = [];
  const server = createServer((request, response) => {
    hits.push(request.url);
    response.writeHead(200, { "access-control-allow-origin": "*", "content-type": "text/javascript" });
    response.end("export default 1;");
  });
  server.on("upgrade", (request, socket) => {
    hits.push(`ws:${request.url}`);
    socket.destroy();
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  return { url: `http://127.0.0.1:${port}`, hits, close: () => new Promise((resolve) => server.close(resolve)) };
}

/** 外站：給一頁攻擊頁 */
async function startForeign(html) {
  const server = createServer((_request, response) => {
    response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    response.end(html);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return { url: `http://127.0.0.1:${server.address().port}/`, close: () => new Promise((resolve) => server.close(resolve)) };
}

const bootScript = (html) => {
  const scripts = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)];
  assert.equal(scripts.length, 1);
  return `'sha256-${createHash("sha256").update(scripts[0][1], "utf8").digest("base64")}'`;
};

/** 卡片腳本裡的攻擊（整條路：卡片在 QuickJS 裡）：結果寫進 schema 的預設值帶回來 */
function attackCard(probe) {
  return `import { registerMvuSchema } from '${MVU_ZOD}';
const out = {};
const t = async (name, fn) => { try { out[name] = 'ALLOWED ' + String(await fn()); } catch (e) { out[name] = 'blocked ' + String(e && e.name); } };
await t('nestedWorker', () => new Worker('data:text/javascript,1'));
await t('nestedBlobWorker', () => new Worker(URL.createObjectURL(new Blob(['1']))));
await t('idbRead', () => new Promise((res, rej) => { const r = indexedDB.open('tt-web-sentinel'); r.onsuccess = () => res('opened'); r.onerror = () => rej(r.error); }));
await t('idbViaProto', () => Object.getPrototypeOf(globalThis).indexedDB.open('tt-web'));
await t('idbViaCtor', () => globalThis.constructor.prototype.indexedDB.open('tt-web'));
await t('idbDelete', () => indexedDB.deleteDatabase('tt-web-sentinel'));
await t('secondBlob', () => URL.createObjectURL(new Blob(['export default 1'], { type: 'text/javascript' })));
await t('rtc', () => new RTCPeerConnection());
await t('fetch', () => fetch('${probe}/card-fetch'));
await t('xhr', () => { const x = new XMLHttpRequest(); x.open('GET', '${probe}/card-xhr'); x.send(); return 'sent'; });
await t('ws', () => new WebSocket('${probe.replace("http", "ws")}/card-ws'));
await t('eventSource', () => new EventSource('${probe}/card-es'));
await t('forgeReply', () => postMessage({ id: 1, ok: true, json: '{}' }));
await t('selfPost', () => self.postMessage('x'));
await t('importScripts', () => importScripts('${probe}/card-import-scripts'));
await t('caches', () => caches.keys());
await t('broadcast', () => new BroadcastChannel('tt'));
await t('wasm', () => WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0])));
export const Schema = z.object({ probe: z.string().prefault(JSON.stringify(out)) });
$(() => registerMvuSchema(Schema));
`;
}

/** 第一道防線（探測引擎直接在 Worker 跑，由宿主頁照正式流程送進 /mvu-zod）：只靠不透明來源與 CSP。wasmCompile 應放行
 *  （QuickJS 要用），其餘都該擋 */
function layerOneEngine(probe, appOrigin, asset) {
  return `onmessage = async (event) => {
  const port = event.ports[0];
  const out = {};
  const t = async (name, fn) => { try { out[name] = 'ALLOWED ' + String(await fn()); } catch (e) { out[name] = 'blocked ' + String(e && e.name); } };
  const idb = (fn) => new Promise((res, rej) => { const r = fn(); r.onsuccess = () => res('ok ' + String(r.result && r.result.name)); r.onerror = () => rej(r.error); });
  await t('idbRead', () => idb(() => indexedDB.open('tt-web-sentinel')));
  await t('idbAppDb', () => idb(() => indexedDB.open('tt-web')));
  await t('idbDelete', () => idb(() => indexedDB.deleteDatabase('tt-web-sentinel')));
  await t('fetch', () => fetch('${probe}/l1-fetch').then((r) => r.status));
  await t('ws', () => new Promise((res, rej) => { const s = new WebSocket('${probe.replace("http", "ws")}/l1-ws'); s.onopen = () => res('open'); s.onerror = () => rej(new Error('ws')); setTimeout(() => rej(new Error('ws-timeout')), 1500); }));
  await t('rtc', () => typeof RTCPeerConnection === 'undefined' ? Promise.reject(new Error('absent')) : new RTCPeerConnection());
  await t('newFunction', () => new Function('return 1')());
  await t('wasmCompile', () => WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0])).then(() => 'compiled'));
  await t('importProbe', () => import('${probe}/l1-import.js'));
  await t('importAppAsset', () => import('${appOrigin}/assets/${asset}'));
  await t('secondBlobImportsProbe', async () => (await import(URL.createObjectURL(new Blob(['import "${probe}/l1-via-blob.js"; export default 1'], { type: 'text/javascript' })))).default);
  port.postMessage(out);
};
postMessage('up');`;
}

const assertAllBlocked = (out, label, except = []) => {
  for (const [name, value] of Object.entries(out)) {
    if (except.includes(name)) continue;
    assert.match(value, /^blocked /, `${label}：${name} 應失敗，實得 ${value}`);
  }
};

async function checkDevHeaders() {
  step("開發伺服器：/mvu-zod 與 /mvu-zod.html 都帶同一份隔離 CSP，hash 對得上送出的開機碼");
  const vite = await createViteServer({ root: ROOT, logLevel: "warn", server: { port: 0, host: "127.0.0.1" } });
  await vite.listen();
  try {
    const base = vite.resolvedUrls.local[0];
    for (const path of ["mvu-zod", "mvu-zod.html"]) {
      const response = await fetch(new URL(path, base));
      assert.equal(response.status, 200, path);
      const csp = response.headers.get("content-security-policy");
      assert.ok(csp?.includes(`script-src ${bootScript(await response.text())} blob: 'wasm-unsafe-eval';`), `${path}：${csp}`);
      assert.doesNotMatch(csp, /(^|\s)'unsafe-eval'/);
      assert.match(csp, /connect-src 'none'/);
      assert.match(csp, /sandbox allow-scripts/);
    }
  } finally {
    await vite.close();
  }
}

await build({ root: ROOT, logLevel: "warn" });
const dist = join(ROOT, "dist");
const runtimeChunk = readdirSync(join(dist, "assets")).find((name) => /^runtime-.*\.js$/.test(name));
const someAsset = readdirSync(join(dist, "assets")).find((name) => /^index-.*\.js$/.test(name));
assert.ok(runtimeChunk && someAsset, "找不到執行期 chunk");
// 主頁 CSP 探測：放成同站模組由頁面自己載入（測試框架的 evaluate 在 WebKit 不受頁面 CSP 管，不能直接在裡面試）
const HOST_PROBE = "e2e-host-probe.js";
writeFileSync(
  join(dist, HOST_PROBE),
  `export async function probe() {
  const out = {};
  try { await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0])); out.wasm = "ALLOWED"; } catch (error) { out.wasm = "blocked " + error.name; }
  try { out.eval = "ALLOWED " + (0, eval)("1"); } catch (error) { out.eval = "blocked " + error.name; }
  try { out.newFunction = "ALLOWED " + new Function("return 1")(); } catch (error) { out.newFunction = "blocked " + error.name; }
  return out;
}
`,
);
const server = await startStaticServer(dist);
const appOrigin = new URL(server.url).origin;
const probe = await startProbe();

const attackPage = `<!doctype html><meta charset="utf-8"><body><script>
const APP = ${JSON.stringify(appOrigin)};
const PROBE = ${JSON.stringify(probe.url)};
const evil = "onmessage = (e) => { e.ports[0].postMessage('pwned'); fetch(" + JSON.stringify(PROBE + "/pwned") + "); }; postMessage('up');";
window.received = [];
window.attacked = [];
function attack(target, label) {
  window.attacked.push(label + (target ? "" : ":no-window"));
  if (!target) return;
  const worker = new MessageChannel();
  const control = new MessageChannel();
  worker.port1.onmessage = (e) => window.received.push(e.data);
  control.port1.onmessage = (e) => window.received.push(e.data);
  try { target.postMessage({ type: "tt-mvu-zod-boot", engine: evil }, "*", [worker.port2, control.port2]); } catch (e) { window.received.push("post-failed " + e); }
}
const popup = window.open(APP + "/mvu-zod");
setTimeout(() => attack(popup, "popup"), 1500);
for (const sandbox of ["allow-scripts", null]) {
  const frame = document.createElement("iframe");
  if (sandbox) frame.setAttribute("sandbox", sandbox);
  frame.src = APP + "/mvu-zod";
  frame.onload = () => attack(frame.contentWindow, "frame-" + (sandbox || "plain"));
  document.body.append(frame);
}
</script>`;
const foreign = await startForeign(attackPage);

const browsers = [["webkit", () => webkit.launch()]];
browsers.push(["chrome", () => chromium.launch({ channel: "chrome", headless: true })]);

try {
  step("正式建置：/mvu-zod 的 CSP hash 對得上送出的開機碼；/mvu-zod.html 轉到 /mvu-zod");
  const frameResponse = await fetch(new URL("mvu-zod", server.url));
  const frameCsp = frameResponse.headers.get("content-security-policy");
  assert.ok(frameCsp?.includes(`script-src ${bootScript(await frameResponse.text())} blob: 'wasm-unsafe-eval';`), frameCsp ?? "沒有 CSP");
  assert.doesNotMatch(frameCsp, /(^|\s)'unsafe-eval'/);
  const hostCsp = (await fetch(server.url)).headers.get("content-security-policy") ?? "";
  assert.match(hostCsp, /script-src 'self'/, "主頁沒有 CSP");
  assert.doesNotMatch(hostCsp, /unsafe-eval/, "主頁不得放行 eval／wasm");
  assert.match(frameCsp, /default-src 'none'.*connect-src 'none'.*frame-ancestors 'self'.*sandbox allow-scripts/);
  const redirect = await fetch(new URL("mvu-zod.html", server.url), { redirect: "manual" });
  assert.equal(redirect.status, 308);
  assert.equal(redirect.headers.get("location"), "/mvu-zod");
  await checkDevHeaders();

  for (const [name, launch] of browsers) {
    let browser;
    try {
      browser = await launch();
    } catch (error) {
      if (name === "webkit") throw error;
      console.log(`（略過 ${name}：本機啟動不了——${String(error).split("\n")[0]}）`);
      continue;
    }
    console.log(`\n== ${name}`);
    try {
      const page = await browser.newPage();
      const cspErrors = [];
      page.on("console", (message) => message.type() === "error" && /Content Security Policy|Refused to/i.test(message.text()) && cspErrors.push(message.text()));
      // 宿主頁載入時會查 GitHub 最新版：外部請求一律中止（不用來判定隔離）
      await page.route(/^https?:\/\/(?!127\.0\.0\.1)/, (route) => route.abort());
      await page.goto(server.url);
      await page.evaluate(async (chunk) => {
        window.__tt = await import(`/assets/${chunk}`);
        await new Promise((resolve, reject) => {
          const request = indexedDB.open("tt-web-sentinel", 1);
          request.onupgradeneeded = () => request.result.createObjectStore("s");
          request.onsuccess = () => {
            const tx = request.result.transaction("s", "readwrite");
            tx.objectStore("s").put("SENTINEL", "k");
            tx.oncomplete = () => {
              request.result.close();
              resolve();
            };
            tx.onerror = () => reject(tx.error);
          };
          request.onerror = () => reject(request.error);
        });
      }, runtimeChunk);

      step("主頁：wasm 編譯與 JS eval 都被 CSP 擋");
      const hostBlocked = await page.evaluate(async (path) => (await import(`/${path}`)).probe(), HOST_PROBE);
      console.log("  ", JSON.stringify(hostBlocked));
      // WebKit 的文件本身不拿 CSP 管 wasm 編譯（Worker 裡才管；本案沒改主頁 CSP，這是瀏覽器既有行為），只在 Chrome 驗
      assertAllBlocked(hostBlocked, "主頁", name === "webkit" ? ["wasm"] : []);
      cspErrors.length = 0;

      step("同一份引擎跑共用 41 案，與桌面基準逐項相同");
      const golden = JSON.parse(fixture("zod-corpus-golden.json"));
      const output = await page.evaluate(
        async ({ schema, initvar, cases }) => {
          const runner = window.__tt.createZodRunner({ schema });
          const init = await runner.initialize(initvar);
          const results = [];
          for (const { message } of cases) results.push(await runner.parse(message, { stat_data: init.stat_data }, { user: "玩家", char: null }));
          runner.dispose();
          return { init, results };
        },
        { schema: fixture("clean-zod-schema.txt"), initvar: YAML.parse(fixture("clean-zod-initvar.yaml")), cases: golden.cases },
      );
      assert.deepEqual(output.init, golden.init);
      golden.cases.forEach(({ name: label, message: _message, feedback: _feedback, ...expected }, index) => assert.deepEqual(output.results[index], expected, label));
      assert.deepEqual(cspErrors, [], "引擎正常運作時不該有 CSP 違規");

      if (existsSync(REAL_GOLDEN)) {
        step("真卡（TestCards 本機限定）：原文 schema 送進 QuickJS，開局與一批指令和 Node 基準相同");
        for (const card of JSON.parse(readFileSync(REAL_GOLDEN, "utf8"))) {
          const actual = await page.evaluate(
            async ({ schema, initvar, message }) => {
              const runner = window.__tt.createZodRunner({ schema });
              try {
                const init = await runner.initialize(initvar);
                return { init, detailed: await runner.parse(message, { stat_data: init.stat_data }, { user: "玩家", char: null }) };
              } finally {
                runner.dispose();
              }
            },
            card,
          );
          // 基準是桌面入口的輸出、另帶回饋句（feedback）；網頁 runner 只回契約本體
          const { feedback: _feedback, ...detailed } = card.detailed;
          assert.deepEqual(actual, { init: card.init, detailed }, card.name);
        }
      } else {
        console.log("（略過真卡：沒有 TestCards/zod-cards-golden.json）");
      }

      step("第一道防線：不透明來源＋CSP 擋下存檔、網路、eval、非 blob 模組與二層 blob 偷載；blob Worker 繼承得到 wasm 編譯");
      const layerOne = await page.evaluate(
        (engine) =>
          new Promise((resolve, reject) => {
            const frame = document.createElement("iframe");
            frame.setAttribute("sandbox", "allow-scripts");
            frame.src = "/mvu-zod.html";
            const worker = new MessageChannel();
            const control = new MessageChannel();
            const timer = setTimeout(() => reject(new Error("第一道防線探測沒有回報")), 15_000);
            worker.port1.onmessage = (event) => {
              clearTimeout(timer);
              control.port1.postMessage("kill");
              frame.remove();
              resolve(event.data);
            };
            control.port1.onmessage = (event) => reject(new Error(JSON.stringify(event.data)));
            frame.addEventListener("load", () => frame.contentWindow.postMessage({ type: "tt-mvu-zod-boot", engine }, "*", [worker.port2, control.port2]), { once: true });
            document.body.append(frame);
          }),
        layerOneEngine(probe.url, appOrigin, someAsset),
      );
      console.log("  ", JSON.stringify(layerOne));
      assertAllBlocked(layerOne, "第一道防線", ["wasmCompile"]);
      assert.equal(layerOne.wasmCompile, "ALLOWED compiled", "隔離頁的 blob Worker 要能編 wasm");

      step("整條路：卡片（QuickJS 裡）的巢狀 Worker、存檔讀寫刪、Blob、WebRTC、網路、假結果、WebAssembly 全部失敗");
      const attacked = await page.evaluate(async (schema) => {
        const runner = window.__tt.createZodRunner({ schema });
        try {
          return (await runner.initialize({})).stat_data.probe;
        } finally {
          runner.dispose();
        }
      }, attackCard(probe.url));
      const cardOut = JSON.parse(attacked);
      console.log("  ", JSON.stringify(cardOut));
      assertAllBlocked(cardOut, "卡片腳本");

      step("白名單網址的動態 import 照樣載入（同桌面 loader，含非字面量），頂層 await 與非頂層（then）都能註冊");
      const dynamic = await page.evaluate(async (mvuZod) => {
        const out = [];
        for (const schema of [
          `const m = await import('${mvuZod}');\nm.registerMvuSchema(z.object({ a: z.number() }));`,
          `import('${mvuZod}').then((m) => m.registerMvuSchema(z.object({ a: z.number() })));`,
          `const m = await import(\`${mvuZod}\`);\nm.registerMvuSchema(z.object({ a: z.number() }));`,
          `const u = '${mvuZod}'.slice(0);\nconst m = await import(u);\nm.registerMvuSchema(z.object({ a: z.number() }));`,
        ]) {
          const runner = window.__tt.createZodRunner({ schema });
          try {
            out.push((await runner.parse("_.set('a', 1);", { stat_data: { a: 0 } }, null)).table.stat_data);
          } finally {
            runner.dispose();
          }
        }
        return out;
      }, MVU_ZOD);
      assert.deepEqual(dynamic, [{ a: 1 }, { a: 1 }, { a: 1 }, { a: 1 }]);

      step("白名單外的 import（靜態、字面量與非字面量動態、墊片內部名）：整次失敗（schema-load），不連網");
      for (const schema of [
        `import x from '${probe.url}/static.js';\nimport { registerMvuSchema } from '${MVU_ZOD}';\n$(() => registerMvuSchema(z.object({})));`,
        `import { registerMvuSchema } from '${MVU_ZOD}';\nconst u = '${probe.url}/' + 'dyn.js';\nawait import(u);\n$(() => registerMvuSchema(z.object({})));`,
        `import { registerMvuSchema } from '${MVU_ZOD}';\nawait import('${probe.url}/dyn-literal.js');\n$(() => registerMvuSchema(z.object({})));`,
        `import { registerMvuSchema as r } from 'tt:mvu_zod';\nr(z.object({}));`,
      ]) {
        const kind = await page.evaluate(async (text) => {
          const runner = window.__tt.createZodRunner({ schema: text });
          try {
            await runner.initialize({});
            return "ok";
          } catch (error) {
            return error.kind;
          } finally {
            runner.dispose();
          }
        }, schema);
        assert.equal(kind, "schema-load");
      }

      step("期限：transform 無窮迴圈與載入時無窮迴圈都在 5 秒期限作廢（deadline），下一次派工重建可用");
      const deadline = await page.evaluate(async (mvuZod) => {
        const looping = `import { registerMvuSchema } from '${mvuZod}';\n$(() => registerMvuSchema(z.object({ n: z.number().transform((v) => { if (v === 13) for (;;) {} return v; }).prefault(0) })));`;
        const runner = window.__tt.createZodRunner({ schema: looping });
        const out = {};
        await runner.initialize({});
        const started = performance.now();
        try {
          await runner.parse("_.set('n', 13);", { stat_data: { n: 0 } }, null);
          out.loop = "ok";
        } catch (error) {
          out.loop = error.kind;
        }
        out.ms = Math.round(performance.now() - started);
        out.after = (await runner.parse("_.set('n', 2);", { stat_data: { n: 0 } }, null)).table.stat_data.n;
        runner.dispose();
        const hung = window.__tt.createZodRunner({ schema: `import { registerMvuSchema } from '${mvuZod}';\nfor (;;) {}` });
        try {
          await hung.initialize({});
          out.load = "ok";
        } catch (error) {
          out.load = error.kind;
        }
        hung.dispose();
        return out;
      }, MVU_ZOD);
      console.log("  ", JSON.stringify(deadline));
      assert.equal(deadline.loop, "deadline");
      assert.ok(deadline.ms >= 4900 && deadline.ms < 7000, `期限 ${deadline.ms} ms`);
      assert.equal(deadline.after, 2);
      assert.equal(deadline.load, "deadline");

      step("上限：記憶體炸彈（transform 期、載入期）整次失敗、線性記憶體封頂 128 MB；堆疊、丟 null、Promise 無窮迴圈整次失敗；之後重建可用");
      const limits = await page.evaluate(async (mvuZod) => {
        const head = `import { registerMvuSchema } from '${mvuZod}';\n`;
        const transform = (body) => `${head}registerMvuSchema(z.object({ n: z.number().transform((v) => { if (v === 13) { ${body}; } return v; }).prefault(0) }));`;
        const atLoad = (body) => `${head}${body};\nregisterMvuSchema(z.object({ n: z.number().prefault(0) }));`;
        const bombs = {
          string: "const a = []; for (let i = 0; ; i++) a.push('x'.repeat(1 << 20) + i)",
          object: "const a = []; for (let i = 0; ; i++) a.push({ i, s: [i, i, i] })",
          arrayBuffer: "const a = []; for (;;) a.push(new ArrayBuffer(1 << 20))",
          typedArray: "const a = []; for (;;) a.push(new Float64Array(1 << 20))",
        };
        const megabytes = (message) => Number(/線性記憶體 (\d+) MB/.exec(message)?.[1] ?? NaN);
        const out = {};
        const run = async (label, schema, phase) => {
          const runner = window.__tt.createZodRunner({ schema });
          const started = performance.now();
          try {
            await runner.initialize({});
            if (phase !== "load") await runner.parse("_.set('n', 13);", { stat_data: { n: 0 } }, null);
            out[label] = { kind: "ok" };
          } catch (error) {
            out[label] = { kind: error.kind, mb: megabytes(error.message), ms: Math.round(performance.now() - started) };
          }
          // 同一支 runner 下一次派工重建可用（載入期炸彈的 schema 每次都炸，只驗 transform 期）
          if (phase !== "load") out[label].after = (await runner.parse("_.set('n', 2);", { stat_data: { n: 0 } }, null)).table.stat_data.n;
          runner.dispose();
        };
        for (const [name, body] of Object.entries(bombs)) {
          await run(`transform-${name}`, transform(body), "transform");
          await run(`load-${name}`, atLoad(body), "load");
        }
        await run("transform-recursion", transform("const f = (n) => f(n + 1) + 1; f(0)"), "transform");
        await run("transform-throwNull", transform("throw null"), "transform");
        await run("load-jobs", atLoad("const f = () => { Promise.resolve().then(f); }; f()"), "load");
        return out;
      }, MVU_ZOD);
      console.log("  ", JSON.stringify(limits));
      for (const name of ["string", "object", "arrayBuffer", "typedArray"]) {
        const transformed = limits[`transform-${name}`];
        assert.equal(transformed.kind, "memory", `transform-${name}`);
        assert.ok(transformed.mb <= 128, `transform-${name} 線性記憶體 ${transformed.mb} MB`);
        assert.equal(transformed.after, 2, `transform-${name} 之後重建`);
        // 載入期失敗都歸 schema-load（同桌面：載入卡片時的任何失敗）
        assert.equal(limits[`load-${name}`].kind, "schema-load", `load-${name}`);
        assert.ok(limits[`load-${name}`].mb <= 128, `load-${name} 線性記憶體 ${limits[`load-${name}`].mb} MB`);
      }
      assert.equal(limits["transform-recursion"].kind, "stack");
      assert.equal(limits["transform-recursion"].after, 2);
      assert.equal(limits["transform-throwNull"].kind, "memory");
      assert.equal(limits["transform-throwNull"].after, 2);
      assert.equal(limits["load-jobs"].kind, "deadline");

      step("炸彈進行中主頁期限先到：照樣 terminate 收掉（deadline），之後重建可用");
      const outer = await page.evaluate(async (mvuZod) => {
        // 慢慢配置（每 MB 之間忙 20 ms），主頁期限 1 秒先到、Worker 自己的 5 秒期限還沒到
        const schema = `import { registerMvuSchema } from '${mvuZod}';\nregisterMvuSchema(z.object({ n: z.number().transform((v) => { if (v === 13) { const a = []; for (;;) { const t = Date.now(); while (Date.now() - t < 20) {} a.push(new ArrayBuffer(1 << 20)); } } return v; }).prefault(0) }));`;
        const runner = window.__tt.createZodRunner({ schema, deadlineMs: 1000 });
        await runner.initialize({});
        const started = performance.now();
        const out = {};
        try {
          await runner.parse("_.set('n', 13);", { stat_data: { n: 0 } }, null);
          out.kind = "ok";
        } catch (error) {
          out.kind = error.kind;
        }
        out.ms = Math.round(performance.now() - started);
        out.after = (await runner.parse("_.set('n', 2);", { stat_data: { n: 0 } }, null)).table.stat_data.n;
        runner.dispose();
        return out;
      }, MVU_ZOD);
      console.log("  ", JSON.stringify(outer));
      assert.equal(outer.kind, "deadline");
      assert.ok(outer.ms < 2000, `主頁期限 ${outer.ms} ms`);
      assert.equal(outer.after, 2);

      step("開機訊息只收父頁與本站：外站直接開頁、嵌入（含沙盒與不沙盒）後送開機訊息，惡意引擎都不會跑");
      const attacker = await browser.newPage();
      const popupOpened = attacker.waitForEvent("popup");
      await attacker.goto(foreign.url);
      const popup = await popupOpened;
      await popup.waitForLoadState();
      // 直接打開的那一頁是不透明來源（CSP sandbox），開機碼在頂層直接不裝監聽
      assert.equal(await popup.evaluate(() => window.origin), "null");
      await attacker.waitForTimeout(4000);
      assert.deepEqual((await attacker.evaluate(() => window.attacked)).sort(), ["frame-allow-scripts", "frame-plain", "popup"]);
      assert.deepEqual(await attacker.evaluate(() => window.received), []);
      await attacker.close();

      step("存檔哨兵完好");
      const sentinel = await page.evaluate(
        () =>
          new Promise((resolve) => {
            const request = indexedDB.open("tt-web-sentinel");
            request.onsuccess = () => {
              const get = request.result.transaction("s").objectStore("s").get("k");
              get.onsuccess = () => resolve(get.result);
              get.onerror = () => resolve("read-error");
            };
            request.onerror = () => resolve("open-error");
          }),
      );
      assert.equal(sentinel, "SENTINEL");
      assert.deepEqual(probe.hits, [], `探測伺服器不該收到任何請求：${probe.hits.join(", ")}`);
    } finally {
      await browser.close();
    }
  }
  console.log("\n✓ zod 隔離驗收全部通過");
} finally {
  await server.close();
  await probe.close();
  await foreign.close();
}
