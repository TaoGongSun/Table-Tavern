import { expect, it } from "vitest";
import { CARDS, Room, playing } from "./room-test-kit";
import { cleanDicePool, cleanHistory } from "./validate";
const cards = CARDS.map((card, index) => ({
  ...card,
  claimedBy: null,
  name: index === 0 ? "沃恩" : "雷克",
}));
const pool = "d20:[19,3]";
const history = (dicePool: unknown) => ({
  speaker: { kind: "gm" },
  text: "沃恩 (d20#1=19)",
  interrupted: false,
  at: 1,
  dicePool,
});

it("池只驗長度且只給 GM，超限不影響故事", () => {
  expect(cleanDicePool(pool)).toBe(pool);
  expect(cleanDicePool("獸".repeat(171))).toBeUndefined();
  expect(cleanDicePool({ faces: [] })).toBeUndefined();
  expect(
    cleanHistory([history(pool), history("x".repeat(513))], cards)?.map((p) => p.dicePool),
  ).toEqual([pool, undefined]);
  const npc = {
    ...history(pool),
    speaker: { kind: "npc", cardId: CARDS[0].cardId, name: CARDS[0].name },
  };
  expect(cleanHistory([npc], cards)?.[0].dicePool).toBeUndefined();
});
it("即時段與 seed／sync 保留同一池", async () => {
  const room = await playing();
  await room.send("a", "round.resolve", { round: 2 });
  const dispatchId = room.dispatchId();
  await room.send("x1", "dispatch.ack", { dispatchId });
  await room.send("x1", "gen.part.begin", { dispatchId, part: 0, speaker: { kind: "gm" } });
  await room.send("x1", "gen.part.end", {
    dispatchId,
    part: 0,
    text: "沃恩 (d20#1=19)",
    interrupted: false,
    dicePool: pool,
  });
  expect(room.events("story.part").at(-1)?.payload.dicePool).toBe(pool);
  const seeded = await Room.create();
  await seeded.exec();
  await seeded.send("x1", "room.seed", { part: 0, total: 1, cards, history: [history(pool)] });
  expect(seeded.events("story.part")[0].payload.dicePool).toBe(pool);
  await seeded.join("a", "a", { pairCode: await seeded.pairCode(true) });
  await seeded.send("a", "seat.claim", { cardId: CARDS[0].cardId });
  await seeded.send("a", "room.start");
  await seeded.finishGen("開場");
  await seeded.send("x1", "status", { keepAwake: null, busy: false, paused: true });
  await seeded.disconnect("x1");
  await seeded.exec("x2");
  const reply = await seeded.ask("x2", "story.sync", {
    part: 0,
    total: 1,
    keepThrough: null,
    items: [{ part: history(pool) }],
  });
  expect(reply?.t).toBe("ok");
  expect(seeded.events("story.part").at(-1)?.payload.dicePool).toBe(pool);
});

it("keeps a solo public roll when reopening a room", () => {
  const roll = {
    ...history(pool),
    speaker: { kind: "system", marker: "dice_roll" },
    text: "沃恩 1d20: [19] = 19",
  };
  expect(cleanHistory([roll], cards)?.[0]).toMatchObject({
    text: roll.text,
    speaker: roll.speaker,
  });
});
