#!/usr/bin/env node
// 測試通道 CLI：主線對測試包（npm run harness:build）送指令、印 JSON 結果。規格：.ai/plans/test-harness.md。
// 用法：node scripts/harness.mjs <指令> [參數]，全域選項 --root DIR（預設 $TMPDIR/tt-harness）。
import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO = fileURLToPath(new URL("..", import.meta.url));
const IDENTIFIER = "com.tabletavern.app.harness";
const DEFAULT_APP = path.join(
  REPO,
  "src-tauri/target/release/bundle/macos/Table Tavern Harness.app/Contents/MacOS/table-tavern",
);

function parseArgs(argv) {
  const positional = [];
  const flags = {};
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg.startsWith("--")) {
      const key = arg.slice(2);
      const next = argv[i + 1];
      if (next === undefined || next.startsWith("--")) flags[key] = true;
      else flags[key] = argv[++i];
    } else if (arg === "-f") {
      flags.file = argv[++i];
    } else {
      positional.push(arg);
    }
  }
  return { positional, flags };
}

function out(value) {
  console.log(JSON.stringify(value, null, 2));
}

function die(message, extra) {
  console.log(JSON.stringify({ ok: false, error: message, ...extra }, null, 2));
  process.exit(1);
}

/** 與 Rust 端同規則：先解析系統別名，不存在的路徑解析最近存在的祖先再接回。 */
function canonicalLenient(p) {
  const rest = [];
  let cur = p;
  for (;;) {
    try {
      return path.join(fs.realpathSync(cur), ...rest.reverse());
    } catch {
      const parent = path.dirname(cur);
      if (parent === cur) throw new Error(`找不到存在的祖先：${p}`);
      rest.push(path.basename(cur));
      cur = parent;
    }
  }
}

function validateRoot(raw) {
  if (!path.isAbsolute(raw)) die(`root 必須是絕對路徑：${raw}`);
  if (raw.split(path.sep).includes("..")) die(`root 不得含 ..：${raw}`);
  const root = canonicalLenient(raw);
  const home = canonicalLenient(os.homedir());
  const inside = (a, b) => a === b || a.startsWith(b + path.sep);
  if (inside(home, root)) die(`root 不得是家目錄或其祖先：${root}`);
  const support = path.join(os.homedir(), "Library/Application Support");
  const guarded = [
    path.join(os.homedir(), "Documents/TableTavern"),
    path.join(support, "TableTavern"),
    path.join(support, `${IDENTIFIER}.control`),
    path.join(support, IDENTIFIER),
    path.join(os.homedir(), "Library/WebKit", IDENTIFIER),
    path.join(os.homedir(), "Library/Caches", IDENTIFIER),
  ].map(canonicalLenient);
  for (const g of guarded) {
    if (inside(root, g) || inside(g, root)) die(`root ${root} 與受保護路徑 ${g} 重疊`);
  }
  return root;
}

function rootFrom(flags) {
  return validateRoot(flags.root ?? process.env.TT_HARNESS_ROOT ?? path.join(os.tmpdir(), "tt-harness"));
}

function readDiscovery(root) {
  try {
    return JSON.parse(fs.readFileSync(path.join(root, "harness.json"), "utf8"));
  } catch {
    return null;
  }
}

function alive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

async function call(root, method, route, body) {
  const info = readDiscovery(root);
  if (!info) die(`${root} 沒有 harness.json：測試實例沒在跑，先 launch`);
  let res;
  try {
    res = await fetch(`http://127.0.0.1:${info.port}${route}`, {
      method,
      headers: {
        Authorization: `Bearer ${info.token}`,
        ...(body === undefined ? {} : { "Content-Type": "application/json" }),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch (error) {
    die(`連不上控制埠：${error.message}`, { pid: info.pid, alive: alive(info.pid) });
  }
  return res.json();
}

function finish(result) {
  out(result);
  if (!result.ok) process.exit(1);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function launch(flags) {
  const root = rootFrom(flags);
  const app = flags.app ?? DEFAULT_APP;
  if (!fs.existsSync(app)) die(`找不到測試包執行檔：${app}（先 npm run harness:build）`);
  const existing = readDiscovery(root);
  if (existing && alive(existing.pid)) {
    die(`這個 root 已有測試實例（pid ${existing.pid}），先 status 或 quit`);
  }
  if (existing) fs.rmSync(path.join(root, "harness.json"), { force: true });
  const configFrom = flags["config-from"];
  if (configFrom && !fs.existsSync(configFrom)) die(`找不到 --config-from：${configFrom}`);
  const launchId = randomBytes(8).toString("hex");
  const logPath = path.join(os.tmpdir(), `tt-harness-${launchId}.log`);
  const log = fs.openSync(logPath, "a");
  const child = spawn(app, [], {
    detached: true,
    stdio: ["ignore", log, log],
    env: {
      ...process.env,
      TT_HARNESS_ROOT: root,
      TT_HARNESS_LAUNCH_ID: launchId,
      TT_HARNESS_FRESH: flags.fresh ? "1" : "",
      TT_HARNESS_CONFIG_FROM: configFrom ? path.resolve(configFrom) : "",
    },
  });
  let exited = null;
  child.on("exit", (code, signal) => (exited = { code, signal }));
  child.unref();
  const deadline = Date.now() + Number(flags.timeout ?? 60000);
  while (Date.now() < deadline) {
    if (exited) {
      die("測試實例啟動後就結束", { exit: exited, log: fs.readFileSync(logPath, "utf8").slice(-2000) });
    }
    const info = readDiscovery(root);
    if (info) {
      if (info.launchId !== launchId) {
        die("harness.json 不是本次啟動寫的（可能有別的實例）", { found: info.launchId, expected: launchId });
      }
      const status = await call(root, "GET", "/status");
      if (status.ok) return out({ ok: true, value: { ...status.value, port: info.port, log: logPath } });
    }
    await sleep(250);
  }
  die("等待測試實例就緒逾時", { log: logPath, pid: child.pid });
}

async function quit(root) {
  const info = readDiscovery(root);
  if (!info) die("沒有在跑的測試實例");
  const result = await call(root, "POST", "/quit");
  for (let i = 0; i < 80 && alive(info.pid); i++) await sleep(250);
  if (alive(info.pid)) die("送出 quit 後 20 秒內未退出", { pid: info.pid, response: result });
  finish({ ...result, exited: true });
}

async function dialogWait(root, flags) {
  const deadline = Date.now() + Number(flags.timeout ?? 15000);
  while (Date.now() < deadline) {
    const result = await call(root, "GET", "/dialogs");
    if (!result.ok) return finish(result);
    if (result.value.length > 0) return finish({ ok: true, value: result.value });
    await sleep(200);
  }
  die("等待對話窗逾時");
}

async function shot(root, file) {
  if (!file) die("用法：shot <out.png>");
  // app 內以 WKWebView takeSnapshot 拍 webview 內容，不需螢幕錄製授權（screencapture 需要）。
  const result = await call(root, "GET", "/shot");
  if (!result.ok) return finish(result);
  const target = path.resolve(file);
  const png = Buffer.from(result.value.pngBase64, "base64");
  fs.writeFileSync(target, png);
  out({ ok: true, value: { path: target, bytes: png.length } });
}

const MIME = {
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".webp": "image/webp",
  ".json": "application/json",
};

// 與 src-tauri/src/harness/server.rs 的 MAX_BODY 同值：超過就在本地擋下，不送出。
const MAX_BODY = 32 * 1024 * 1024;
const MAX_FILE = 24 * 1024 * 1024;

async function fileInput(root, target, filePath) {
  if (!target || !filePath) die("用法：file <target> <path>");
  const source = path.resolve(filePath);
  const stat = fs.statSync(source, { throwIfNoEntry: false });
  if (!stat?.isFile()) die(`找不到檔案：${source}`);
  if (stat.size > MAX_FILE) die(`檔案 ${stat.size} bytes 超過上限 ${MAX_FILE}（base64 後會超過控制埠 body 上限）`);
  const type = MIME[path.extname(source).toLowerCase()] ?? "application/octet-stream";
  const base64 = fs.readFileSync(source).toString("base64");
  const js = `return await H.file(${JSON.stringify(target)}, ${JSON.stringify(path.basename(source))}, ${JSON.stringify(type)}, ${JSON.stringify(base64)})`;
  const length = Buffer.byteLength(JSON.stringify({ js }));
  if (length > MAX_BODY) die(`請求 body ${length} bytes 超過控制埠上限 ${MAX_BODY}，未送出`);
  finish(await call(root, "POST", "/eval", { js }));
}

/** 呼叫頁面端 H 的一個方法；參數一律 JSON 字串化，target 不會被當成程式碼。 */
async function helper(root, method, params, timeoutMs) {
  const js = `return await H.${method}(${params.map((p) => JSON.stringify(p)).join(", ")})`;
  finish(await call(root, "POST", "/eval", { js, timeoutMs }));
}

function parseJsonArg(text, label) {
  if (text === undefined) return undefined;
  try {
    return JSON.parse(text);
  } catch (error) {
    die(`${label} 不是 JSON：${error.message}`);
  }
}

const USAGE = `用法：node scripts/harness.mjs <指令> [--root DIR]
  launch [--fresh] [--config-from FILE] [--app PATH]
  status | quit
  eval|js '<js>' | eval|js -f file.js  [--timeout ms]
  dialogs | dialog-wait [--timeout ms] | answer <id|next> <按鈕標籤|ok|cancel|路徑>
  file <target> <path>
  text [target] | query <target> | click <target> | submit <target> | fill <target> <value>
  select <target> <value> | press <target> <key> | wait <target> [--gone] [--timeout ms]
  invoke <command> [json] | route [worldId] | ai-log
  openrouter-origin <http://127.0.0.1:PORT/api/v1 | clear>（智慧免費改打本機假端點，見 scripts/harness-fake-openrouter.mjs）
  shot <out.png>
target：CSS selector、text=完整文字、text*=部分文字、role=button[name="名稱"]、role=button[name*="部分"]`;

const { positional, flags } = parseArgs(process.argv.slice(2));
const [command, ...args] = positional;
switch (command) {
  case "launch":
    await launch(flags);
    break;
  case "status":
    finish(await call(rootFrom(flags), "GET", "/status"));
    break;
  case "quit":
    await quit(rootFrom(flags));
    break;
  // `js` 是別名：部分 shell 包裝會把字面上的 eval 當危險指令擋下。
  case "js":
  case "eval": {
    const js = flags.file ? fs.readFileSync(flags.file, "utf8") : args.join(" ");
    if (!js) die("用法：eval '<js>'");
    const timeoutMs = flags.timeout ? Number(flags.timeout) : undefined;
    finish(await call(rootFrom(flags), "POST", "/eval", { js, timeoutMs }));
    break;
  }
  case "dialogs":
    finish(await call(rootFrom(flags), "GET", "/dialogs"));
    break;
  case "dialog-wait":
    await dialogWait(rootFrom(flags), flags);
    break;
  case "answer":
    if (args.length < 2) die("用法：answer <id|next> <choice>");
    finish(await call(rootFrom(flags), "POST", "/answer", { id: args[0], choice: args.slice(1).join(" ") }));
    break;
  case "shot":
    await shot(rootFrom(flags), args[0]);
    break;
  case "file":
    await fileInput(rootFrom(flags), args[0], args[1]);
    break;
  case "text":
    await helper(rootFrom(flags), "text", args.slice(0, 1));
    break;
  case "query":
  case "click":
  case "submit":
    if (!args[0]) die(`用法：${command} <target>`);
    await helper(rootFrom(flags), command, [args[0]]);
    break;
  case "fill":
  case "select":
  case "press":
    if (args.length < 2) die(`用法：${command} <target> <value>`);
    await helper(rootFrom(flags), command, [args[0], args.slice(1).join(" ")]);
    break;
  case "wait": {
    if (!args[0]) die("用法：wait <target> [--gone] [--timeout ms]");
    const timeout = Number(flags.timeout ?? 10000);
    await helper(rootFrom(flags), "wait", [args[0], { gone: Boolean(flags.gone), timeout }], timeout + 5000);
    break;
  }
  case "invoke":
    if (!args[0]) die("用法：invoke <command> [json]");
    await helper(rootFrom(flags), "invoke", [args[0], parseJsonArg(args[1], "invoke 參數") ?? {}]);
    break;
  case "route":
    finish(await call(rootFrom(flags), "POST", "/route", args[0] ? { worldId: args[0] } : {}));
    break;
  case "ai-log":
    finish(await call(rootFrom(flags), "GET", "/ai-log"));
    break;
  case "openrouter-origin":
    if (!args[0]) die("用法：openrouter-origin <http://127.0.0.1:PORT/api/v1 | clear>");
    finish(
      await call(rootFrom(flags), "POST", "/openrouter-origin", {
        origin: args[0] === "clear" ? null : args[0],
      }),
    );
    break;
  default:
    console.log(USAGE);
    process.exit(command ? 1 : 0);
}
