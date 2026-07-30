// 語系字典體檢：佔位符一致性＋按鈕文字寬度。改文案或加語系後跑 `npm run check:i18n`。
// 缺鍵不必在這裡管——src/i18n/index.ts 的型別會讓 tsc 直接編譯失敗。
import { build } from "esbuild";
import { readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const ROOT = new URL("..", import.meta.url).pathname;
const I18N = join(ROOT, "src/i18n");
const OUT = join(tmpdir(), `tt-i18n-${process.pid}`);

const files = readdirSync(I18N).filter((f) => f.endsWith(".ts") && f !== "index.ts");
await build({
  entryPoints: files.map((f) => join(I18N, f)),
  outdir: OUT,
  format: "esm",
  logLevel: "warning",
});

/** @type {Record<string, Record<string, string>>} */
const dicts = {};
for (const file of files) {
  const code = file.replace(/\.ts$/, "");
  const module = await import(join(OUT, `${code}.js`));
  dicts[code] = Object.values(module)[0];
}
rmSync(OUT, { recursive: true, force: true });

const canon = dicts["zh-TW"];
const en = dicts["en"];
const keys = Object.keys(canon);
const placeholders = (text) => (String(text).match(/\{[a-zA-Z]+\}/g) ?? []).sort().join(",");

// 按鈕與頁籤：只有真的放在窄容器裡的字才受寬度限制
const app = readFileSync(join(ROOT, "src/App.tsx"), "utf8");
const css = readFileSync(join(ROOT, "src/App.css"), "utf8");
const buttonKeys = new Set();
for (const match of app.matchAll(/<button\b[\s\S]*?<\/button>/g)) {
  const body = match[0].slice(match[0].indexOf(">") + 1);
  for (const hit of body.matchAll(/\bt\("([a-zA-Z0-9_]+)"/g)) buttonKeys.add(hit[1]);
}
for (const hit of app.matchAll(/\bt\("([a-zA-Z0-9_]*Tab)"/g)) buttonKeys.add(hit[1]);

// 中日韓字佔兩格；上限取中英兩版較寬者的 1.3 倍＋2，因為介面本來就容得下那兩版
const WIDE = /[ᄀ-ᅟ⺀-꓏가-힣豈-﫿︰-﹏＀-｠￠-￦]/;
const width = (text) => [...String(text)].reduce((n, c) => n + (WIDE.test(c) ? 2 : 1), 0);

// 語言本身沒有更短的地道說法，且所在列已有折行或充足寬度保護
const WRAP_SAFE_LONG = new Set([
  "de:editBtn",
  "de:hideActs",
  "fr:onboardSaveBtn",
  "ru:removeImageBtn",
  "ru:send",
  "ru:worldbookSaveEntry",
]);

// 寬度估算只守單顆文案；真正防溢出的版面契約也一併鎖住
const layoutContracts = [
  ["一般按鈕列可折行", /\.row\s*\{[^}]*flex-wrap:\s*wrap/s],
  ["桌面標題列操作可折行", /\.chat-header-actions\s*\{[^}]*flex-wrap:\s*wrap/s],
  ["輸入區操作可折行", /\.composer-send\s*\{[^}]*flex-wrap:\s*wrap/s],
  ["角色名按鈕有寬度上限", /\.request-reply\s*\{[^}]*max-width:/s],
  ["角色名過長時省略", /\.request-reply-label\s*\{[^}]*text-overflow:\s*ellipsis/s],
];
const missingLayoutContracts = layoutContracts
  .filter(([, pattern]) => !pattern.test(css))
  .map(([name]) => name);

let failed = false;
if (missingLayoutContracts.length) {
  failed = true;
  console.log(`FAIL layout: 缺少 ${missingLayoutContracts.join("、")}`);
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

if (failed) {
  console.log("\n若某顆按鈕沒有更短的地道說法，先確認所在按鈕列可折行，再加入 WRAP_SAFE_LONG。");
  process.exit(1);
}
