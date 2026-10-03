import { describe, expect, it } from "vitest";
import { composeFrames, confirmFrames } from "./refactor-frame";
import { mergeRefactorInterfaces, type RefactorInterface } from "./refactor-review";

const STATUS = '<Status_block>\n状态栏:\n  地点: "📍 {{地點}}"\n</Status_block>';
const BODY = "{{本回合.正文}}";

const defining = (overrides: Partial<RefactorInterface> = {}): RefactorInterface =>
  ({
    state_fields: { 地點: "北境驿站 前院" },
    source_uids: ["21"],
    raw: "STATE 21",
    shell: STATUS,
    rules: { 地點: { kind: "text", update: "replace", inject: "turn" } },
    guide: "地點每回合必報",
    ...overrides,
  }) as RefactorInterface;

describe("confirmFrames", () => {
  it("只有定義骨架含相同容器標籤的候選才當外框，其餘照一般條目展開", () => {
    const frame = { uid: "22", tags: ["maintext", "Status_block"] };
    const lone = { uid: "30", tags: ["Other_block"] };
    expect(confirmFrames([frame, lone], [STATUS])).toEqual({ frames: [frame], expand: [lone] });
  });

  it("外框沒有對應的定義條目（沒有骨架）時照常展開", () => {
    const frame = { uid: "22", tags: ["maintext", "Status_block"] };
    expect(confirmFrames([frame], [])).toEqual({ frames: [], expand: [frame] });
    expect(confirmFrames([frame], [""])).toEqual({ frames: [], expand: [frame] });
  });
});

describe("composeFrames", () => {
  it("照外框順序組殼：骨架有的容器原樣搬入，唯一沒對上的容器放正文槽", () => {
    const result = composeFrames(STATUS, [{ uid: "22", tags: ["maintext", "Status_block"] }]);
    expect(result).toEqual({
      shell: `<maintext>\n${BODY}\n</maintext>\n${STATUS}`,
      applied: ["22"],
      failed: [],
    });
  });

  it("兩個外框同容器：都套上，正文槽與狀態區塊各只有一份", () => {
    const result = composeFrames(STATUS, [
      { uid: "23", tags: ["maintext", "Status_block"] },
      { uid: "22", tags: ["maintext", "Status_block"] },
    ]);
    expect(result.applied).toEqual(["22", "23"]);
    expect(result.failed).toEqual([]);
    expect(result.shell).toBe(`<maintext>\n${BODY}\n</maintext>\n${STATUS}`);
    expect(result.shell.split(BODY)).toHaveLength(2);
    expect(result.shell.split("<Status_block>")).toHaveLength(2);
  });

  it("外框容器順序與定義骨架不同：照外框順序重排", () => {
    const skeleton = `<maintext>\n${BODY}\n</maintext>\n${STATUS}`;
    const result = composeFrames(skeleton, [{ uid: "22", tags: ["Status_block", "maintext"] }]);
    expect(result.shell).toBe(`${STATUS}\n<maintext>\n${BODY}\n</maintext>`);
    expect(result.applied).toEqual(["22"]);
  });

  it("骨架其餘內容接在外框後面", () => {
    const result = composeFrames(`前言\n${STATUS}\n尾聲`, [{ uid: "22", tags: ["maintext", "Status_block"] }]);
    expect(result.shell).toBe(`<maintext>\n${BODY}\n</maintext>\n${STATUS}\n前言\n\n尾聲`);
  });

  it("組不起來的外框列為失敗、骨架不動：沒對上的容器超過一個、骨架已有正文槽又多一個容器", () => {
    expect(composeFrames(STATUS, [{ uid: "22", tags: ["think", "maintext", "Status_block"] }])).toEqual({
      shell: STATUS,
      applied: [],
      failed: ["22"],
    });
    const withBody = `<content>\n${BODY}\n</content>\n${STATUS}`;
    expect(composeFrames(withBody, [{ uid: "22", tags: ["maintext", "Status_block"] }])).toEqual({
      shell: withBody,
      applied: [],
      failed: ["22"],
    });
  });
});

describe("mergeRefactorInterfaces（外框）", () => {
  it("外框只貢獻容器排法：來源隨介面消耗，不產欄位、規則與指引", () => {
    const merged = mergeRefactorInterfaces([defining()], [{ uid: "22", tags: ["maintext", "Status_block"] }]);
    expect(merged.conflict).toBeNull();
    expect(merged.failedFrames).toEqual([]);
    expect(merged.interface?.source_uids).toEqual(["21", "22"]);
    expect(merged.interface?.shell).toBe(`<maintext>\n${BODY}\n</maintext>\n${STATUS}`);
    expect(merged.interface?.guide).toBe("地點每回合必報");
    expect(merged.interface?.state_fields).toEqual({ 地點: "北境驿站 前院" });
  });

  it("沒有骨架可組時外框全部列為失敗", () => {
    const frame = { uid: "22", tags: ["maintext", "Status_block"] };
    expect(mergeRefactorInterfaces([], [frame]).failedFrames).toEqual(["22"]);
    const conflicting = mergeRefactorInterfaces(
      [defining(), defining({ source_uids: ["24"], state_fields: { 地點: "別處" } })],
      [frame],
    );
    expect(conflicting.conflict).toBe("地點");
    expect(conflicting.failedFrames).toEqual(["22"]);
  });
});
