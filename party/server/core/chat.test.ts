// 桌邊聊天：只送成員連線、快照與 live 依收件人過濾、執行端兩條 welcome 都是空的。
import { describe, expect, it } from "vitest";
import { toWebSave } from "./export";
import { Room } from "./room-test-kit";
import { buildSnapshot, liveValues } from "./snapshot";
import type { Live } from "./types";

const LIVE: Live = { stream: null, seed: null, buckets: {}, uploads: {}, cardReqs: {} };
const texts = (frames: { p: { msg?: { text: string } } }[]) => frames.map((f) => f.p.msg!.text);

async function lobby() {
  const room = await Room.create();
  await room.exec();
  await room.join("a", "阿明", { pairCode: await room.pairCode(true) });
  await room.join("b", "小華");
  await room.join("c", "阿凱");
  return room;
}

const helloExec = (room: Room, lastSeq: number) =>
  room.ask("x1", "hello", {
    protocol: 1,
    role: "executor",
    roomCode: room.code,
    executorToken: room.executorToken,
    lastSeq,
    inflight: null,
    client: { name: "desktop", version: "0" },
  });

describe("桌邊聊天", () => {
  it("公開訊息未入座也能發、全員收到、執行端收不到，而且不進事件", async () => {
    const room = await lobby();
    const reply = await room.ask("b", "chat.send", { text: "還沒入座也能說" });
    expect(reply?.p.result).toEqual({ id: 1 });
    expect(room.s.chat).toEqual([
      { id: 1, at: room.clock, from: "m2", to: null, text: "還沒入座也能說" },
    ]);
    for (const id of ["a", "b", "c"])
      expect(texts(room.frames(id, "vol", "chat"))).toEqual(["還沒入座也能說"]);
    expect(room.frames("x1", "vol", "chat")).toEqual([]);
    expect(JSON.stringify(room.store.events)).not.toContain("還沒入座也能說");
    expect(room.s.status).toBe("lobby");
  });

  it("私訊只給雙方；快照與 live 依收件人過濾", async () => {
    const room = await lobby();
    await room.send("b", "chat.send", { text: "公開的" });
    await room.send("a", "chat.send", { text: "只給小華", to: "m2" });
    expect(texts(room.frames("a", "vol", "chat"))).toEqual(["公開的", "只給小華"]);
    expect(texts(room.frames("b", "vol", "chat"))).toEqual(["公開的", "只給小華"]);
    expect(texts(room.frames("c", "vol", "chat"))).toEqual(["公開的"]);
    expect(room.frames("x1", "vol", "chat")).toEqual([]);
    const see = (viewer: string) =>
      buildSnapshot(room.s, room.store.events, viewer, LIVE).chat.map((m) => m.text);
    expect(see("m1")).toEqual(["公開的", "只給小華"]);
    expect(see("m3")).toEqual(["公開的"]);
    expect(see("exec")).toEqual([]);
    expect(liveValues(room.s, LIVE, "m2").chat.map((m) => m.text)).toEqual(["公開的", "只給小華"]);
    expect(liveValues(room.s, LIVE, "m3").chat.map((m) => m.text)).toEqual(["公開的"]);
    expect(liveValues(room.s, LIVE, "exec").chat).toEqual([]);
    await room.disconnect("c");
    await room.connect("c");
    await room.ask("c", "hello", {
      protocol: 1,
      role: "member",
      deviceId: room.deviceId("c"),
      lastSeq: room.s.seq,
      client: { name: "party", version: "0" },
    });
    expect(
      room
        .frames("c", "welcome")
        .at(-1)!
        .p.live.chat.map((m: { text: string }) => m.text),
    ).toEqual(["公開的"]);
  });

  it("執行端首次 welcome 與補事件重連都拿不到聊天", async () => {
    const room = await lobby();
    await room.send("a", "chat.send", { text: "執行端不該看到" });
    await room.disconnect("x1");
    await room.connect("x1");
    await helloExec(room, 0);
    expect(room.frames("x1", "welcome").at(-1)!.p.snapshot.chat).toEqual([]);
    await room.disconnect("x1");
    await room.connect("x1");
    await helloExec(room, room.s.seq);
    const welcome = room.frames("x1", "welcome").at(-1)!;
    expect(welcome.p.snapshot).toBeUndefined();
    expect(welcome.p.live.chat).toEqual([]);
  });

  it("超過 200 則從最舊丟；中文訊息把 UTF-8 JSON 壓回 64 KiB", async () => {
    const room = await lobby();
    for (let i = 0; i < 201; i++) {
      if (i % 10 === 0 && i) room.clock += 10_000;
      await room.send("a", "chat.send", { text: `n${i}` });
    }
    expect(room.s.chat).toHaveLength(200);
    expect(room.s.chat![0].text).toBe("n1");
    expect(room.s.chat!.at(-1)!.text).toBe("n200");

    const text = "測".repeat(500);
    let sent = 0;
    for (; sent < 80; sent++) {
      if (sent % 10 === 0) room.clock += 10_000;
      expect((await room.ask("a", "chat.send", { text }))?.t).toBe("ok");
    }
    const bytes = new TextEncoder().encode(JSON.stringify(room.s.chat)).length;
    expect(room.s.chat!.length).toBeLessThan(sent);
    expect(room.s.chat!.length).toBeLessThanOrEqual(200);
    expect(bytes).toBeLessThanOrEqual(64 * 1024);
    expect(room.s.chat!.at(-1)!.text).toBe(text);
  });

  it("每人 10 秒 10 則；to 給自己、不存在或 null 被拒", async () => {
    const room = await lobby();
    for (let i = 0; i < 10; i++)
      expect((await room.ask("a", "chat.send", { text: `m${i}` }))?.t).toBe("ok");
    expect((await room.ask("a", "chat.send", { text: "太多" }))?.p.code).toBe("E_RATE_LIMITED");
    expect(room.s.chat).toHaveLength(10);
    room.clock += 10_000;
    expect((await room.ask("a", "chat.send", { text: "窗口過了" }))?.t).toBe("ok");
    expect((await room.ask("a", "chat.send", { text: "自己", to: "m1" }))?.p.code).toBe(
      "E_STATE_CONFLICT",
    );
    expect((await room.ask("a", "chat.send", { text: "沒有這人", to: "nope" }))?.p.code).toBe(
      "E_STATE_CONFLICT",
    );
    expect((await room.ask("a", "chat.send", { text: "" }))?.p.code).toBe("E_TEXT_INVALID");
    expect((await room.ask("a", "chat.send", { text: "測".repeat(501) }))?.p.code).toBe(
      "E_TEXT_INVALID",
    );
    const kept = room.s.chat!.length;
    await room.send("a", "chat.send", { text: "空的", to: null });
    expect(room.frames("a", "err").at(-1)?.p.code).toBe("E_BAD_FRAME");
    expect(room.closed.a).toBe(4400);
    expect(room.s.chat).toHaveLength(kept);
    expect(room.s.chat!.at(-1)!.text).toBe("窗口過了");
  });

  it("關房後被拒；匯出不含聊天；舊狀態沒有 chat 欄位時照常", async () => {
    const room = await lobby();
    expect(room.s.chat).toBeUndefined();
    expect(buildSnapshot(room.s, room.store.events, "m1", LIVE).chat).toEqual([]);
    expect(liveValues(room.s, LIVE, "m1").chat).toEqual([]);
    expect((await room.ask("a", "chat.send", { text: "別寫進故事" }))?.p.result).toEqual({ id: 1 });
    expect(JSON.stringify(toWebSave(room.s, room.store.events, room.clock))).not.toContain(
      "別寫進故事",
    );

    await room.send("a", "room.close");
    await room.connect("b2");
    await room.ask("b2", "hello", {
      protocol: 1,
      role: "member",
      deviceId: room.deviceId("b"),
      lastSeq: 0,
      client: { name: "party", version: "0" },
    });
    expect((await room.ask("b2", "chat.send", { text: "關了" }))?.p.code).toBe("E_ROOM_CLOSED");
  });
});
