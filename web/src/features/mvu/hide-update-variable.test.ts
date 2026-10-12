import { describe, expect, it } from "vitest";
import reply from "@desktop/shared/contracts/reply-cleanup/cases.json";
import { hideUpdateVariable, hideUpdateVariableStreaming } from "./hide-update-variable";

const SHARED = ["unclosed-update", "unclosed-update-upper", "closed-update-then-text"];

describe("hiding <UpdateVariable> on screen", () => {
  for (const name of SHARED) {
    const found = reply.cases.find((entry) => entry.name === name);
    if (!found) throw new Error(`共用案例 ${name} 不見了`);
    it(`${name}: matches the desktop final and stream columns`, () => {
      expect(hideUpdateVariable(found.input)).toBe(found.final_complete);
      expect(hideUpdateVariableStreaming(found.input)).toBe(found.stream);
    });
  }

  it("streaming holds back a half-written tag", () => {
    expect(hideUpdateVariableStreaming("正文<Upd")).toBe("正文");
  });

  it("strips every closed block in one message", () => {
    expect(hideUpdateVariable("甲<UpdateVariable>a</UpdateVariable>乙<UpdateVariable>b</UpdateVariable>丙")).toBe("甲乙丙");
  });

  it("a cut-off block keeps the status placeholder the host appended", () => {
    expect(hideUpdateVariable("正文<UpdateVariable>半截\n\n<StatusPlaceHolderImpl/>")).toBe("正文\n\n<StatusPlaceHolderImpl/>");
  });
});
