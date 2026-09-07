// 目錄結構關卡：`npm run check:structure`，由 verify 呼叫。規範全文見 docs/STRUCTURE.md。
// 只擋機器判得準的四件事；「這支檔案該屬哪個 feature」是語意問題，交給 review，
// 不讓 checker 假裝判得出來。
//
// 這裡刻意沒有 allowlist／legacy baseline：source-structure 已經把 src 根層清空，
// 不存在需要豁免的舊檔。一旦開放例外清單，紅燈的預設解法會變成「把新檔加進清單」，
// 關卡就退化成橡皮圖章。要改頂層架構，就得連這個檔案一起改，讓架構變更在 review 裡藏不住。
import { readdirSync, statSync } from "node:fs";
import { join, relative, basename, extname } from "node:path";
import { fileURLToPath } from "node:url";

const SRC = fileURLToPath(new URL("../src/", import.meta.url));
const SOURCE_EXT = new Set([".ts", ".tsx", ".css"]);
// 根層只放應用程式入口
const ROOT_ALLOWED = new Set(["App.tsx", "App.css", "main.tsx", "vite-env.d.ts"]);
// 不知道放哪就丟進去的垃圾桶目錄，任意深度都禁
const DUMPING_GROUNDS = new Set(["utils", "helpers", "misc", "common"]);
// 過渡名：真正該做的是拆責任或想清楚命名，不是留一個「之後再說」的檔名
const PLACEHOLDER_NAME = /^(temp|tmp|new|old|copy|backup|utils?|helpers?|misc|common)\b|\d+$/i;

const problems = [];
const rel = (p) => relative(SRC, p).replaceAll("\\", "/");

function walk(dir, depth = 0) {
  for (const entry of readdirSync(dir).sort()) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      if (DUMPING_GROUNDS.has(entry.toLowerCase())) {
        problems.push(
          `${rel(full)}/ ：禁止 utils／helpers／misc／common 這類垃圾桶目錄。` +
            `不知道一支檔案該放哪，代表它的責任還沒想清楚，先判斷它真正的 owner。`,
        );
      }
      walk(full, depth + 1);
      continue;
    }
    if (!SOURCE_EXT.has(extname(entry))) continue;

    if (depth === 0 && !ROOT_ALLOWED.has(entry)) {
      problems.push(
        `${entry} ：src 根層只放應用程式入口（${[...ROOT_ALLOWED].join("、")}）。` +
          `新功能請放進 features/<name>/，跨 feature 共用的放 shared/<area>/。`,
      );
    }

    const stem = basename(entry).replace(/\.(test\.)?(tsx?|css)$/, "");
    if (PLACEHOLDER_NAME.test(stem)) {
      problems.push(`${rel(full)} ：檔名像過渡命名（${stem}）。請改成描述責任的名字。`);
    }

    if (entry.endsWith(".test.ts") || entry.endsWith(".test.tsx")) {
      const owner = rel(full).split("/");
      const inFeature = owner[0] === "features" && owner.length >= 3;
      const inShared = owner[0] === "shared" && owner.length >= 3;
      if (!inFeature && !inShared) {
        problems.push(
          `${rel(full)} ：測試要跟著它測的東西住，放進 features/<name>/ 或 shared/<area>/。`,
        );
      }
    }

    if (entry.endsWith(".tsx") && /^use[A-Z]/.test(entry)) {
      problems.push(`${rel(full)} ：hook 是純邏輯，副檔名用 .ts；.tsx 留給有 JSX 的元件。`);
    }
  }
}

walk(SRC);

if (problems.length) {
  console.error(`✗ 目錄結構有 ${problems.length} 處不合規範（見 docs/STRUCTURE.md）：\n`);
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}
console.log("✓ 目錄結構符合規範");
