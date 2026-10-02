// 正式發版流程不得帶測試通道：release 工作流程與發版腳本裡出現 test-harness feature 或測試包設定就擋。
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const FILES = [
  ".github/workflows/release.yml",
  ...fs
    .readdirSync(path.join(ROOT, "scripts/release"))
    .filter((name) => name.endsWith(".mjs"))
    .map((name) => `scripts/release/${name}`),
];
const FORBIDDEN = ["test-harness", "tauri.harness.conf", "harness:build"];

const hits = [];
for (const file of FILES) {
  const text = fs.readFileSync(path.join(ROOT, file), "utf8");
  for (const word of FORBIDDEN) {
    if (text.includes(word)) hits.push(`${file}: ${word}`);
  }
}
if (hits.length > 0) {
  console.error(`發版流程不得帶測試通道：\n${hits.join("\n")}`);
  process.exit(1);
}
console.log(`harness 隔離檢查通過（${FILES.length} 支發版檔）`);
