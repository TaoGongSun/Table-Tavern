import { describe, expect, it } from "vitest";
import { fillShellPlaceholders, type StateNode } from "./refactor-shell";

describe("fillShellPlaceholders", () => {
  it("換成葉子值，HTML escape 五個特殊字元", () => {
    const tree: Record<string, StateNode> = { name: `<script>alert("x")</script> & 'quote'` };

    const result = fillShellPlaceholders("<div>{{name}}</div>", tree);

    expect(result).toBe(
      "<div>&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; &#39;quote&#39;</div>",
    );
    expect(result).not.toContain("<script>");
  });

  it("巢狀路徑逐層查值", () => {
    const tree: Record<string, StateNode> = { World: { Time: "清晨" }, 亞瑟: { HP: "480/500" } };

    expect(fillShellPlaceholders("{{World.Time}} / {{亞瑟.HP}}", tree)).toBe("清晨 / 480/500");
  });

  it("路徑查不到就換成空字串", () => {
    const tree: Record<string, StateNode> = { World: { Time: "清晨" } };

    expect(fillShellPlaceholders("[{{World.Weather}}][{{Nope.Nothing}}]", tree)).toBe("[][]");
  });

  it("路徑落在分支節點（非葉子）也換成空字串", () => {
    const tree: Record<string, StateNode> = { World: { Time: "清晨" } };

    expect(fillShellPlaceholders("[{{World}}]", tree)).toBe("[]");
  });

  it("非佔位的花括號不誤傷：單花括號 CSS／JS 區塊原樣保留", () => {
    const shell = "<style>.foo { color: red; }</style><script>var o = {a:1};</script>{{World.Time}}";
    const tree: Record<string, StateNode> = { World: { Time: "正午" } };

    const result = fillShellPlaceholders(shell, tree);

    expect(result).toContain(".foo { color: red; }");
    expect(result).toContain("var o = {a:1};");
    expect(result).toContain("正午");
  });

  it("佔位符內容含換行就不算佔位符，原樣保留", () => {
    const shell = "{{World.\nTime}}";
    const tree: Record<string, StateNode> = { World: { Time: "正午" } };

    expect(fillShellPlaceholders(shell, tree)).toBe(shell);
  });

  it("同一份殼裡多個佔位符各自替換", () => {
    const tree: Record<string, StateNode> = { a: "1", b: "2" };

    expect(fillShellPlaceholders("{{a}}-{{b}}-{{a}}", tree)).toBe("1-2-1");
  });
});
