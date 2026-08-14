import { describe, expect, it } from "vitest";
import { detectRefactorTristate } from "./refactor-mode";
import { type CardInterface, type InterfaceScript } from "./interface-card";

function script(): InterfaceScript {
  return { name: "s", find_regex: "/x/", replace_string: "<div>x</div>", trim_strings: [], min_depth: null, max_depth: null };
}

function card(overrides: Partial<CardInterface>): CardInterface {
  return { character_id: "c1", character_name: "角色", scripts: [], unsupported: null, opening: null, ...overrides };
}

describe("detectRefactorTristate", () => {
  it("無介面卡＝none（免問直跑角色線）", () => {
    expect(detectRefactorTristate([])).toBe("none");
    expect(detectRefactorTristate([card({})])).toBe("none");
  });

  it("有可用顯示腳本＝supported（進二選一）", () => {
    expect(detectRefactorTristate([card({ scripts: [script()] })])).toBe("supported");
  });

  it("只有 DRM／雲端載入器卡＝unsupported（擋下）", () => {
    expect(detectRefactorTristate([card({ unsupported: "remote_loader" })])).toBe("unsupported");
    expect(detectRefactorTristate([card({ unsupported: "scrypt" })])).toBe("unsupported");
  });

  it("混合桌取還有得救優先：一張可接管＋一張擋下＝supported", () => {
    expect(
      detectRefactorTristate([card({ unsupported: "remote_loader" }), card({ character_id: "c2", scripts: [script()] })]),
    ).toBe("supported");
  });
});
