// 收 Windows／macOS 兩包產物：改無空格檔名、組 latest.json、用 minisign 驗簽。
// 正式模式才呼叫 gh 建草稿再公開。演練模式只把檔案放進 --out。
//
//   node scripts/release/finalize.mjs \
//     --mode rehearsal|release \
//     --windows <dir> --macos <dir> --out <dir> \
//     [--tag v0.2.0] [--skip-verify] [--pub-date <RFC3339>]
//
// --out 會先清空。--skip-verify 只給演練：本機沒有 minisign 時用，正式模式拒絕。
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { evaluateVersions, readVersions } from "../check-version.mjs";
import {
  buildLatestJson,
  decodeMinisignArmor,
  extractChangelogNotes,
  githubReleasePlan,
  planReleaseFiles,
  readUpdaterConfig,
  resolveNotes,
} from "./plan.mjs";

const ROOT = fileURLToPath(new URL("../..", import.meta.url));
const RFC3339 =
  /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/;

function walk(dir) {
  const found = [];
  const visit = (current, prefix) => {
    for (const entry of readdirSync(current, { withFileTypes: true }).sort()) {
      const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory()) visit(join(current, entry.name), rel);
      else found.push(rel);
    }
  };
  visit(dir, "");
  return found;
}

function assertSafeOut(outDir, inputs) {
  const out = resolve(outDir);
  for (const input of inputs) {
    const dir = resolve(input);
    const nested = out === dir || out.startsWith(dir + sep) || dir.startsWith(out + sep);
    if (nested) throw new Error("--out 不能和輸入目錄重疊");
  }
}

function runGh(args) {
  const run = spawnSync("gh", args, { encoding: "utf8" });
  if (run.error) {
    throw new Error(`gh 起不來：${run.error.message}`);
  }
  return run;
}

function mustGh(args) {
  const run = runGh(args);
  if (run.status !== 0) {
    const detail = (run.stderr || run.stdout || "").trim();
    throw new Error(`gh 失敗（exit ${run.status}）：\n${detail}`);
  }
  return run.stdout;
}

function lookupRelease(tag, slug) {
  const run = runGh([
    "release",
    "view",
    tag,
    "--repo",
    slug,
    "--json",
    "isDraft,tagName",
  ]);
  if (run.status !== 0) {
    const detail = `${run.stderr ?? ""}\n${run.stdout ?? ""}`;
    if (/release not found|not found/i.test(detail)) return null;
    throw new Error(`查詢 release 失敗：\n${detail.trim()}`);
  }
  return JSON.parse(run.stdout);
}

function publishRelease({ tag, version, notes, outDir, slug, plan }) {
  if (plan.deleteDraft) {
    mustGh(["release", "delete", tag, "--repo", slug, "--yes"]);
  }
  const notesDir = mkdtempSync(join(tmpdir(), "tt-notes-"));
  const notesPath = join(notesDir, "notes.md");
  writeFileSync(notesPath, notes.length ? `${notes.replace(/\n?$/, "\n")}` : "\n");
  const assets = readdirSync(outDir)
    .filter((name) => !name.startsWith("."))
    .map((name) => join(outDir, name));
  try {
    const create = [
      "release",
      "create",
      tag,
      "--repo",
      slug,
      "--draft",
      "--verify-tag",
      "--title",
      `Table Tavern ${version}`,
      "--notes-file",
      notesPath,
      ...assets,
    ];
    if (plan.prerelease) create.push("--prerelease");
    mustGh(create);
    // 草稿建完、檔案傳完，才改成公開。按 tag 更新要用 gh release edit
    // （GitHub 的更新端點要 release id，而且按 tag 查不到草稿）。
    const edit = [
      "release",
      "edit",
      tag,
      "--repo",
      slug,
      "--draft=false",
      `--prerelease=${plan.prerelease}`,
      `--latest=${plan.latest}`,
    ];
    try {
      mustGh(edit);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      throw new Error(
        `${message}\n草稿 release 可能已留下，下次正式跑會先刪草稿再重傳。`,
      );
    }
  } finally {
    rmSync(notesDir, { recursive: true, force: true });
  }
}

function verifyWithMinisign({ file, sigText, pubkeyText }) {
  const dir = mkdtempSync(join(tmpdir(), "tt-minisign-"));
  try {
    const pubPath = join(dir, "pub.key");
    const sigPath = join(dir, "artifact.sig");
    writeFileSync(pubPath, decodeMinisignArmor("公鑰", pubkeyText));
    writeFileSync(sigPath, decodeMinisignArmor(basename(file) + ".sig", sigText));
    const run = spawnSync(
      "minisign",
      ["-V", "-m", file, "-x", sigPath, "-p", pubPath],
      { encoding: "utf8" },
    );
    if (run.error) {
      if (run.error.code === "ENOENT") {
        throw new Error(
          "找不到 minisign 執行檔。CI（ubuntu）用 apt 安裝；" +
            "本機演練請加 --skip-verify，不要為了這步裝系統套件。",
        );
      }
      throw new Error(`minisign 起不來：${run.error.message}`);
    }
    if (run.status !== 0) {
      const detail = (run.stderr || run.stdout || "").trim();
      throw new Error(`minisign 驗簽失敗（${basename(file)}）：\n${detail}`);
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

export function finalizeRelease(options) {
  const mode = options.mode;
  if (mode !== "rehearsal" && mode !== "release") {
    throw new Error("--mode 要是 rehearsal 或 release");
  }
  if (options.skipVerify && mode === "release") {
    throw new Error("正式模式不能 --skip-verify");
  }
  if (!options.windowsDir || !options.macosDir || !options.outDir) {
    throw new Error("需要 --windows、--macos、--out");
  }
  if (mode === "release" && !options.tag) {
    throw new Error("正式模式需要 --tag（應為 v<版本>）");
  }
  if (options.pubDate !== undefined && !RFC3339.test(options.pubDate)) {
    throw new Error(`--pub-date 不是 RFC3339：${options.pubDate}`);
  }

  const root = options.root ?? ROOT;
  const read = readVersions(root);
  const checked = evaluateVersions(read, mode === "release" ? options.tag : undefined);
  if (read.errors.length || !checked.ok) {
    throw new Error(
      ["版本不一致：", ...checked.lines, ...read.errors, ...checked.problems].join("\n"),
    );
  }
  const version = checked.version;
  const tag = options.tag || `v${version}`;

  const configPath = options.configPath ?? join(root, "src-tauri/tauri.conf.json");
  const changelogPath = options.changelogPath ?? join(root, "CHANGELOG.md");
  const conf = JSON.parse(readFileSync(configPath, "utf8"));
  const updater = readUpdaterConfig(conf);
  if (process.env.GITHUB_REPOSITORY && process.env.GITHUB_REPOSITORY !== updater.slug) {
    throw new Error(
      `GITHUB_REPOSITORY（${process.env.GITHUB_REPOSITORY}）與 endpoints 的 repo` +
        `（${updater.slug}）不一致`,
    );
  }

  const changelog = readFileSync(changelogPath, "utf8");
  const extracted = extractChangelogNotes(changelog, version);
  const notes = resolveNotes({ ...extracted, mode, version });

  assertSafeOut(options.outDir, [options.windowsDir, options.macosDir]);
  const outDir = resolve(options.outDir);
  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });

  const planned = planReleaseFiles(
    { windows: walk(options.windowsDir), macos: walk(options.macosDir) },
    version,
  );
  for (const copy of planned.copies) {
    const srcRoot = copy.platform === "windows" ? options.windowsDir : options.macosDir;
    copyFileSync(join(srcRoot, copy.from), join(outDir, copy.to));
  }

  const exeSignature = readFileSync(join(outDir, `${planned.names.exe}.sig`), "utf8");
  const appSignature = readFileSync(join(outDir, `${planned.names.appTarGz}.sig`), "utf8");
  const pubDate = options.pubDate ?? new Date().toISOString();
  const latest = buildLatestJson({
    version,
    notes: notes.notes,
    pubDate,
    tag,
    repoBase: updater.repoBase,
    exeSignature,
    appSignature,
  });
  writeFileSync(join(outDir, "latest.json"), `${JSON.stringify(latest, null, 2)}\n`);

  console.log(`模式：${mode}`);
  console.log(`版本：${version}`);
  console.log(`下載路徑用的 tag：${tag}`);
  for (const copy of planned.copies) {
    console.log(`改名：${copy.from} → ${copy.to}`);
  }
  if (notes.warning) console.log(notes.warning);
  console.log(`寫入 ${join(outDir, "latest.json")}`);

  if (!options.skipVerify) {
    for (const item of planned.requireSig) {
      verifyWithMinisign({
        file: join(outDir, item.file),
        sigText: readFileSync(join(outDir, item.sig), "utf8"),
        pubkeyText: updater.pubkey,
      });
      console.log(`驗簽通過：${item.file}`);
    }
  } else {
    console.log("已跳過驗簽（--skip-verify）。");
  }

  if (mode === "rehearsal") {
    console.log("演練模式：不建 GitHub release。");
    return { version, tag, outDir, latest };
  }

  const existing = lookupRelease(tag, updater.slug);
  const releasePlan = githubReleasePlan({
    version,
    existing: existing ? { isDraft: existing.isDraft } : null,
    tag,
  });
  if (!releasePlan.ok) throw new Error(releasePlan.error);
  publishRelease({
    tag,
    version,
    notes: notes.notes,
    outDir,
    slug: updater.slug,
    plan: releasePlan,
  });
  const kind = releasePlan.prerelease ? "預發布，不當 latest" : "最新正式版";
  console.log(`已公開 release ${tag}（${kind}）。`);
  return { version, tag, outDir, latest };
}

function parseArgs(argv) {
  const out = { skipVerify: false };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const take = () => {
      const value = argv[++i];
      if (value == null || value.startsWith("--")) throw new Error(`${arg} 缺值`);
      return value;
    };
    switch (arg) {
      case "--mode":
        out.mode = take();
        break;
      case "--windows":
        out.windowsDir = take();
        break;
      case "--macos":
        out.macosDir = take();
        break;
      case "--out":
        out.outDir = take();
        break;
      case "--tag":
        out.tag = take();
        break;
      case "--changelog":
        out.changelogPath = take();
        break;
      case "--config":
        out.configPath = take();
        break;
      case "--pub-date":
        out.pubDate = take();
        break;
      case "--skip-verify":
        out.skipVerify = true;
        break;
      case "--help":
        out.help = true;
        break;
      default:
        throw new Error(`不認得的參數：${arg}`);
    }
  }
  return out;
}

function isDirectRun() {
  const entry = process.argv[1];
  if (!entry) return false;
  return import.meta.url === pathToFileURL(resolve(entry)).href;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    console.log(
      "用法：node scripts/release/finalize.mjs " +
        "--mode rehearsal|release --windows <dir> --macos <dir> --out <dir> " +
        "[--tag <tag>] [--skip-verify] [--pub-date <RFC3339>]",
    );
    return;
  }
  finalizeRelease(args);
}

if (isDirectRun()) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  }
}
