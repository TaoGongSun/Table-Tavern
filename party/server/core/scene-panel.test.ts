import { describe, expect, it } from "vitest";
import { CARDS, Room, playing } from "./room-test-kit";
import { cleanScenePanel } from "./validate";

const raw = {
  characters: { 明: { status: "守門", items: ["鑰匙"], secret: "no" } },
  tracking: { goals: ["回家"], promises: [], clues: ["腳印"], extra: [] },
  extra: "no",
};
const panel = {
  characters: { 明: { status: "守門", items: ["鑰匙"] } },
  tracking: { goals: ["回家"], promises: [], clues: ["腳印"] },
};
const history = (value: unknown) => ({
  speaker: { kind: "gm" },
  text: "夜了",
  interrupted: false,
  at: 1,
  panel: value,
});

describe("cleanScenePanel", () => {
  it("只收已知欄；空字串與空清單可以清空；非物件與全錯型不附", () => {
    expect(cleanScenePanel(raw)).toEqual(panel);
    for (const v of [
      null,
      [],
      "x",
      1,
      {},
      { characters: [] },
      { characters: { 明: null } },
      { characters: { 明: { status: 1, items: ["x", 2] } }, tracking: { goals: [null] } },
    ])
      expect(cleanScenePanel(v)).toBeNull();
    expect(
      cleanScenePanel({ characters: { 明: { status: "", items: [] } }, tracking: { clues: [] } }),
    ).toEqual({ characters: { 明: { status: "", items: [] } }, tracking: { clues: [] } });
    expect(cleanScenePanel({ characters: {} })).toEqual({ characters: {} });
    expect(
      cleanScenePanel({
        characters: { 明: { status: "ok", items: [1] } },
        tracking: { goals: 1, clues: [] },
      }),
    ).toEqual({ characters: { 明: { status: "ok" } }, tracking: { clues: [] } });
  });

  it("12 格、64 字鍵、60 字狀態、8×24 持有物、三欄各 6×48；字數按 Unicode 字元", () => {
    const cell = { status: "🐕".repeat(61), items: Array(9).fill("🐕".repeat(25)) };
    const value = cleanScenePanel({
      characters: Object.fromEntries([
        ["名".repeat(65), cell],
        ["名".repeat(64), cell],
        ...Array.from({ length: 13 }, (_, i) => [`人${i}`, cell]),
      ]),
      tracking: Object.fromEntries(
        ["goals", "promises", "clues"].map((k) => [k, Array(7).fill("🐕".repeat(49))]),
      ),
    })!;
    expect(Object.keys(value.characters!)).toHaveLength(12);
    expect(value.characters!["名".repeat(65)]).toBeUndefined();
    expect(value.characters!["名".repeat(64)]).toEqual({
      status: "🐕".repeat(60),
      items: Array(8).fill("🐕".repeat(24)),
    });
    for (const entries of Object.values(value.tracking!))
      expect(entries).toEqual(Array(6).fill("🐕".repeat(48)));
    const special = cleanScenePanel(JSON.parse('{"characters":{"__proto__":{"status":"ok"}}}'))!;
    expect(Object.keys(special.characters!)).toEqual(["__proto__"]);
  });
});

it("gen.part.end 廣播洗過的 panel，錯型不影響故事", async () => {
  const room = await playing();
  await room.send("a", "round.resolve", { round: 2 });
  const dispatchId = room.dispatchId();
  await room.send("x1", "dispatch.ack", { dispatchId });
  for (const [part, value] of [raw, []].entries()) {
    await room.send("x1", "gen.part.begin", { dispatchId, part, speaker: { kind: "gm" } });
    await room.send("x1", "gen.part.end", {
      dispatchId,
      part,
      text: "夜了",
      interrupted: false,
      panel: value,
    });
  }
  const parts = room.events("story.part").slice(-2);
  expect(parts[0].payload.panel).toEqual(panel);
  expect(parts[1].payload.panel).toBeUndefined();
  expect(parts[1].payload.text).toBe("夜了");
});

it("room.seed 與 story.sync 都走相同 panel 驗法並落地", async () => {
  const room = await Room.create();
  await room.exec();
  await room.send("x1", "room.seed", {
    part: 0,
    total: 1,
    cards: CARDS,
    history: [history(raw), history([])],
  });
  let parts = room.events("story.part");
  expect(parts[0].payload.panel).toEqual(panel);
  expect(parts[1].payload.panel).toBeUndefined();
  await room.join("a", "a", { pairCode: await room.pairCode(true) });
  await room.send("a", "seat.claim", { cardId: CARDS[0].cardId });
  await room.send("a", "room.start");
  await room.finishGen("開場");
  await room.send("x1", "status", { keepAwake: null, busy: false, paused: true });
  await room.disconnect("x1");
  await room.exec("x2");
  expect(room.s.exec?.sync).toBe("needed");
  const reply = await room.ask("x2", "story.sync", {
    part: 0,
    total: 1,
    keepThrough: null,
    items: [{ part: history(raw) }, { part: history({ tracking: { clues: [1] } }) }],
  });
  parts = room.events("story.part").slice(-2);
  expect(parts[0].payload.panel).toEqual(panel);
  expect(parts[1].payload.panel).toBeUndefined();
  expect(reply?.t).toBe("ok");
  expect(room.s.exec?.sync).toBeNull();
});
