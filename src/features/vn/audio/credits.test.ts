import { describe, expect, it } from "vitest";
import type { AudioEntry } from "../../../shared/contracts/backend-contracts";
import { creditLines, licenseLabel, sourceHost } from "./credits";

function entry(partial: Partial<AudioEntry> & Pick<AudioEntry, "kind">): AudioEntry {
  return {
    key: "k",
    title: "t",
    author: "",
    source_url: "",
    license: "",
    ...partial,
  };
}

describe("licenseLabel", () => {
  it("maps the three known licenses and keeps anything else", () => {
    expect(licenseLabel("https://creativecommons.org/licenses/by/4.0/")).toBe("CC BY 4.0");
    expect(licenseLabel("Attribution")).toBe("CC BY");
    expect(licenseLabel("Creative Commons 0")).toBe("CC0");
    expect(licenseLabel("  CC-BY-SA  ")).toBe("CC-BY-SA");
  });
});

describe("sourceHost", () => {
  it("drops a leading www and returns nothing for a bad url", () => {
    expect(sourceHost("https://www.incompetech.com/music/royalty-free/")).toBe("incompetech.com");
    expect(sourceHost("https://freesound.org/people/x")).toBe("freesound.org");
    expect(sourceHost("not a url")).toBe("");
    expect(sourceHost("")).toBe("");
  });
});

describe("creditLines", () => {
  it("splits bgm and sfx, dedupes in first-seen order, and drops empty groups", () => {
    const lines = creditLines([
      entry({ kind: "sfx", author: "Ann", source_url: "https://www.freesound.org/a", license: "Creative Commons 0" }),
      entry({ kind: "bgm", author: "Kevin MacLeod", source_url: "https://www.incompetech.com/a", license: "https://creativecommons.org/licenses/by/4.0/" }),
      entry({ kind: "bgm", author: "Kevin MacLeod", source_url: "https://incompetech.com/b", license: "Attribution" }),
      entry({ kind: "bgm", author: "  Ada  ", source_url: "notaurl", license: "" }),
      entry({ kind: "other", author: "Skip" }),
    ]);
    expect(lines.map((line) => line.kind)).toEqual(["bgm", "sfx"]);
    expect(lines[0]).toMatchObject({
      authors: ["Kevin MacLeod", "Ada"],
      hosts: ["incompetech.com"],
      licenses: ["CC BY 4.0", "CC BY"],
      more: 0,
    });
    expect(lines[1]).toMatchObject({ authors: ["Ann"], hosts: ["freesound.org"], licenses: ["CC0"], more: 0 });
  });

  it("keeps eight authors and reports how many were left off", () => {
    const authors = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];
    const [line] = creditLines(authors.map((author) => entry({ kind: "bgm", author, key: author })));
    expect(line.authors).toEqual(authors.slice(0, 8));
    expect(line.more).toBe(2);
  });
});
