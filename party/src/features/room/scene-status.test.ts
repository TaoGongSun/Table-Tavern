import { describe, expect, it } from "vitest";
import { setLangOnly } from "../../i18n";
import { mergedSceneStatus, visibleSceneStatus } from "./scene-status";
import type { StoryPart } from "../../../server/core/types";

const part = (state?: StoryPart["state"], n = 0): StoryPart => ({
  round: 1,
  part: n,
  speaker: { kind: "gm" },
  text: "a",
  interrupted: false,
  at: 1,
  ...(state ? { state } : {}),
});

describe("房間狀態列合併", () => {
  it("有鍵就換，空字串清空，沒有的鍵留著；空白不畫", () => {
    setLangOnly("zh-TW");
    const parts = [
      part({ time: "夜", place: "酒館", present: "阿明" }, 0),
      part({ place: "" }, 1),
      part(undefined, 2),
      part({ present: "   " }, 3),
    ];
    expect(mergedSceneStatus(parts)).toEqual({ time: "夜", place: "", present: "   " });
    expect(visibleSceneStatus(parts)).toEqual([{ key: "time", label: "時間", text: "夜" }]);
  });
});

it("panel 整份取代、不深合併；沒有 panel 沿用；收回後重算", () => {
  const old = { characters: { 明: { status: "守門", items: ["鑰匙"] } } };
  const next = { tracking: { clues: ["腳印"] } };
  const parts = [
    { ...part({ time: "夜" }), panel: old },
    part({ place: "酒館" }),
    { ...part(), panel: next },
    part(),
  ];
  expect(mergedSceneStatus(parts)).toEqual({ time: "夜", place: "酒館", panel: next });
  expect(mergedSceneStatus(parts.slice(0, 2)).panel).toEqual(old);
  expect(mergedSceneStatus([...parts, { panel: { characters: {} } }]).panel).toEqual({
    characters: {},
  });
});
