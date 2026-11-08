import { expect, it } from "vitest";
import { RULE_BOOKS, ruleBookOf, ruleKeys } from "./rule-books";

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
  expect(ruleBookOf({ title: "沃恩的酒館" })).toBeUndefined();
});
