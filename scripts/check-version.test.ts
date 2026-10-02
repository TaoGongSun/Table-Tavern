import { spawnSync } from "node:child_process";
import { describe, expect, it } from "vitest";
import { cargoPackageVersion, evaluateVersions, readVersions } from "./check-version.mjs";

describe("cargoPackageVersion", () => {
  it("只認 [package] 裡的 version，不看依賴", () => {
    const toml = `
[package]
name = "table-tavern"
version = "0.2.0"

[dependencies]
serde = { version = "1" }
`;
    expect(cargoPackageVersion(toml)).toBe("0.2.0");
  });

  it("單引號也收", () => {
    expect(cargoPackageVersion("[package]\nversion = '1.2.3'\n")).toBe("1.2.3");
  });

  it("沒有 [package].version 就回 null", () => {
    expect(cargoPackageVersion("[dependencies]\nversion = \"9.9.9\"\n")).toBe(null);
  });
});

describe("evaluateVersions", () => {
  const same = { packageJson: "0.2.0", cargo: "0.2.0", tauri: "0.2.0" };

  it("三處相同且沒給 tag 就過", () => {
    const result = evaluateVersions(same, undefined);
    expect(result.ok).toBe(true);
    expect(result.version).toBe("0.2.0");
  });

  it("有一處不同就失敗，並帶上三處的值", () => {
    const result = evaluateVersions({ ...same, cargo: "0.2.1" }, undefined);
    expect(result.ok).toBe(false);
    expect(result.lines.join("\n")).toContain("0.2.1");
    expect(result.lines.join("\n")).toContain("package.json: 0.2.0");
  });

  it("tag 要等於 v<版本>", () => {
    expect(evaluateVersions(same, "v0.2.0").ok).toBe(true);
    const bad = evaluateVersions(same, "v0.2.1");
    expect(bad.ok).toBe(false);
    expect(bad.problems.join("\n")).toContain("tag 應為 v0.2.0");
    expect(bad.lines.join("\n")).toContain("tag: v0.2.1");
  });

  it("test-v* 不算正式 tag", () => {
    expect(evaluateVersions(same, "test-v0.2.0").ok).toBe(false);
  });
});

describe("check-version.mjs", () => {
  it("目前 repo 三處一致", () => {
    const read = readVersions();
    expect(read.errors).toEqual([]);
    expect(evaluateVersions(read, undefined).ok).toBe(true);
  });

  it("不帶 tag 時 exit 0，帶錯 tag 時印出各處的值", () => {
    const version = readVersions().packageJson;
    const ok = spawnSync("node", ["scripts/check-version.mjs"], { encoding: "utf8" });
    expect(ok.status).toBe(0);
    expect(ok.stdout).toContain(version);

    const bad = spawnSync("node", ["scripts/check-version.mjs", "v9.9.9"], {
      encoding: "utf8",
    });
    expect(bad.status).not.toBe(0);
    expect(bad.stderr).toContain(`package.json: ${version}`);
    expect(bad.stderr).toContain(`Cargo.toml [package].version: ${version}`);
    expect(bad.stderr).toContain(`tauri.conf.json: ${version}`);
    expect(bad.stderr).toContain("tag: v9.9.9");
  });
});
