import { expect, it } from "vitest";
import { RULE_BOOKS, bookAdded, ruleBookOf, ruleKeys } from "./rule-books";

const [srd] = RULE_BOOKS;
const entry = (title: string) => {
  const book = JSON.parse(srd.json) as { entries: Record<string, { comment: string; key: string[] }> };
  const found = Object.values(book.entries).find((e) => e.comment === title)!;
  return { title, keys: found.key };
};

it("folds rule-book keywords to the UI language plus English", () => {
  const advantage = entry("Advantage and Disadvantage");
  expect(ruleBookOf(advantage)).toBe(srd);
  expect(ruleKeys(srd, advantage, "zh-TW")).toEqual({
    shown: ["advantage", "disadvantage", "優勢", "劣勢"],
    hidden: 8,
  });
  const edited = { ...advantage, keys: [...advantage.keys, "佔上風"] };
  expect(ruleKeys(srd, edited, "en").shown).toEqual(["advantage", "disadvantage", "佔上風"]);
  const repeated = { ...advantage, keys: [...advantage.keys, "Advantage", "優勢"] };
  expect(ruleKeys(srd, repeated, "zh-TW").shown).toEqual(["advantage", "disadvantage", "優勢", "劣勢"]);
  expect(ruleBookOf({ title: "沃恩的酒館" })).toBeUndefined();
});

it("keeps fingerprints and titles unique, and each book parseable with an attribution entry", () => {
  const titles = RULE_BOOKS.flatMap((book) => [...book.titles]);
  expect(new Set(titles).size).toBe(titles.length);
  for (const book of RULE_BOOKS) {
    for (const other of RULE_BOOKS) {
      if (other === book) continue;
      expect(book.fingerprint.includes(other.fingerprint)).toBe(false);
    }
    const data = JSON.parse(book.json) as {
      name: string;
      entries: Record<string, { comment: string; content: string; key: string[]; constant: boolean }>;
    };
    expect(data.name).toBe(book.name);
    const values = Object.values(data.entries);
    expect(values.length).toBeGreaterThanOrEqual(21);
    expect(values.length).toBeLessThanOrEqual(61);
    const credit = values.find((entry) => entry.content.includes(book.fingerprint));
    expect(credit?.key).toEqual([]);
    expect(credit?.constant).toBe(false);
    const listed = values.map((entry) => ({ title: entry.comment, content: entry.content }));
    expect(bookAdded(book, listed)).toBe(true);
    for (const other of RULE_BOOKS) {
      if (other !== book) expect(bookAdded(other, listed)).toBe(false);
    }
    for (const entry of values) {
      const folded = entry.key.map((key) => key.toLowerCase());
      expect(new Set(folded).size).toBe(folded.length);
      if (entry === credit || book.id === "srd51") continue;
      const lines = entry.content.split("\n").length;
      expect(lines).toBeGreaterThanOrEqual(2);
      expect(lines).toBeLessThanOrEqual(5);
    }
  }
});
