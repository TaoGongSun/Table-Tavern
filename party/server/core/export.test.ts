// 有損匯出（協定 §9.3）過網頁存檔契約的兩個 TS 入口：共用契約的 parseWebSave 與網頁版接著玩的 restoreWebSaveText。
import { parseWebSave } from "@desktop/shared/contracts/web-save/web-save";
import { restoreWebSaveText } from "@web/features/saves/web-save-codec";
import { describe, expect, it } from "vitest";
import { playing } from "./room-test-kit";

describe("匯出成網頁存檔", () => {
  it("房主取得契約 v1 JSON：行動→user、GM→char、NPC 加卡名、幕摘要、觀望與系統段不輸出；兩端解析都收", async () => {
    const room = await playing();
    await room.send("b", "action.submit", { round: 2, text: "我推門" });
    await room.send("a", "action.skip", { round: 2 });
    const dispatchId = room.dispatchId();
    const part = (n: number, speaker: unknown, text: string, interrupted = false) =>
      room
        .send("x1", "gen.part.begin", { dispatchId, part: n, speaker })
        .then(() => room.send("x1", "gen.part.end", { dispatchId, part: n, text, interrupted }));
    await part(0, { kind: "gm" }, "門開了");
    await part(1, { kind: "npc", cardId: "npc", name: "店主" }, "歡迎", true);
    await part(2, { kind: "system", marker: "state_update" }, "狀態");
    await room.send("x1", "gen.done", { dispatchId, aborted: false });
    await room.send("a", "scene.advance", { round: 3 });
    const { jobId } = room.frames("x1", "job.scene_advance")[0].p;
    await room.send("x1", "job.result", { jobId, ok: true, result: { summary: "第一幕結束" } });

    const deny = await room.run({ kind: "http", route: "export", deviceId: room.deviceId("b") });
    expect(deny?.http?.status).toBe(403);
    const out = await room.run({ kind: "http", route: "export", deviceId: room.deviceId("a") });
    const save = out!.http!.body as Record<string, any>;
    expect(
      save.messages.map((m: { id: string; role: string; text: string }) => [m.id, m.role, m.text]),
    ).toEqual([
      ["r1p0", "char", "開場"],
      ["r2a1", "user", "角色2：我推門"],
      ["r2p0", "char", "門開了"],
      ["r2p1", "char", "店主：歡迎"],
      ["s1", "char", "第一幕結束"],
    ]);
    expect(save.messages[3].interrupted).toBe(true);
    expect(save.card).toEqual({
      spec: "chara_card_v2",
      spec_version: "2.0",
      data: { name: "GM", first_mes: "開場" },
    });
    expect(save.user_name).toBe("a");
    expect(save.extensions.table_tavern_room).toMatchObject({
      version: 1,
      scenes: [
        { scene: 1, firstMessageId: "r1p0", summaryMessageId: "s1" },
        { scene: 2, firstMessageId: null, summaryMessageId: null },
      ],
      speakers: {
        r2a1: { kind: "player", cardId: "c2", name: "角色2" },
        r2p1: { kind: "npc", cardId: "npc", name: "店主" },
      },
    });
    expect(save.extensions.table_tavern_room.cast).toContainEqual({
      cardId: "c1",
      name: "角色1",
      player: true,
    });
    const text = JSON.stringify(save);
    expect(text).not.toMatch(/memberId|m1|m2|dev-/);

    const parsed = parseWebSave(text);
    expect(parsed.ok).toBe(true);
    const restored = restoreWebSaveText(text);
    expect(restored.ok && restored.game.entries.map((e) => e.role)).toEqual([
      "char",
      "user",
      "char",
      "char",
      "char",
    ]);
  });
});
