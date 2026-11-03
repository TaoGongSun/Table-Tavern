import { describe, expect, it } from "vitest";
import type { SceneLabel } from "../../../shared/contracts/backend-contracts";
import { nextScene, previousScene } from "./replay-nav";

const label = (patch: Partial<SceneLabel>): SceneLabel => ({ base: 0, version: 1, parent: null, ...patch });

describe("replay scene links", () => {
  // 0 換幕→1；從 0 分岔→2（parent 也記 0）；2 換幕→3
  const labels = {
    "1": label({ parent: 0 }),
    "2": label({ parent: 0, forked: true, source: 0 }),
    "3": label({ parent: 2 }),
  };

  it("the next scene is the scene-change child, never a fork", () => {
    expect(nextScene(labels, 0)).toBe(1);
    expect(nextScene(labels, 2)).toBe(3);
    expect(nextScene(labels, 3)).toBeNull();
  });

  it("the previous scene follows a fork's source and a scene change's parent", () => {
    expect(previousScene(labels, 2)).toBe(0);
    expect(previousScene(labels, 3)).toBe(2);
    expect(previousScene(labels, 0)).toBeNull();
  });
});
