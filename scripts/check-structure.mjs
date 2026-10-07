// 目錄結構關卡：`npm run check:structure`，由 verify 呼叫。規範全文見 docs/STRUCTURE.md。
// 只擋機器判得準的五件事；「這支檔案該屬哪個 feature」是語意問題，交給 review，
// 不讓 checker 假裝判得出來。
//
// 清單只列架構上的固定位置（根層入口、組合外殼），禁止逐檔豁免違規：一旦清單變成例外收容所，
// 紅燈的預設解法就會是「把新檔加進清單」，關卡退化成橡皮圖章。要改就得改這個檔，
// 讓架構變更在 review 裡藏不住。
import { readdirSync, statSync } from "node:fs";
import { join, relative, basename, extname } from "node:path";
import { fileURLToPath } from "node:url";

// 兩棵原始碼樹各自套同一套規則：桌面版 src/ 與網頁版 web/src/。根層入口、組合外殼清單分開列。
const TREES = [
  {
    label: "src",
    dir: fileURLToPath(new URL("../src/", import.meta.url)),
    // 根層只放應用程式入口
    rootAllowed: new Set(["App.tsx", "App.css", "main.tsx", "vite-env.d.ts"]),
    // views／controllers 只放把多個 feature 組起來的外殼（相對 src/ 的完整路徑，涵蓋任意深度）
    shellAllowed: new Set([
      "views/AppDialogs.tsx",
      "views/AppWorkspace.tsx",
      "views/MainView.tsx",
      "views/TableToolbar.tsx",
      "controllers/useWorkspaceNavigationController.ts",
    ]),
  },
  {
    label: "web/src",
    dir: fileURLToPath(new URL("../web/src/", import.meta.url)),
    rootAllowed: new Set(["App.tsx", "App.css", "main.tsx", "vite-env.d.ts"]),
    shellAllowed: new Set(),
  },
];
const SOURCE_EXT = new Set([".ts", ".tsx", ".css"]);
// 不知道放哪就丟進去的垃圾桶目錄，任意深度都禁
const DUMPING_GROUNDS = new Set(["utils", "helpers", "misc", "common"]);
// 過渡名：真正該做的是拆責任或想清楚命名，不是留一個「之後再說」的檔名
const PLACEHOLDER_NAME = /^(temp|tmp|new|old|copy|backup|utils?|helpers?|misc|common)\b|\d+$/i;

const problems = [];

function walk(tree, dir = tree.dir, depth = 0) {
  // 訊息裡的路徑：桌面版維持相對 src/（沿用既有訊息），網頁版前面帶 web/src/ 好分辨
  const rel = (p) => relative(tree.dir, p).replaceAll("\\", "/");
  const at = (p) => (tree.label === "src" ? rel(p) : `${tree.label}/${rel(p)}`);
  for (const entry of readdirSync(dir).sort()) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      if (DUMPING_GROUNDS.has(entry.toLowerCase())) {
        problems.push(
          `${at(full)}/ ：禁止 utils／helpers／misc／common 這類垃圾桶目錄。` +
            `不知道一支檔案該放哪，代表它的責任還沒想清楚，先判斷它真正的 owner。`,
        );
      }
      walk(tree, full, depth + 1);
      continue;
    }
    if (!SOURCE_EXT.has(extname(entry))) continue;

    if (depth === 0 && !tree.rootAllowed.has(entry)) {
      problems.push(
        `${at(full)} ：${tree.label} 根層只放應用程式入口（${[...tree.rootAllowed].join("、")}）。` +
          `新功能請放進 features/<name>/，跨 feature 共用的放 shared/<area>/。`,
      );
    }

    const path = rel(full);
    if (/^(views|controllers)\//.test(path) && !tree.shellAllowed.has(path)) {
      problems.push(
        `${at(full)} ：views／controllers 只放跨 feature 的組合外殼，單一功能的檔案放 features/<name>/；` +
          `新增外殼要改 scripts/check-structure.mjs 的 shellAllowed 清單。`,
      );
    }

    const stem = basename(entry).replace(/\.(test\.)?(tsx?|css)$/, "");
    if (PLACEHOLDER_NAME.test(stem)) {
      problems.push(`${at(full)} ：檔名像過渡命名（${stem}）。請改成描述責任的名字。`);
    }

    if (entry.endsWith(".test.ts") || entry.endsWith(".test.tsx")) {
      const owner = path.split("/");
      const inFeature = owner[0] === "features" && owner.length >= 3;
      const inShared = owner[0] === "shared" && owner.length >= 3;
      if (!inFeature && !inShared) {
        problems.push(
          `${at(full)} ：測試要跟著它測的東西住，放進 features/<name>/ 或 shared/<area>/。`,
        );
      }
    }

    if (entry.endsWith(".tsx") && /^use[A-Z]/.test(entry)) {
      problems.push(`${at(full)} ：hook 是純邏輯，副檔名用 .ts；.tsx 留給有 JSX 的元件。`);
    }
  }
}

for (const tree of TREES) walk(tree);

if (problems.length) {
  console.error(`✗ 目錄結構有 ${problems.length} 處不合規範（見 docs/STRUCTURE.md）：\n`);
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}
console.log("✓ 目錄結構符合規範");
