import { describe, expect, it } from "vitest";
import { commitVariables, copyVariables, createChatVariables, turnCommits, VariableScope } from "./variables";

describe("變數的寫入紀錄（存檔匯出判斷這桌碰過哪些 global 鍵）", () => {
  it("寫入、刪除、加減都記；寫回同值也算", () => {
    const scope = new VariableScope({ a: 1 });
    scope.set("a", 1);
    scope.add("b", 2);
    scope.del("c");
    expect([...scope.written].sort()).toEqual(["a", "b", "c"]);
  });

  it("a failed indexed set is not recorded as written", () => {
    const scope = new VariableScope({ a: "不是 JSON" });
    scope.set("a", 1, { index: "0" });
    expect(scope.values.a).toBe("不是 JSON");
    expect(scope.written.has("a")).toBe(false);
    scope.set("list", "x", { index: "0" });
    expect(scope.values.list).toBe('["x"]');
    expect(scope.written.has("list")).toBe(true);
  });

  it("副本寫過的鍵在提交時併回原本的變數", () => {
    const variables = createChatVariables({}, {});
    const copy = copyVariables(variables);
    copy.global.set("g", 1);
    expect(variables.global.written.has("g")).toBe(false);
    commitVariables(variables, copy);
    expect(variables.global.written.has("g")).toBe(true);
  });

  it("given the last committed state, keys changed elsewhere during the turn survive every later commit (added, changed, deleted)", () => {
    const variables = createChatVariables({ kept: 1, changed: 1, gone: 1, mine: 1 }, {});
    const turn = turnCommits(variables);
    const copy = copyVariables(variables);
    copy.local.set("mine", 2);
    copy.local.set("changed", 9);
    variables.local.values.changed = 3;
    variables.local.values.added = 4;
    delete variables.local.values.gone;
    commitVariables(variables, copy, turn);
    expect(variables.local.values).toEqual({ kept: 1, changed: 3, mine: 2, added: 4 });
    // 下一發從回合開頭的副本重算：先前外部改過的鍵照樣保住
    commitVariables(variables, copyVariables(createChatVariables({ kept: 1, changed: 1, gone: 1, mine: 1 }, {})), turn);
    expect(variables.local.values).toEqual({ kept: 1, changed: 3, mine: 1, added: 4 });
  });
});
