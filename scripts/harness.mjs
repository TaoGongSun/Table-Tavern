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

const USAGE = `用法：node scripts/harness.mjs <指令> [--root DIR]
  launch [--fresh] [--config-from FILE] [--app PATH]
  status | quit
  eval|js '<js>' | eval|js -f file.js  [--timeout ms]
  dialogs | dialog-wait [--timeout ms] | answer <id|next> <按鈕標籤|ok|cancel|路徑>
  shot <out.png>`;

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
  default:
    console.log(USAGE);
    process.exit(command ? 1 : 0);
}
