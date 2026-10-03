// 語系字典體檢：佔位符一致性＋按鈕文字寬度＋後端訊息代碼對照。改文案或加語系後跑 `npm run check:i18n`。
// 主字典缺鍵不必在這裡管——src/i18n/index.ts 的型別會讓 tsc 直接編譯失敗。
// features/ 補充字典併進同一個鍵空間一起查；每份都要匯出 `*_MESSAGE_KEYS` 與 `*Message(lang, key)`。
import { build } from "esbuild";
import { readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

// 不能用 URL.pathname：Windows 上會是 /D:/... 開頭，join 之後變成 D:\D:\...
const ROOT = fileURLToPath(new URL("..", import.meta.url));
const I18N = join(ROOT, "src/i18n");
const OUT = join(tmpdir(), `tt-i18n-${process.pid}`);

// 語系字典的檔名就是語系代碼（en.ts、pt-BR.ts…）；plural.ts 等工具檔不算
const files = readdirSync(I18N).filter((f) => /^[a-z]{2}(-[A-Z]{2})?\.ts$/.test(f));
const featureFiles = readdirSync(join(I18N, "features")).filter(
  (f) => f.endsWith(".ts") && !f.endsWith(".test.ts"),
);
await build({
  entryPoints: [join(I18N, "plural.ts"), ...files.map((f) => join(I18N, f)), ...featureFiles.map((f) => join(I18N, "features", f))],
  outdir: OUT,
  outbase: I18N,
  // 打包：補充字典可能 import 共用 JSON（例如 shared/contracts/transcript-marker.json），暫存目錄裡找不到相對路徑
  bundle: true,
  format: "esm",
  logLevel: "warning",
});

// 動態 import 要餵 file:// URL：Windows 的 C:\... 不是合法的 ESM specifier。
const load = (rel) => import(pathToFileURL(join(OUT, rel.replace(/\.ts$/, ".js"))).href);

/** @type {Record<string, Record<string, string>>} */
const dicts = {};
for (const file of files) {
  dicts[file.replace(/\.ts$/, "")] = Object.values(await load(file))[0];
}
const { messagePlaceholders, widestRendering } = await load("plural.ts");
const featureModules = {};
for (const file of featureFiles) featureModules[file] = await load(`features/${file}`);
rmSync(OUT, { recursive: true, force: true });

let failed = false;
const fail = (message) => {
  failed = true;
  console.log(`FAIL ${message}`);
};

const langs = Object.keys(dicts);
for (const [file, module] of Object.entries(featureModules)) {
  const keyLists = Object.entries(module).filter(([name, value]) => name.endsWith("_MESSAGE_KEYS") && Array.isArray(value));
  const messages = Object.entries(module).filter(([name, value]) => name.endsWith("Message") && typeof value === "function");
  if (keyLists.length !== 1 || messages.length !== 1) {
    fail(`features/${file}: 要剛好匯出一個 *_MESSAGE_KEYS 與一個 *Message(lang, key)`);
    continue;
  }
  const [[, keys]] = keyLists;
  const [[, message]] = messages;
  for (const key of keys) {
    if (key in dicts["zh-TW"]) fail(`features/${file}: ${key} 與主字典重複`);
    for (const lang of langs) {
      const text = message(lang, key);
      if (typeof text !== "string") fail(`features/${file}: ${lang} 缺 ${key}`);
      dicts[lang][key] = text;
    }
  }
}

// ── 後端訊息代碼：從 Rust 抽出實際送到前端的 serde 值，跟字典逐一對上。抽不到就擋，不靜默放行。

/** 抓 `attr` 正下方 `pub enum name { ... }` 的內容；本體裡出現任何屬性或 tuple 都當不認得。 */
function rustEnumBody(file, attr, name) {
  // 先拿掉註解：doc comment 裡的括號不能算進列舉本體。
  const source = readFileSync(join(ROOT, file), "utf8").replace(/\/\/.*$/gm, "");
  const head = source.match(new RegExp(`${attr.replace(/[[\]()]/g, "\\$&")}\\s*pub enum ${name} \\{`));
  if (!head) throw new Error(`${file} 找不到 ${attr} pub enum ${name}`);
  let depth = 1;
  let i = head.index + head[0].length;
  const start = i;
  for (; i < source.length && depth > 0; i += 1) {
    if (source[i] === "{") depth += 1;
    if (source[i] === "}") depth -= 1;
  }
  if (depth !== 0) throw new Error(`${file} 的 ${name} 括號不成對`);
  const body = source.slice(start, i - 1);
  if (/#\[|\(/.test(body)) throw new Error(`${file} 的 ${name} 有屬性或 tuple 變體，抽取規則不認得`);
  return body;
}

/** serde rename_all = "snake_case" 對變體名的轉法：每個大寫字母（首字除外）前加底線再轉小寫。 */
const serdeSnake = (name) => name.replace(/[A-Z]/g, (c, i) => (i > 0 ? "_" : "") + c.toLowerCase());

const RUST_TYPES = {
  String: "string",
  bool: "boolean",
  ...Object.fromEntries(
    ["u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize", "f32", "f64"].map((t) => [t, "number"]),
  ),
};

/** @returns {Map<string, Record<string, string>>} code → {欄位: 型別} */
function rustVariants(file, attr, name) {
  const body = rustEnumBody(file, attr, name);
  const variants = new Map();
  const pattern = /\s*([A-Z]\w*)\s*(?:\{([^{}]*)\})?\s*(?:,|$)/y;
  let rest = body.trim();
  while (rest) {
    pattern.lastIndex = 0;
    const hit = pattern.exec(rest);
    if (!hit) throw new Error(`${file} 的 ${name} 看不懂這段：${rest.slice(0, 40)}`);
    const fields = {};
    for (const raw of (hit[2] ?? "").split(",").map((f) => f.trim()).filter(Boolean)) {
      const field = raw.match(/^(?:pub\s+)?(\w+)\s*:\s*(\w+)$/);
      if (!field || !RUST_TYPES[field[2]]) throw new Error(`${file} 的 ${name}::${hit[1]} 欄位不認得：${raw}`);
      fields[field[1]] = RUST_TYPES[field[2]];
    }
    variants.set(serdeSnake(hit[1]), fields);
    rest = rest.slice(hit[0].length).trim();
  }
  if (variants.size === 0) throw new Error(`${file} 的 ${name} 抽不到任何變體`);
  return variants;
}

function checkBackendCodes() {
  const canon = dicts["zh-TW"];
  const backend = featureModules["backend-msg.ts"];
  let codes;
  let reasons;
  try {
    codes = rustVariants("src-tauri/src/ui_msg.rs", '#[serde(tag = "code", rename_all = "snake_case")]', "UiMsg");
    reasons = rustVariants("src-tauri/src/data/format/commit.rs", '#[serde(rename_all = "snake_case")]', "RepairReason");
  } catch (error) {
    fail(`backend codes: ${error.message}`);
    return;
  }
  if (!backend) {
    fail("backend codes: 缺 src/i18n/features/backend-msg.ts");
    return;
  }
  const problems = [];
  const sameSet = (a, b) => a.length === b.length && a.every((x) => b.includes(x));

  const beKeys = Object.keys(canon).filter((k) => k.startsWith("be_")).map((k) => k.slice(3));
  for (const code of codes.keys()) if (!beKeys.includes(code)) problems.push(`缺 be_${code}`);
  for (const code of beKeys) if (!codes.has(code)) problems.push(`be_${code} 在 ui_msg.rs 沒有對應變體`);
  const params = backend.BACKEND_MSG_PARAMS ?? {};
  for (const [code, fields] of codes) {
    if (JSON.stringify(Object.entries(params[code] ?? {}).sort()) !== JSON.stringify(Object.entries(fields).sort())) {
      problems.push(`BACKEND_MSG_PARAMS.${code} 應為 ${JSON.stringify(fields)}`);
    }
    for (const lang of langs) {
      const used = [...new Set(placeholderNames(dicts[lang][`be_${code}`]) ?? [])];
      if (!sameSet(used, Object.keys(fields))) {
        problems.push(`${lang} be_${code} 佔位符 {${used.join("},{")}} 應為 Rust 欄位 {${Object.keys(fields).join("},{")}}`);
      }
    }
  }
  for (const code of Object.keys(params)) if (!codes.has(code)) problems.push(`BACKEND_MSG_PARAMS.${code} 在 ui_msg.rs 沒有對應變體`);

  const repairKeys = Object.keys(canon).filter((k) => k.startsWith("needsRepair_")).map((k) => k.slice("needsRepair_".length));
  const rustReasons = [...reasons.keys()];
  if (!sameSet(repairKeys, rustReasons)) problems.push(`needsRepair_* 應涵蓋 ${rustReasons.join("、")}，實為 ${repairKeys.join("、")}`);
  if (!sameSet([...(backend.NEEDS_REPAIR_REASONS ?? [])], rustReasons)) problems.push("NEEDS_REPAIR_REASONS 與 RepairReason 不符");
  for (const reason of repairKeys) {
    const expected = reason === "io" ? ["error"] : [];
    if (!sameSet([...new Set(placeholderNames(canon[`needsRepair_${reason}`]) ?? [])], expected)) {
      problems.push(`needsRepair_${reason} 佔位符應為 ${expected.map((n) => `{${n}}`).join("") || "無"}`);
    }
  }

  if (problems.length) {
    fail(`backend codes: ${problems.length} 處不符`);
    for (const problem of problems) console.log(`  ${problem}`);
  } else {
    console.log(`OK   backend codes: ${codes.size} 個 UiMsg、${reasons.size} 個 RepairReason 與字典一致`);
  }
}

const canon = dicts["zh-TW"];
const en = dicts["en"];
const keys = Object.keys(canon);
// 佔位符照 plural.ts 的文法算（plural 區塊算參數一次＋分支佔位符一次）；語法錯另外報
const placeholderNames = (text) => messagePlaceholders(String(text)).names;
const placeholders = (text) => {
  const shape = messagePlaceholders(String(text));
  return "error" in shape ? `語法錯：${shape.error}` : shape.names.map((n) => `{${n}}`).join(",");
};

// 按鈕與頁籤：只有真的放在窄容器裡的字才受寬度限制。
// 掃 src 下所有 .tsx——元件搬出 App.tsx 後按鈕仍在掃描範圍內，數字下降即代表漏掃。
const tsxFiles = readdirSync(join(ROOT, "src"), { recursive: true, withFileTypes: true })
  .filter((entry) => entry.isFile() && entry.name.endsWith(".tsx"))
  .map((entry) => join(entry.parentPath, entry.name));
const appCss = readFileSync(join(ROOT, "src/App.css"), "utf8");
const css = [
  appCss,
  ...[...appCss.matchAll(/@import\s+["'](.+?\.css)["'];/g)].map(([, path]) =>
    readFileSync(join(ROOT, "src", path), "utf8"),
  ),
].join("\n");
const buttonKeys = new Set();
for (const file of tsxFiles) {
  const source = readFileSync(file, "utf8");
  for (const match of source.matchAll(/<button\b[\s\S]*?<\/button>/g)) {
    const body = match[0].slice(match[0].indexOf(">") + 1);
    for (const hit of body.matchAll(/\bt\("([a-zA-Z0-9_]+)"/g)) buttonKeys.add(hit[1]);
  }
  for (const hit of source.matchAll(/\bt\("([a-zA-Z0-9_]*Tab)"/g)) buttonKeys.add(hit[1]);
}

// 中日韓字佔兩格；上限取中英兩版較寬者的 1.3 倍＋2，因為介面本來就容得下那兩版
const WIDE = /[ᄀ-ᅟ⺀-꓏가-힣豈-﫿︰-﹏＀-｠￠-￦]/;
const charWidth = (text) => [...String(text)].reduce((n, c) => n + (WIDE.test(c) ? 2 : 1), 0);
// plural 字串照最寬的分支量，ICU 語法本身不算寬度
const width = (text) => charWidth(widestRendering(String(text), charWidth));

// 語言本身沒有更短的地道說法，且所在列已有折行或充足寬度保護
const WRAP_SAFE_LONG = new Set([
  // 這不是按鈕文案，是沒有玩家卡時的代稱（狀態欄的值欄是 1fr 彈性欄，本來就放得下整句狀態）
  "ja:playerLabel",
  "de:editBtn",
  "de:hideActs",
  "ru:removeImageBtn",
  "ru:send",
  "ru:worldbookSaveEntry",
  // features/ 字典併入檢查時既有的長字：所在列 .row／.smart-free-new-actions 都可折行
  "de:onboardManualSaving",
  "fr:onboardManualSaving",
  "de:smartFreeNewUse",
  "es:smartFreeNewUse",
  "fr:smartFreeNewUse",
  "ru:smartFreeNewUse",
  // 編輯頁頂列的主鈕：沒有更短的地道說法；主鈕不縮，空間由頂列標題先讓出
  "de:saveBtn",
  "fr:saveBtn",
  "ru:saveBtn",
]);

// 寬度估算只守單顆文案；真正防溢出的版面契約也一併鎖住
// 牌桌工具列與輸入動作列刻意不換行（800×600 最小設計尺寸），靠縮寬＋省略號防溢出
const layoutContracts = [
  ["一般按鈕列可折行", /\.row\s*\{[^}]*flex-wrap:\s*wrap/s],
  ["按鈕標籤過長時省略", /\.btn-label\s*\{[^}]*text-overflow:\s*ellipsis/s],
  ["桌名過長時省略", /\.table-title-text\s*\{[^}]*text-overflow:\s*ellipsis/s],
  ["工具列有字鈕可縮", /\.table-toolbar\s*>\s*\.btn-shrink\s*\{[^}]*flex:\s*0 1 auto/s],
  ["相連按鈕組可縮", /\.btn-seg\s*>\s*\.btn\s*\{[^}]*min-width:\s*0/s],
  ["編輯頁頂列有字鈕可縮", /\.edit-page-bar\s*>\s*\.btn-shrink[^{]*\{[^}]*flex:\s*0 100 auto/s],
  ["編輯頁主鈕不縮", /\.edit-page-bar\s*>\s*\.btn-primary\.btn-shrink[^{]*\{[^}]*flex-shrink:\s*0/s],
];
const missingLayoutContracts = layoutContracts
  .filter(([, pattern]) => !pattern.test(css))
  .map(([name]) => name);

if (missingLayoutContracts.length) fail(`layout: 缺少 ${missingLayoutContracts.join("、")}`);
for (const code of Object.keys(dicts).sort()) {
  const broken = keys.filter((k) => "error" in messagePlaceholders(String(dicts[code][k])));
  if (broken.length) {
    fail(`${code}: ${broken.length} 個字串語法錯`);
    for (const k of broken) console.log(`  ${k}: ${messagePlaceholders(String(dicts[code][k])).error}`);
  }
}
for (const code of Object.keys(dicts).sort()) {
  if (code === "zh-TW") continue;
  const dict = dicts[code];
  const badPlaceholders = keys.filter((k) => placeholders(canon[k]) !== placeholders(dict[k]));
  const tooWide = [...buttonKeys]
    .filter((k) => k in canon && !WRAP_SAFE_LONG.has(`${code}:${k}`))
    .map((k) => ({ k, w: width(dict[k]), budget: Math.ceil(Math.max(width(canon[k]), width(en[k])) * 1.3) + 2 }))
    .filter((x) => x.w > x.budget);

  if (badPlaceholders.length || tooWide.length) {
    failed = true;
    console.log(`FAIL ${code}: 佔位符不符 ${badPlaceholders.length}、按鈕過寬 ${tooWide.length}`);
    for (const k of badPlaceholders) console.log(`  佔位符 ${k}: 應為 ${placeholders(canon[k]) || "無"}，實為 ${placeholders(dict[k]) || "無"}`);
    for (const x of tooWide) console.log(`  過寬 ${x.k}: 寬 ${x.w} > 上限 ${x.budget} — "${dict[x.k]}"`);
  } else {
    console.log(`OK   ${code}: 佔位符一致、${buttonKeys.size} 顆按鈕都在寬度上限內`);
  }
}

checkBackendCodes();

if (failed) {
  console.log("\n若某顆按鈕沒有更短的地道說法，先確認所在按鈕列可折行，再加入 WRAP_SAFE_LONG。");
  process.exit(1);
}

