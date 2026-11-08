// 房間玩家自己擲骰（room-player-dice）：伺服器擲、擲果公開不作廢、下一次派工整批帶走，順序與桌面一致。
import { describe, expect, it } from "vitest";
import { applyFrame, mergeStory, type RoomView } from "../../src/features/room/room-state";
import { parseFormula, rollText } from "./dice";
import { toWebSave } from "./export";
import { CARDS, playing, Room, STAGE } from "./room-test-kit";
import { buildSnapshot, keptCount } from "./snapshot";
import type { Frame, StoryPart } from "./types";

const LIVE = { stream: null, seed: null, buckets: {}, uploads: {}, cardReqs: {} };
const story = (room: Room, viewer = "m2") =>
  buildSnapshot(room.s, room.store.events, viewer, LIVE).story;
const rows = (parts: StoryPart[]) => parts.filter((p) => p.rollBy).map((p) => p.text);
const on = (room: Room, id = "x1") =>
  room.send(id, "status", { keepAwake: null, busy: false, diceEnabled: true });
const roll = (room: Room, who: string, formula = "1d20", submit?: boolean) =>
  room.ask(who, "action.roll", {
    round: room.s.round!.round,
    formula,
    ...(submit ? { submit } : {}),
  });
const dispatches = (room: Room, id = "x1") => room.frames(id, "dispatch").map((f) => f.p);
const lastDispatch = (room: Room, id = "x1") => dispatches(room, id).at(-1);
const clientStory = (room: Room, id: string) =>
  (room.out[id] ?? []).reduce<RoomView | null>(
    (v, f) => (f.t === "welcome" && v ? v : applyFrame(v, f as Frame)),
    null,
  )!.story;
/** 回應收回工作：桌面把那批刪乾淨了 */
async function undo(room: Room, round: number, mode: "edit" | "regenerate") {
  await room.send("a", "round.undo", { round, mode });
  const job = room.frames("x1", "job.undo").at(-1)!.p;
  await room.send("x1", "job.result", { jobId: job.jobId, ok: true });
}
async function dicePlaying() {
  const room = await playing();
  await on(room);
  return room;
}
/** 第一輪：a 擲 A、兩人送出、GM 跑完。回傳那回合號與 A 的文字 */
async function rolledRound(room: Room) {
  const round = room.s.round!.round;
  await roll(room, "a", "1d20");
  const a = room.s.pendingRolls![0].text;
  await room.send("a", "action.submit", { round, text: "我拔劍" });
  await room.send("b", "action.skip", { round });
  await room.finishGen("GM 接著說");
  return { round, a };
}

describe("算式", () => {
  it("嚴格解析，範圍同單人", () => {
    for (const bad of ["0d6", "101d6", "1d1", "1d1001", "1d6+10000", "2d20kh1", "1d6+1d4", "abc"])
      expect(parseFormula(bad), bad).toBeNull();
    expect(parseFormula("x".repeat(129))).toBeNull();
    expect(parseFormula(" D6 ")).toEqual({ count: 1, sides: 6, modifier: 0 });
    expect(parseFormula("100d1000 - 9999")).toEqual({ count: 100, sides: 1000, modifier: -9999 });
  });

  it("文字同單人格式；拒絕取樣丟掉尾端的值", () => {
    let n = 0;
    // 第一次取到 2^32-1（6 面會被拒），之後是 3 → 面值 4
    const rng = () => new Uint8Array(n++ === 0 ? [255, 255, 255, 255] : [0, 0, 0, 3]);
    expect(rollText("沃恩", parseFormula("2d6+3")!, rng)).toBe("沃恩 2d6+3: [4, 4] +3 = 11");
    expect(rollText("沃恩", parseFormula("1d6-1")!, rng)).toBe("沃恩 1d6-1: [4] -1 = 3");
    expect(rollText("雷克", parseFormula("d20")!, rng)).toBe("雷克 1d20: [4] = 4");
  });
});

describe("action.roll", () => {
  it("非法算式、沒座位、已送出、交給 GM、超次數、關擲骰都拒，狀態不動", async () => {
    const room = await playing();
    expect((await roll(room, "a"))?.p.detail).toBe("dice_disabled");
    await on(room);
    expect((await roll(room, "a", "1d1"))?.p.code).toBe("E_TEXT_INVALID");
    await room.join("c");
    expect((await roll(room, "c"))?.p.code).toBe("E_SEAT_REQUIRED");
    expect((await room.ask("a", "action.roll", { round: 1, formula: "1d6" }))?.p.code).toBe(
      "E_ROUND_STALE",
    );
    await room.send("b", "action.skip", { round: room.s.round!.round });
    expect((await roll(room, "b"))?.p.code).toBe("E_STATE_CONFLICT");
    expect(room.s.pendingRolls ?? []).toEqual([]);
    for (let i = 0; i < 8; i++) {
      room.clock += 10_000; // 過頻率窗
      expect((await roll(room, "a"))?.t).toBe("ok");
    }
    room.clock += 10_000;
    expect((await roll(room, "a"))?.p.code).toBe("E_STATE_CONFLICT");
    await room.send("a", "action.submit", { round: room.s.round!.round, text: "好" });
    expect((await roll(room, "a"))?.t).toBe("err");
  });

  it("限頻：10 秒內第 6 次被擋", async () => {
    const room = await dicePlaying();
    for (let i = 0; i < 5; i++) expect((await roll(room, "a"))?.t).toBe("ok");
    expect((await roll(room, "a"))?.p.code).toBe("E_RATE_LIMITED");
  });

  it("面值在範圍內；其他人立刻在故事串看到，名字取自卡名、段號從 -100 起", async () => {
    const room = await dicePlaying();
    await roll(room, "a", "3d8+2");
    const row = clientStory(room, "b").at(-1)!;
    expect(row).toMatchObject({
      part: -100,
      rollBy: "m1",
      speaker: { kind: "system", marker: "dice_roll" },
    });
    const [, faces] = /^角色1 3d8\+2: \[(.+)\] \+2 = \d+$/.exec(row.text)!;
    expect(faces.split(", ").every((f) => +f >= 1 && +f <= 8)).toBe(true);
  });

  it("submit：同一筆交易標成已送出、文字空；全員完成就結算，空文字不畫列、不留玩家句", async () => {
    const room = await dicePlaying();
    const round = room.s.round!.round;
    await roll(room, "a", "1d20", true);
    expect(room.s.round!.roster[0]).toMatchObject({ status: "submitted", text: "" });
    await room.send("b", "action.submit", { round, text: "我推門" });
    expect(room.s.round!.phase).toBe("queued");
    const d = lastDispatch(room);
    expect(d.actions[0]).toMatchObject({ memberId: "m1", kind: "act", text: "" });
    expect(d.rolls).toHaveLength(1);
    const texts = story(room).filter((p) => p.round === round);
    expect(texts.map((p) => p.part).sort()).toEqual([-100, -2]);
    const saved = toWebSave(room.s, room.store.events, room.clock);
    expect(saved.messages.some((m) => m.text.startsWith("角色1："))).toBe(false); // 空文字不留玩家句
    // 想補字走既有的收回修改：不留空草稿
    const next = await dicePlaying();
    await roll(next, "a", "1d6", true);
    await next.send("a", "action.retract", { round: next.s.round!.round });
    expect(next.s.round!.roster[0]).toMatchObject({ status: "pending", text: null });
    expect(next.s.drafts.m1).toBeUndefined();
  });

  it("換座：待落的擲果跟著卡走", async () => {
    const room = await dicePlaying();
    await room.join("c");
    await roll(room, "b");
    await room.send("a", "member.reassign", { from: "m2", to: "m3" });
    expect(room.s.pendingRolls![0].memberId).toBe("m3");
  });
});

describe("擲果不作廢", () => {
  it("收回行動後還在，進下一次派工；派工之後待落清空，不重複帶", async () => {
    const room = await dicePlaying();
    const round = room.s.round!.round;
    await room.send("a", "action.submit", { round, text: "我拔劍" });
    await room.send("a", "action.retract", { round });
    await roll(room, "b");
    const text = room.s.pendingRolls![0].text;
    await room.send("a", "action.submit", { round, text: "我拔劍" });
    await room.send("b", "action.submit", { round, text: "我推門" });
    expect(lastDispatch(room).rolls.map((r: any) => r.text)).toEqual([text]);
    expect(room.s.pendingRolls).toEqual([]);
    await room.finishGen();
    await room.send("a", "action.skip", { round: room.s.round!.round });
    await room.send("b", "action.skip", { round: room.s.round!.round });
    expect(lastDispatch(room).rolls).toEqual([]);
  });

  it("房主 GM 鈕帶走擲果；收回那個 GM 回合放回待落，列不動、不重複", async () => {
    const room = await dicePlaying();
    const before = room.s.round!.round;
    await roll(room, "b");
    const text = room.s.pendingRolls![0].text;
    await room.send("a", "host.gm", { round: before, mode: "narrate" });
    const host = lastDispatch(room);
    expect(host.rolls.map((r: any) => r.text)).toEqual([text]);
    await room.finishGen("旁白");
    await undo(room, host.round, "edit");
    expect(room.s.pendingRolls!.map((r) => r.text)).toEqual([text]);
    expect(rows(story(room))).toEqual([text]);
    await room.send("a", "action.skip", { round: room.s.round!.round });
    await room.send("b", "action.skip", { round: room.s.round!.round });
    expect(lastDispatch(room).rolls.map((r: any) => r.text)).toEqual([text]);
  });

  it("收回：A（被收回的回合）、B（下一回合待落），故事串與下一次派工都是 A 在前", async () => {
    const room = await dicePlaying();
    const { round, a } = await rolledRound(room);
    await roll(room, "b", "1d6");
    const b = room.s.pendingRolls![0].text;
    await undo(room, round, "edit");
    expect(room.s.pendingRolls!.map((r) => r.text)).toEqual([a, b]);
    for (const parts of [story(room), clientStory(room, "b")]) expect(rows(parts)).toEqual([a, b]);
    expect(story(room).some((p) => p.text === "我拔劍")).toBe(false); // 行動列照舊撤掉
    expect(story(room).at(-1)!.part).toBe(33);
    const next = room.s.round!.round;
    await room.send("a", "action.submit", { round: next, text: "重來" });
    await room.send("b", "action.skip", { round: next });
    expect(lastDispatch(room).rolls.map((r: any) => r.text)).toEqual([a, b]);
    expect(rows(story(room))).toEqual([a, b]);
    expect(room.s.pendingRolls).toEqual([]);
  });

  it("重生：新派工＝原擲果＋待落（原的在前），列不消失也不重複", async () => {
    const room = await dicePlaying();
    const { round, a } = await rolledRound(room);
    await roll(room, "b", "1d6");
    const b = room.s.pendingRolls![0].text;
    await undo(room, round, "regenerate");
    const d = lastDispatch(room);
    expect(d.rolls.map((r: any) => r.text)).toEqual([a, b]);
    expect(d.actions[0].text).toBe("我拔劍");
    expect(room.s.pendingRolls).toEqual([]);
    for (const parts of [story(room), clientStory(room, "b")]) expect(rows(parts)).toEqual([a, b]);
  });

  it("收回失敗：擲果不放回", async () => {
    const room = await dicePlaying();
    const { round } = await rolledRound(room);
    await room.send("a", "round.undo", { round, mode: "edit" });
    const job = room.frames("x1", "job.undo").at(-1)!.p;
    await room.send("x1", "job.result", { jobId: job.jobId, ok: false });
    expect(room.s.pendingRolls).toEqual([]);
  });

  it("換幕清空：列留在前一幕，不進 GM", async () => {
    const room = await dicePlaying();
    await roll(room, "b");
    await room.send("a", "scene.advance", { round: room.s.round!.round });
    expect(room.s.pendingRolls).toHaveLength(1); // 換幕失敗會回同一幕，還要留著
    const job = room.frames("x1", "job.scene_advance").at(-1)!.p;
    await room.send("x1", "job.result", {
      jobId: job.jobId,
      ok: true,
      result: { summary: "摘要" },
    });
    expect(room.s.pendingRolls).toEqual([]);
    expect(lastDispatch(room).rolls).toEqual([]);
  });

  it("舊快照沒有這些欄位照常：派工帶空清單、收回不出錯", async () => {
    const room = await playing();
    delete room.store.state!.pendingRolls;
    const round = room.s.round!.round;
    await room.send("a", "action.skip", { round });
    await room.send("b", "action.skip", { round });
    const d = lastDispatch(room);
    expect(d.rolls).toEqual([]);
    await room.finishGen();
    delete room.store.state!.lastDispatch!.rolls;
    await undo(room, round, "edit");
    expect(room.s.pendingRolls).toEqual([]);
  });
});

describe("段號與投影", () => {
  it("多人多骰加上觀望與行動：不撞號，mergeStory、keptCount、匯出都對得上", async () => {
    const room = await dicePlaying();
    const round = room.s.round!.round;
    await roll(room, "a");
    await roll(room, "b");
    await roll(room, "a", "2d6");
    await room.send("a", "action.submit", { round, text: "我拔劍" });
    await room.send("a", "round.resolve", { round });
    const parts = story(room).filter((p) => p.round === round);
    expect(parts.map((p) => p.part)).toEqual([-100, -101, -102, -1, -2]);
    expect(new Set(parts.map((p) => `${p.round}:${p.part}`)).size).toBe(parts.length);
    expect(mergeStory(parts, parts)).toHaveLength(parts.length);
    expect(clientStory(room, "b").filter((p) => p.round === round)).toEqual(parts);
    expect(keptCount(parts, { round, part: -102 }, 0)).toBe(3);
    const saved = toWebSave(room.s, room.store.events, room.clock);
    expect(saved.messages.filter((m) => m.id.startsWith(`r${round}`)).map((m) => m.id)).toEqual([
      `r${round}p-100`,
      `r${round}p-101`,
      `r${round}p-102`,
      `r${round}a0`,
    ]);
  });

  it("匯出：擲骰→結算→GM 後看得到擲果，帶 system／dice_roll 說話者、順序同故事串；收回也還在", async () => {
    const room = await dicePlaying();
    const { round, a } = await rolledRound(room);
    const out = () => toWebSave(room.s, room.store.events, room.clock);
    const saved = out();
    expect(saved.messages.slice(-3).map((m) => [m.id, m.role, m.text])).toEqual([
      [`r${round}p-100`, "char", a],
      [`r${round}a0`, "user", "角色1：我拔劍"],
      [`r${round}p0`, "char", "GM 接著說"],
    ]);
    expect(saved.extensions.table_tavern_room.speakers[`r${round}p-100`]).toEqual({
      kind: "system",
      marker: "dice_roll",
    });
    expect(JSON.stringify(saved)).not.toContain('"m1"');
    await undo(room, round, "edit");
    const texts = out().messages.map((m) => m.text);
    expect(texts.filter((t) => t === a)).toHaveLength(1);
    expect(texts).not.toContain("GM 接著說");
    // 走真正的匯出入口（reducer 的 HTTP 路徑）結果相同
    const http = await room.run({ kind: "http", route: "export", deviceId: room.deviceId("a") });
    expect((http!.http!.body as { messages: unknown[] }).messages).toEqual(out().messages);
  });

  it("開關：status 帶 diceEnabled，立刻推給客戶端，快照也有；舊版不報＝關", async () => {
    const room = await playing();
    await on(room);
    expect(room.frames("b", "vol", "executor").at(-1)?.p.diceEnabled).toBe(true);
    await room.send("x1", "status", { keepAwake: null, busy: false });
    expect(room.frames("b", "vol", "executor").at(-1)?.p.diceEnabled).toBe(false);
    expect(buildSnapshot(room.s, room.store.events, "m2", LIVE).executor.diceEnabled).toBe(false);
  });
});

describe("重開補送（§3.3）", () => {
  const gm = (text: string) => ({
    part: { speaker: { kind: "gm" }, text, interrupted: false, at: 0 },
  });
  /** 送過種子的房間：開場跑完、b 擲一顆（待落）、房主關房 */
  async function paused(rollFirst = true) {
    const room = await Room.create();
    await room.exec();
    await room.send("x1", "room.seed", {
      part: 0,
      total: 1,
      cards: [...CARDS, STAGE],
      history: [gm("種子開場").part],
    });
    for (const [i, id] of ["a", "b"].entries()) {
      await room.join(id, id, i === 0 ? { pairCode: await room.pairCode(true) } : {});
      await room.send(id, "seat.claim", { cardId: CARDS[i].cardId });
    }
    await on(room);
    await room.send("a", "room.start");
    await room.finishGen("開場");
    if (rollFirst) await roll(room, "b");
    await room.send("x1", "status", { keepAwake: null, busy: false, paused: true });
    await room.disconnect("x1");
    return room;
  }
  const sync = (room: Room, p: Record<string, unknown>) =>
    room.ask("x2", "story.sync", { part: 0, total: 1, ...p });
  const execStory = (room: Room) => room.frames("x2", "welcome")[0].p.snapshot.story as StoryPart[];
  const at = (p: StoryPart) => ({ round: p.round, part: p.part });

  it("待落中重開、沒有補送：執行端快照沒有那列、不截斷，回合不動", async () => {
    const room = await paused();
    const round = room.s.round!.round;
    await room.exec("x2");
    const seen = execStory(room);
    expect(rows(seen)).toEqual([]);
    await sync(room, { keepThrough: at(seen.at(-1)!) });
    expect(room.events("story.truncated")).toEqual([]);
    expect(room.s.round!.round).toBe(round);
    expect(rows(story(room))).toHaveLength(1);
    expect(room.s.pendingRolls).toHaveLength(1);
  });

  it("待落中重開、有補送：補送在前、擲果在後，段號不變；手機與快照同順序", async () => {
    const room = await paused();
    const [roll0] = room.s.pendingRolls!;
    await room.exec("x2");
    const seen = execStory(room);
    await sync(room, { keepThrough: at(seen.at(-1)!), items: [gm("單人玩的一段")] });
    const after = story(room);
    expect(after.map((p) => p.text).slice(-2)).toEqual(["單人玩的一段", roll0.text]);
    expect(after.at(-1)).toMatchObject({ round: roll0.round, part: roll0.part });
    expect(clientStory(room, "b").map((p) => p.text)).toEqual(after.map((p) => p.text));
    expect(
      toWebSave(room.s, room.store.events, room.clock)
        .messages.map((m) => m.text)
        .slice(-2),
    ).toEqual(["單人玩的一段", roll0.text]);
    // 下一次派工的順序也相同：桌面落成「補送、擲果」
    const round = room.s.round!.round;
    await room.send("a", "action.skip", { round });
    await room.send("b", "action.skip", { round });
    expect(lastDispatch(room, "x2").rolls.map((r: any) => r.text)).toEqual([roll0.text]);
  });

  it("待落中重開、確實要截斷：撤段後擲果照樣留著", async () => {
    const room = await paused();
    const [roll0] = room.s.pendingRolls!;
    await room.exec("x2");
    await sync(room, { keepThrough: at(execStory(room)[0]), items: [gm("改走別條路")] });
    const after = story(room);
    expect(after.some((p) => p.text === "開場")).toBe(false);
    expect(after.map((p) => p.text).slice(-2)).toEqual(["改走別條路", roll0.text]);
  });

  it("待落列排在 keepThrough 前（收回提示）：錨點退回，重發不重複", async () => {
    const room = await paused(false);
    await room.exec("x2");
    await sync(room, { keepThrough: at(execStory(room).at(-1)!) });
    await on(room, "x2");
    const round = room.s.round!.round;
    await roll(room, "a");
    const a = room.s.pendingRolls![0].text;
    await room.send("a", "action.skip", { round });
    await room.send("b", "action.skip", { round });
    await room.finishGen("接著", "x2");
    await room.send("a", "round.undo", { round, mode: "edit" });
    const job = room.frames("x2", "job.undo").at(-1)!.p;
    await room.send("x2", "job.result", { jobId: job.jobId, ok: true });
    expect(story(room).at(-1)!.part).toBe(33);
    await room.send("x2", "status", {
      keepAwake: null,
      busy: false,
      paused: true,
      diceEnabled: true,
    });
    await room.disconnect("x2");
    await room.connect("x3");
    await room.ask("x3", "hello", {
      protocol: 1,
      role: "executor",
      roomCode: room.code,
      executorToken: room.executorToken,
      lastSeq: 0,
      inflight: null,
      client: { name: "desktop", version: "0" },
    });
    const seen = room.frames("x3", "welcome")[0].p.snapshot.story as StoryPart[];
    expect(rows(seen)).toEqual([]);
    await room.ask("x3", "story.sync", {
      part: 0,
      total: 1,
      keepThrough: at(seen.at(-1)!),
      items: [gm("補送段")],
    });
    const after = story(room);
    expect(rows(after)).toEqual([a]);
    expect(after.at(-1)!.text).toBe(a);
    expect(clientStory(room, "b").filter((p) => p.text === a)).toHaveLength(1);
  });

  it("匯出：補送進來的 story.part 擲果也輸出；截斷撤掉的不留，重發的不重複", async () => {
    const room = await paused();
    const [roll0] = room.s.pendingRolls!;
    await room.exec("x2");
    const solo = {
      part: {
        speaker: { kind: "system", marker: "dice_roll" },
        text: "角色1 1d6: [3] = 3",
        interrupted: false,
        at: 0,
      },
    };
    await sync(room, { keepThrough: at(execStory(room).at(-1)!), items: [solo, gm("單人一段")] });
    const texts = () =>
      toWebSave(room.s, room.store.events, room.clock).messages.map((m) => m.text);
    expect(texts().slice(-3)).toEqual(["角色1 1d6: [3] = 3", "單人一段", roll0.text]);
    expect(texts().filter((t) => t === roll0.text)).toHaveLength(1);
    await room.send("x2", "status", {
      keepAwake: null,
      busy: false,
      paused: true,
      diceEnabled: true,
    });
    await room.disconnect("x2");
    await room.connect("x3");
    await room.ask("x3", "hello", {
      protocol: 1,
      role: "executor",
      roomCode: room.code,
      executorToken: room.executorToken,
      lastSeq: 0,
      inflight: null,
      client: { name: "desktop", version: "0" },
    });
    const seen = room.frames("x3", "welcome")[0].p.snapshot.story as StoryPart[];
    const anchor = seen.find((p) => p.text === "種子開場")!;
    await room.ask("x3", "story.sync", { part: 0, total: 1, keepThrough: at(anchor) });
    expect(texts()).not.toContain("角色1 1d6: [3] = 3");
    expect(texts().filter((t) => t === roll0.text)).toHaveLength(1);
  });

  it("補送裡換過幕：擲果留在舊幕，待落清空", async () => {
    const room = await paused();
    const [roll0] = room.s.pendingRolls!;
    await room.exec("x2");
    await sync(room, {
      keepThrough: at(execStory(room).at(-1)!),
      items: [gm("單人收尾"), { summary: "上一幕摘要" }],
    });
    expect(room.s.pendingRolls).toEqual([]);
    const texts = story(room).map((p) => p.text);
    expect(texts.filter((t) => t === roll0.text)).toHaveLength(1);
    expect(texts.indexOf("單人收尾")).toBeLessThan(texts.indexOf(roll0.text));
    expect(texts.indexOf(roll0.text)).toBeLessThan(texts.indexOf("上一幕摘要"));
  });

  it("擲後關房重開：已落地的擲果在快照裡（要跟桌面比對），待落才濾", async () => {
    const room = await paused();
    const [roll0] = room.s.pendingRolls!;
    await room.exec("x2");
    await sync(room, { keepThrough: at(execStory(room).at(-1)!) });
    const round = room.s.round!.round;
    await room.send("a", "action.skip", { round });
    await room.send("b", "action.skip", { round });
    await room.finishGen("接著", "x2");
    await room.send("x2", "status", { keepAwake: null, busy: false, paused: true });
    await room.disconnect("x2");
    await room.connect("x3");
    await room.ask("x3", "hello", {
      protocol: 1,
      role: "executor",
      roomCode: room.code,
      executorToken: room.executorToken,
      lastSeq: 0,
      inflight: null,
      client: { name: "desktop", version: "0" },
    });
    expect(rows(room.frames("x3", "welcome")[0].p.snapshot.story)).toEqual([roll0.text]);
  });
});
