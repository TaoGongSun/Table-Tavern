import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { tableProblem, tableTextProblem, VARS_TABLE_LIMITS } from "./vars-table";

/** 緊湊寫法剛好 `size` 位元組的表（與 Rust json.rs 測試同一個造法）。 */
function tableOfSize(size: number): string {
  const entries: string[] = [];
  let used = 2;
  for (let index = 0; ; index++) {
    const key = `k${index}`;
    const fixed = (index > 0 ? 1 : 0) + key.length + 2 + 1 + 2;
    const room = size - used - fixed;
    const take = Math.min(room, 60_000);
    entries.push(`"${key}":"${"a".repeat(take)}"`);
    used += fixed + take;
    if (take === room) break;
  }
  const text = `{${entries.join(",")}}`;
  expect(text.length).toBe(size);
  return text;
}

interface BoundaryCase {
  name: string;
  number: string;
  spaces: number;
  raw_offset: number;
  text: "ok" | "too-large";
  value: "ok" | "too-large";
}

/** 照 vars-table-boundary.json 的造法：原文剛好 `total` 位元組。 */
function boundaryTable(number: string, spaces: number, total: number): string {
  let text = `{${" ".repeat(spaces)}"n":${number}`;
  let used = text.length + 1;
  for (let index = 0; used < total; index++) {
    const key = `k${index}`;
    const room = total - used - (key.length + 6);
    const take = room <= 60_000 ? room : Math.min(60_000, room - 20);
    text += `,"${key}":"${"a".repeat(take)}"`;
    used += key.length + 6 + take;
  }
  text += "}";
  expect(text.length).toBe(total);
  return text;
}

describe("整張大小的共用邊界案例（vars-table-boundary.json，桌面版 json.rs 讀同一份）", () => {
  const { cases } = JSON.parse(readFileSync(new URL("./vars-table-boundary.json", import.meta.url), "utf8")) as { cases: BoundaryCase[] };
  for (const item of cases) {
    it(item.name, () => {
      const text = boundaryTable(item.number, item.spaces, VARS_TABLE_LIMITS.tableBytes + item.raw_offset);
      expect(tableTextProblem(text) ?? "ok", "text").toBe(item.text);
      expect(tableProblem(JSON.parse(text)) ?? "ok", "value").toBe(item.value);
    });
  }
});

describe("卡片變數表上限（與桌面版 json.rs 同一組）", () => {
  it("整張大小量緊湊寫法：剛好上限收、原文縮排過照樣收、多一個位元組拒", () => {
    const exact = tableOfSize(VARS_TABLE_LIMITS.tableBytes);
    expect(tableTextProblem(exact)).toBeNull();
    const padded = exact.split(",").join(" ,\n   ").split(":").join(" : ");
    expect(padded.length).toBeGreaterThan(VARS_TABLE_LIMITS.tableBytes);
    expect(tableTextProblem(padded)).toBeNull();
    expect(tableTextProblem(tableOfSize(VARS_TABLE_LIMITS.tableBytes + 1).split(",").join(" , "))).toBe("too-large");
    expect(tableTextProblem(`{${" ".repeat(VARS_TABLE_LIMITS.rawBytes)}"a":1}`)).toBe("too-large");
  });

  it("單字串 64 KiB（UTF-8 位元組）、深度、子項、鍵", () => {
    expect(tableProblem({ s: "a".repeat(64 * 1024) })).toBeNull();
    expect(tableProblem({ s: "a".repeat(64 * 1024 + 1) })).toBe("string-too-long");
    expect(tableProblem({ s: "字".repeat(21846) })).toBe("string-too-long");
    let nested: unknown = 1;
    for (let depth = 0; depth < 31; depth++) nested = { a: nested };
    expect(tableProblem(nested)).toBeNull();
    expect(tableProblem({ a: nested })).toBe("too-deep");
    expect(tableProblem({ l: new Array(10_001).fill(0) })).toBe("too-many-children");
    expect(tableProblem({ "": 1 })).toBe("empty-key");
    expect(tableProblem({ ["鍵".repeat(256)]: 1 })).toBeNull();
    expect(tableProblem({ ["k".repeat(257)]: 1 })).toBe("key-too-long");
  });

  it("孤立代理字元與非物件", () => {
    expect(tableTextProblem('{"s":"\\ud800"}')).toBe("invalid-json");
    expect(tableTextProblem('{"\\udc00":1}')).toBe("invalid-json");
    expect(tableTextProblem('{"s":"\\ud83d\\ude00"}')).toBeNull();
    expect(tableProblem([1])).toBe("not-object");
  });
});
