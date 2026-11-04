import { describe, expect, it } from "vitest";
import { CARDS, playing } from "./room-test-kit";
import { cleanHistory, cleanSceneState } from "./validate";

const cards = CARDS.map((card) => ({ ...card, claimedBy: null }));

const gm = (state: unknown) => ({
  speaker: { kind: "gm" },
  text: "夜了",
  interrupted: false,
  at: 1,
  state,
});

describe("cleanSceneState", () => {
  it("只留三個字串鍵；非字串或超過 200 字丟掉該鍵；不是物件整份丟掉；空字串留著", () => {
    expect(cleanSceneState({ time: "夜", place: "", present: "阿明", extra: "x" })).toEqual({
      time: "夜",
      place: "",
      present: "阿明",
    });
    expect(cleanSceneState({ time: "字".repeat(201), place: 1, present: "在" })).toEqual({
      present: "在",
    });
    expect(cleanSceneState({ time: "   " })).toEqual({ time: "   " });
    expect(cleanSceneState({})).toBeNull();
    expect(cleanSceneState(["夜"])).toBeNull();
    expect(cleanSceneState(null)).toBeNull();
    expect(cleanHistory([gm([])], cards)?.[0].state).toBeUndefined();
    expect(cleanHistory([gm({ time: "" })], cards)?.[0]).toMatchObject({
      text: "夜了",
      state: { time: "" },
    });
  });
});

describe("gen.part.end 的 state", () => {
  it("落段只帶洗過的三鍵；陣列整份丟掉，旁白仍在", async () => {
    const room = await playing();
    await room.send("a", "round.resolve", { round: 2 });
    const dispatchId = room.dispatchId();
    await room.send("x1", "dispatch.ack", { dispatchId });
    await room.send("x1", "gen.part.begin", { dispatchId, part: 0, speaker: { kind: "gm" } });
    await room.send("x1", "gen.part.end", {
      dispatchId,
      part: 0,
      text: "夜了",
      interrupted: false,
      state: {
        time: "夜",
        place: "",
        present: "阿明",
        extra: "no",
        bad: 1,
        long: "字".repeat(201),
      },
    });
    expect(room.events("story.part").at(-1)?.payload.state).toEqual({
      time: "夜",
      place: "",
      present: "阿明",
    });

    await room.send("x1", "gen.part.begin", { dispatchId, part: 1, speaker: { kind: "gm" } });
    await room.send("x1", "gen.part.end", {
      dispatchId,
      part: 1,
      text: "仍在",
      interrupted: false,
      state: ["夜"],
    });
    const dropped = room.events("story.part").at(-1)?.payload;
    expect(dropped?.text).toBe("仍在");
    expect(dropped?.state).toBeUndefined();
  });
});
