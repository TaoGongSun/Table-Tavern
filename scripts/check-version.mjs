// 比對三處版本：package.json、Cargo.toml 的 [package].version、tauri.conf.json。
// 給了 tag 還要等於 v<版本>。沒給 tag 的模式由 `npm run verify` 呼叫，不需要先 build。
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = fileURLToPath(new URL("..", import.meta.url));

/** 只取 [package] 表裡的 version，不看依賴或其他表。 */
export function cargoPackageVersion(toml) {
  const match = toml.match(
    /(?:^|\n)\[package\][^\[]*?\nversion\s*=\s*["']([^"']+)["']/,
  );
  if (!match) return null;
  return match[1];
}

function readTextVersion(file, label, parse) {
  try {
    const value = parse(readFileSync(file, "utf8"));
    if (typeof value !== "string" || value === "") {
      return { value: null, error: `${label} 沒有 version 字串` };
    }
    return { value, error: null };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { value: null, error: `${label} 讀不到：${message}` };
  }
}

export function readVersions(root = ROOT) {
  const packageJson = readTextVersion(
    resolve(root, "package.json"),
    "package.json",
    (text) => JSON.parse(text).version,
  );
  const tauri = readTextVersion(
    resolve(root, "src-tauri/tauri.conf.json"),
    "tauri.conf.json",
    (text) => JSON.parse(text).version,
  );
  const cargo = readTextVersion(
    resolve(root, "src-tauri/Cargo.toml"),
    "Cargo.toml [package].version",
    cargoPackageVersion,
  );
  return {
    packageJson: packageJson.value,
    cargo: cargo.value,
    tauri: tauri.value,
    errors: [packageJson.error, cargo.error, tauri.error].filter(Boolean),
  };
}

/**
 * @param {{ packageJson: string|null, cargo: string|null, tauri: string|null }} values
 * @param {string|undefined} tag 有給才檢查等於 v<版本>
 */
export function evaluateVersions(values, tag) {
  const lines = [
    `package.json: ${values.packageJson ?? "（沒有）"}`,
    `Cargo.toml [package].version: ${values.cargo ?? "（沒有）"}`,
    `tauri.conf.json: ${values.tauri ?? "（沒有）"}`,
  ];
  const problems = [];
  const same =
    values.packageJson &&
    values.packageJson === values.cargo &&
    values.packageJson === values.tauri;
  if (!same) problems.push("三處版本要相同");
  const version = same ? values.packageJson : null;
  if (tag !== undefined) {
    lines.push(`tag: ${tag}`);
    const expected = version ? `v${version}` : null;
    if (!expected || tag !== expected) {
      problems.push(expected ? `tag 應為 ${expected}` : "tag 無法與版本對上");
    }
  }
  return { ok: problems.length === 0, version, lines, problems };
}

function isDirectRun() {
  const entry = process.argv[1];
  if (!entry) return false;
  return import.meta.url === pathToFileURL(resolve(entry)).href;
}

function main() {
  const args = process.argv.slice(2);
  if (args.length > 1) {
    console.error("用法：node scripts/check-version.mjs [tag]");
    process.exit(1);
  }
  const tag = args[0];
  const read = readVersions(ROOT);
  const result = evaluateVersions(read, tag);
  if (read.errors.length || !result.ok) {
    console.error("版本不一致：");
    for (const line of result.lines) console.error(`  ${line}`);
    for (const error of read.errors) console.error(`  - ${error}`);
    for (const problem of result.problems) console.error(`  - ${problem}`);
    process.exit(1);
  }
  const suffix = tag ? `（tag ${tag}）` : "";
  console.log(`✓ 版本一致：${result.version}${suffix}`);
}

if (isDirectRun()) main();
