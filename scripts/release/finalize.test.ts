import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { readVersions } from "../check-version.mjs";
import { finalizeRelease } from "./finalize.mjs";

const dirs: string[] = [];

function tempDir() {
  const dir = mkdtempSync(join(tmpdir(), "tt-finalize-"));
  dirs.push(dir);
  return dir;
}

afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function stage(version: string) {
  const root = tempDir();
  const windows = join(root, "windows");
  const macos = join(root, "macos");
  const out = join(root, "out");
  mkdirSync(windows);
  mkdirSync(macos);
  writeFileSync(join(windows, `Table Tavern_${version}_x64-setup.exe`), "exe-bytes");
  writeFileSync(join(windows, `Table Tavern_${version}_x64-setup.exe.sig`), "EXE-SIG\n");
  writeFileSync(join(macos, "Table Tavern.app.tar.gz"), "app-bytes");
  writeFileSync(join(macos, "Table Tavern.app.tar.gz.sig"), "APP-SIG\n");
  writeFileSync(join(macos, `Table Tavern_${version}_aarch64.dmg`), "dmg-bytes");
  const changelog = join(root, "CHANGELOG.md");
  writeFileSync(changelog, `## [${version}] — 2026-10-01\n\n這版說明\n`);
  return { windows, macos, out, changelog };
}

describe("finalizeRelease 演練", () => {
  it("跳過驗簽時改名並寫出 latest.json", () => {
    const version = readVersions().packageJson;
    expect(version).toBeTruthy();
    const staged = stage(version!);
    const result = finalizeRelease({
      mode: "rehearsal",
      windowsDir: staged.windows,
      macosDir: staged.macos,
      outDir: staged.out,
      tag: `test-v${version}`,
      changelogPath: staged.changelog,
      skipVerify: true,
      pubDate: "2026-10-01T03:04:05.000Z",
    });

    expect(result.latest.version).toBe(version);
    expect(result.latest.format_version).toBe(1);
    expect(result.latest.notes).toBe("這版說明");
    expect(result.latest.pub_date).toBe("2026-10-01T03:04:05.000Z");
    expect(result.latest.platforms["windows-x86_64"].signature).toBe("EXE-SIG\n");
    expect(result.latest.platforms["darwin-aarch64"].signature).toBe("APP-SIG\n");
    expect(result.latest.platforms["windows-x86_64"].url).toBe(
      "https://github.com/TaoGongSun/Table-Tavern/releases/download/" +
        `test-v${version}/TableTavern_${version}_x64-setup.exe`,
    );

    const exe = `TableTavern_${version}_x64-setup.exe`;
    expect(readFileSync(join(staged.out, exe), "utf8")).toBe("exe-bytes");
    expect(readFileSync(join(staged.out, `${exe}.sig`), "utf8")).toBe("EXE-SIG\n");
    expect(readFileSync(join(staged.out, `TableTavern_${version}_aarch64.dmg`), "utf8")).toBe(
      "dmg-bytes",
    );
    const written = JSON.parse(readFileSync(join(staged.out, "latest.json"), "utf8"));
    expect(written.format_version).toBe(1);
    expect(written).toEqual(result.latest);
  });

  it("空公鑰在驗簽那一步失敗", () => {
    const version = readVersions().packageJson ?? "";
    const staged = stage(version);
    const confPath = join(staged.windows, "..", "empty.conf.json");
    const conf = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
    conf.plugins.updater.pubkey = "";
    writeFileSync(confPath, JSON.stringify(conf));
    expect(() =>
      finalizeRelease({
        mode: "rehearsal",
        windowsDir: staged.windows,
        macosDir: staged.macos,
        outDir: staged.out,
        changelogPath: staged.changelog,
        configPath: confPath,
        pubDate: "2026-10-01T03:04:05.000Z",
      }),
    ).toThrow(/公鑰 不是 base64/);
  });

  it("正式模式拒絕跳過驗簽，也不會在缺 CHANGELOG 時去叫 gh", () => {
    const version = readVersions().packageJson ?? "";
    const staged = stage(version);
    expect(() =>
      finalizeRelease({
        mode: "release",
        tag: `v${version}`,
        windowsDir: staged.windows,
        macosDir: staged.macos,
        outDir: staged.out,
        changelogPath: staged.changelog,
        skipVerify: true,
        pubDate: "2026-10-01T03:04:05.000Z",
      }),
    ).toThrow(/不能 --skip-verify/);

    writeFileSync(staged.changelog, "## [Unreleased]\n\n沒有這個版本\n");
    expect(() =>
      finalizeRelease({
        mode: "release",
        tag: `v${version}`,
        windowsDir: staged.windows,
        macosDir: staged.macos,
        outDir: join(staged.out, "second"),
        changelogPath: staged.changelog,
        pubDate: "2026-10-01T03:04:05.000Z",
      }),
    ).toThrow(/CHANGELOG/);
  });
});
