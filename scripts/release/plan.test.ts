import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import {
  buildLatestJson,
  decodeMinisignArmor,
  extractChangelogNotes,
  githubReleasePlan,
  hasPrerelease,
  planReleaseFiles,
  readCurrentFormat,
  releaseFilenames,
  resolveNotes,
} from "./plan.mjs";

const emDash = "\u2014";

describe("extractChangelogNotes", () => {
  const markdown = [
    "## [Unreleased]",
    "",
    "還沒發",
    "",
    `## [0.2.0] ${emDash} 2026-07-24（內部測試版）`,
    "",
    "### 新增",
    "- 一項",
    "",
    `## [0.1.0] ${emDash} 2026-07-22（內部測試版）`,
    "",
    "舊的",
    "",
  ].join("\n");

  it("全形破折號的標題也切得到，而且不含下一個二級標題", () => {
    const notes = extractChangelogNotes(markdown, "0.2.0");
    expect(notes.found).toBe(true);
    expect(notes.notes).toBe("### 新增\n- 一項");
    expect(notes.notes).not.toContain("[0.1.0]");
    expect(notes.notes).not.toContain("還沒發");
  });

  it("沒有日期後綴的標題也收", () => {
    expect(extractChangelogNotes("## [1.0.0]\n\n正文\n", "1.0.0")).toEqual({
      found: true,
      notes: "正文",
    });
  });

  it("0.2.0 不會對上 0.2.0-beta", () => {
    const text = "## [0.2.0-beta.1]\n\n預發布\n";
    expect(extractChangelogNotes(text, "0.2.0").found).toBe(false);
    expect(extractChangelogNotes(text, "0.2.0-beta.1").notes).toBe("預發布");
  });

  it("repo 裡的 0.2.0 段落切得到，且不會吃進 0.1.0", () => {
    const changelog = readFileSync("CHANGELOG.md", "utf8");
    const notes = extractChangelogNotes(changelog, "0.2.0");
    expect(notes.found).toBe(true);
    expect(notes.notes).not.toContain("## [0.1.0]");
    expect(notes.notes.length).toBeGreaterThan(0);
  });
});

describe("resolveNotes", () => {
  it("正式發版缺段落就失敗，演練留空", () => {
    expect(() =>
      resolveNotes({ found: false, notes: "", mode: "release", version: "9.9.9" }),
    ).toThrow(/9\.9\.9/);
    const rehearsal = resolveNotes({
      found: false,
      notes: "",
      mode: "rehearsal",
      version: "9.9.9",
    });
    expect(rehearsal.notes).toBe("");
    expect(rehearsal.warning).toContain("演練");
  });
});

describe("hasPrerelease", () => {
  it("有預發布段才算", () => {
    expect(hasPrerelease("0.2.0")).toBe(false);
    expect(hasPrerelease("0.2.0+build.1")).toBe(false);
    expect(hasPrerelease("0.2.0+001")).toBe(false);
    expect(hasPrerelease("0.3.0-beta.1")).toBe(true);
    expect(hasPrerelease("0.3.0-rc.1+build")).toBe(true);
  });

  it("不是 SemVer 就丟錯", () => {
    expect(() => hasPrerelease("v0.2.0")).toThrow(/SemVer/);
    expect(() => hasPrerelease("01.2.0")).toThrow(/SemVer/);
    expect(() => hasPrerelease("0.2.0-01")).toThrow(/SemVer/);
  });
});

describe("githubReleasePlan", () => {
  it("沒有舊 release：正式版設為 latest，預發布不當 latest", () => {
    expect(githubReleasePlan({ version: "0.2.0", existing: null, tag: "v0.2.0" })).toEqual({
      ok: true,
      deleteDraft: false,
      prerelease: false,
      latest: true,
    });
    expect(
      githubReleasePlan({ version: "0.3.0-beta.1", existing: null, tag: "v0.3.0-beta.1" }),
    ).toMatchObject({ ok: true, prerelease: true, latest: false });
  });

  it("草稿先刪，已公開就失敗", () => {
    expect(
      githubReleasePlan({ version: "0.2.0", existing: { isDraft: true }, tag: "v0.2.0" })
        .deleteDraft,
    ).toBe(true);
    const published = githubReleasePlan({
      version: "0.2.0",
      existing: { isDraft: false },
      tag: "v0.2.0",
    });
    expect(published.ok).toBe(false);
    expect(published.error).toContain("已公開");
  });
});

describe("檔名與 latest.json", () => {
  const inputs = {
    windows: [
      "Table Tavern_0.2.0_x64-setup.exe",
      "Table Tavern_0.2.0_x64-setup.exe.sig",
    ],
    macos: [
      "Table Tavern.app.tar.gz",
      "Table Tavern.app.tar.gz.sig",
      "Table Tavern_0.2.0_aarch64.dmg",
      ".DS_Store",
    ],
  };

  it("改成無空格檔名；dmg 沒有 sig 就不列入必驗", () => {
    const plan = planReleaseFiles(inputs, "0.2.0");
    expect(plan.names).toEqual(releaseFilenames("0.2.0"));
    expect(plan.copies.map((copy) => copy.to)).toEqual([
      "TableTavern_0.2.0_x64-setup.exe",
      "TableTavern_0.2.0_x64-setup.exe.sig",
      "TableTavern_0.2.0_aarch64.app.tar.gz",
      "TableTavern_0.2.0_aarch64.app.tar.gz.sig",
      "TableTavern_0.2.0_aarch64.dmg",
    ]);
    expect(plan.requireSig.map((item) => item.file)).toEqual([
      "TableTavern_0.2.0_x64-setup.exe",
      "TableTavern_0.2.0_aarch64.app.tar.gz",
    ]);
  });

  it("msi、.dmg.sig、多餘檔都當不認得的產物拒絕", () => {
    expect(() =>
      planReleaseFiles(
        { windows: [...inputs.windows, "Table Tavern_0.2.0_x64.msi"], macos: inputs.macos },
        "0.2.0",
      ),
    ).toThrow(/不認得/);
    expect(() =>
      planReleaseFiles(
        {
          windows: inputs.windows,
          macos: [...inputs.macos, "Table Tavern_0.2.0_aarch64.dmg.sig"],
        },
        "0.2.0",
      ),
    ).toThrow(/不認得/);
    expect(() =>
      planReleaseFiles(
        { windows: inputs.windows, macos: [...inputs.macos, "notes.txt"] },
        "0.2.0",
      ),
    ).toThrow(/不認得/);
  });

  it("檔名不必含架構字樣，每種仍恰好一個", () => {
    const plan = planReleaseFiles(
      { windows: ["setup.exe", "setup.exe.sig"], macos: inputs.macos },
      "0.2.0",
    );
    expect(plan.copies[0]?.to).toBe("TableTavern_0.2.0_x64-setup.exe");
    expect(plan.copies).toHaveLength(5);
  });

  it("latest.json 的 signature 用 .sig 原文，url 帶 tag 與新檔名", () => {
    const latest = buildLatestJson({
      version: "0.2.0",
      notes: "說明",
      pubDate: "2026-10-01T00:00:00.000Z",
      tag: "test-v0.2.0",
      repoBase: "https://github.com/TaoGongSun/Table-Tavern",
      exeSignature: "EXE-SIG\n",
      appSignature: "APP-SIG\n",
      formatVersion: 1,
    });
    expect(latest).toEqual({
      version: "0.2.0",
      notes: "說明",
      pub_date: "2026-10-01T00:00:00.000Z",
      format_version: 1,
      platforms: {
        "windows-x86_64": {
          url:
            "https://github.com/TaoGongSun/Table-Tavern/releases/download/" +
            "test-v0.2.0/TableTavern_0.2.0_x64-setup.exe",
          signature: "EXE-SIG\n",
        },
        "darwin-aarch64": {
          url:
            "https://github.com/TaoGongSun/Table-Tavern/releases/download/" +
            "test-v0.2.0/TableTavern_0.2.0_aarch64.app.tar.gz",
          signature: "APP-SIG\n",
        },
      },
    });
  });
});

describe("CURRENT_FORMAT", () => {
  it("只認 const，不認 current_format()，而且必須是正整數", () => {
    const source = [
      "pub fn current_format() -> u64 { 99 }",
      "pub const CURRENT_FORMAT: u64 = 1;",
    ].join("\n");
    expect(readCurrentFormat(source)).toBe(1);
    expect(readCurrentFormat(readFileSync("src-tauri/src/data/format/marker.rs", "utf8"))).toBe(1);
    expect(() => readCurrentFormat("pub const CURRENT_FORMAT: u64 = 0;")).toThrow(/正整數/);
    expect(() => readCurrentFormat("沒有常數")).toThrow(/找不到/);
  });
});

describe("公鑰", () => {
  it("base64 解碼後才是 minisign 檔；空的擋在這一步", () => {
    const armor = "untrusted comment: minisign public key: TEST\nRWTTEST\n";
    const encoded = Buffer.from(armor).toString("base64");
    expect(decodeMinisignArmor("公鑰", encoded)).toBe(armor);
    expect(() => decodeMinisignArmor("公鑰", armor)).toThrow(/不是 base64/);
    expect(() => decodeMinisignArmor("公鑰", "")).toThrow(/不是 base64/);
    expect(() => decodeMinisignArmor("公鑰", "   ")).toThrow(/不是 base64/);
  });
});
