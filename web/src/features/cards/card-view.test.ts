import { existsSync, readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { decodeBase64Strict, decodeCardFile } from "./card-file";
import { importCardBytes, importCardFile, MAX_CARD_BYTES } from "./play-card";
import { bookEntriesKeyed, cardView } from "./card-view";

const CONTRACT = fileURLToPath(new URL("../../../../src/shared/contracts/card-view/", import.meta.url));
const read = (file: string) => new Uint8Array(readFileSync(CONTRACT + file));
const golden = JSON.parse(readFileSync(CONTRACT + "golden.json", "utf8")) as Record<string, unknown>;

describe("card view contract (shared golden file with src-tauri card_view_tests.rs)", () => {
  it("covers every fixture", () => {
    expect(Object.keys(golden).length).toBeGreaterThanOrEqual(8);
  });

  for (const [file, expected] of Object.entries(golden)) {
    it(`${file} matches the golden view`, () => {
      expect(cardView(read(file))).toEqual(expected);
    });
  }

  it("object-shaped entries count as entries and expand by numeric uid key", () => {
    const view = cardView(read("object-entries.json"));
    if ("error" in view) throw new Error(view.error);
    expect(view.route.book_entries).toBe(3);
    expect(view.books.worldbook.source).toBe("character_book");
    expect(view.books.worldbook.entries.map((entry) => entry.key)).toEqual(["0", "2", "10"]);
  });

  it("PNG reads tEXt chara before ccv3 and never iTXt", () => {
    expect(cardView(read("composite.png"))).toMatchObject({ source: "png:chara" });
    expect(cardView(read("ccv3-itxt.png"))).toMatchObject({ source: "png:ccv3" });
    expect(cardView(read("itxt-only.png"))).toEqual({ error: "card_png_no_data" });
  });
});

describe("card file decoding edge cases", () => {
  const bytes = (text: string) => new TextEncoder().encode(text);

  it("strict base64 like the desktop decoder", () => {
    expect(new TextDecoder().decode(decodeBase64Strict(bytes("SGVsbG8=")))).toBe("Hello");
    expect(() => decodeBase64Strict(bytes("SGVsbG8!"))).toThrow();
    expect(() => decodeBase64Strict(bytes("AA=A"))).toThrow();
    expect(() => decodeBase64Strict(bytes("SGVsbG8"))).toThrow();
  });

  it("broken PNGs and broken JSON report the desktop error codes", () => {
    const truncated = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 9]);
    expect(decodeCardFile(truncated)).toEqual({ ok: false, error: "png_invalid" });
    expect(decodeCardFile(bytes("{not json"))).toEqual({ ok: false, error: "card_json_invalid" });
    expect(decodeCardFile(new Uint8Array([0xef, 0xbb, 0xbf, ...bytes("{}")]))).toEqual({ ok: false, error: "card_json_invalid" });
    expect(decodeCardFile(new Uint8Array([0xff, 0xfe]))).toEqual({ ok: false, error: "card_json_invalid" });
  });

  /** PNG：簽章＋一串 chunk（CRC 填 0；解析不驗 CRC，同桌面版） */
  const png = (...chunks: [string, Uint8Array][]) => {
    const parts: Uint8Array[] = [new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])];
    for (const [kind, data] of chunks) {
      const head = new Uint8Array(8);
      new DataView(head.buffer).setUint32(0, data.length);
      head.set(bytes(kind), 4);
      parts.push(head, data, new Uint8Array(4));
    }
    const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
    let at = 0;
    for (const part of parts) {
      out.set(part, at);
      at += part.length;
    }
    return out;
  };
  const chara = bytes(`chara\u0000${Buffer.from('{"name":"莉亞"}').toString("base64")}`);

  it("skips a 200 KB unrelated tEXt chunk (huge keyword, no separator) and still finds chara", () => {
    const junk = new Uint8Array(200_000).fill(0x41);
    const keyed = new Uint8Array(200_001).fill(0x42);
    keyed[100_000] = 0;
    const decoded = decodeCardFile(png(["tEXt", junk], ["tEXt", keyed], ["tEXt", chara]));
    expect(decoded).toMatchObject({ ok: true, source: "png:chara", value: { name: "莉亞" } });
    expect(decodeCardFile(png(["tEXt", junk]))).toEqual({ ok: false, error: "card_png_no_data" });
  });

  it("entry expansion puts non-numeric keys last and skips non-object values", () => {
    expect(bookEntriesKeyed({ b: {}, "10": {}, a: {}, "9": {}, "1": "字串", "2": null }).map(([key]) => key)).toEqual(["9", "10", "a", "b"]);
  });
});

describe("import validity (contract 有效性規則)", () => {
  it("rejects a card name the desktop character route would reject", () => {
    expect(importCardBytes(read("multiline-name.json"))).toEqual({ ok: false, error: "name_not_single_line" });
    expect(importCardBytes(read("standalone-book.json"))).toEqual({ ok: false, error: "standalone_book" });
    expect(importCardBytes(read("composite.png"))).toMatchObject({ ok: true });
  });

  it("checks the file size before reading the file", async () => {
    let touched = false;
    const huge = { size: MAX_CARD_BYTES + 1, arrayBuffer: async () => ((touched = true), new ArrayBuffer(0)) };
    expect(await importCardFile(huge)).toEqual({ ok: false, error: "too_large" });
    expect(touched).toBe(false);
  });
});

// 本機實卡（TestCards/，gitignore）：先跑桌面版 writes_local_testcard_views_when_requested 寫出
// `<檔名>.rust.json`，再用同樣兩個環境變數跑這裡逐張比對。CI 沒設，略過。
const testCards = process.env.TT_CARD_VIEW_TESTCARDS;
const rustOut = process.env.TT_CARD_VIEW_OUT;
describe.runIf(Boolean(testCards && rustOut))("local TestCards match the desktop views", () => {
  const files = testCards && existsSync(testCards) ? readdirSync(testCards).filter((file) => /\.(png|json)$/.test(file)) : [];
  it("found cards", () => expect(files.length).toBeGreaterThan(0));
  for (const file of files) {
    it(file, () => {
      const rust = JSON.parse(readFileSync(`${rustOut}/${file}.rust.json`, "utf8")) as unknown;
      expect(cardView(new Uint8Array(readFileSync(`${testCards}/${file}`)))).toEqual(rust);
    });
  }
});
