import { describe, expect, it } from "vitest";
import { fillSkeletonPlaceholders, type StateNode } from "./refactor-shell";

describe("fillSkeletonPlaceholders", () => {
  it("值原文放回、不 escape：清單欄位的內層標籤要留給卡的顯示腳本解析", () => {
    const tree: Record<string, StateNode> = { 物品: "<Item>短劍</Item><Item>火把</Item>" };

    expect(fillSkeletonPlaceholders("<Bag>{{物品}}</Bag>", tree)).toBe(
      "<Bag><Item>短劍</Item><Item>火把</Item></Bag>",
    );
  });

  it("巢狀路徑逐層查值", () => {
    const tree: Record<string, StateNode> = { World: { Time: "清晨" }, 亞瑟: { HP: "480/500" } };

    expect(fillSkeletonPlaceholders("{{World.Time}} / {{亞瑟.HP}}", tree)).toBe("清晨 / 480/500");
  });

  it("路徑查不到就換成空字串", () => {
    const tree: Record<string, StateNode> = { World: { Time: "清晨" } };

    expect(fillSkeletonPlaceholders("[{{World.Weather}}][{{Nope.Nothing}}]", tree)).toBe("[][]");
  });

  it("路徑落在分支節點（非葉子）也換成空字串", () => {
    const tree: Record<string, StateNode> = { World: { Time: "清晨" } };

    expect(fillSkeletonPlaceholders("[{{World}}]", tree)).toBe("[]");
  });

  it("佔位符內容含換行就不算佔位符，原樣保留", () => {
    const skeleton = "{{World.\nTime}}";
    const tree: Record<string, StateNode> = { World: { Time: "正午" } };

    expect(fillSkeletonPlaceholders(skeleton, tree)).toBe(skeleton);
  });

  it("同一份骨架裡多個佔位符各自替換", () => {
    const tree: Record<string, StateNode> = { a: "1", b: "2" };

    expect(fillSkeletonPlaceholders("{{a}}-{{b}}-{{a}}", tree)).toBe("1-2-1");
  });
});
