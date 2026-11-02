// 有損匯出成網頁存檔契約 v1（協定 §9.3、src/shared/contracts/web-save/web-save.md）：只帶文字，不帶卡片原檔與成員資料。
import type { ActionItem, RoomState, Speaker, StoredEvent } from "./types";

type PublicSpeaker = Speaker extends infer S ? (S extends { memberId: string } ? Omit<S, "memberId"> : S) : never;
interface Scene { scene: number; summaryMessageId: string | null; firstMessageId: string | null }

export function toWebSave(s: RoomState, events: StoredEvent[], now: number) {
  const messages: Array<{ id: string; role: "user" | "char"; text: string; ts: string; interrupted?: true }> = [];
  const speakers: Record<string, PublicSpeaker> = {};
  const scenes: Scene[] = [{ scene: 1, summaryMessageId: null, firstMessageId: null }];
  const players = new Set(s.cards.filter((c) => c.claimedBy).map((c) => c.cardId));
  let firstGm: string | null = null;
  const push = (id: string, role: "user" | "char", text: string, at: number, speaker: PublicSpeaker, interrupted = false) => {
    messages.push({ id, role, text, ts: new Date(at).toISOString(), ...(interrupted ? { interrupted: true as const } : {}) });
    speakers[id] = speaker;
    scenes[scenes.length - 1].firstMessageId ??= id;
  };
  for (const { type, at, payload: p } of events) {
    if (type === "round.resolved") {
      (p.batch as ActionItem[]).forEach((a, i) => {
        players.add(a.cardId);
        if (a.kind === "act") push(`r${p.round}a${i}`, "user", `${a.name}：${a.text}`, at, { kind: "player", cardId: a.cardId, name: a.name });
      });
    } else if (type === "story.part") {
      const sp = p.speaker as Speaker;
      if (sp.kind === "system") continue; // 觀望與系統段不輸出
      if (sp.kind === "gm") firstGm ??= (p.text as string) || null;
      const pubSpeaker = sp.kind === "player" ? { kind: sp.kind, cardId: sp.cardId, name: sp.name } : sp;
      push(`r${p.round}p${p.part}`, sp.kind === "player" ? "user" : "char", sp.kind === "gm" ? (p.text as string) : `${sp.name}：${p.text}`, at, pubSpeaker, p.interrupted === true);
    } else if (type === "scene.summary") {
      push(`s${p.scene}`, "char", p.text as string, at, { kind: "system", marker: "scene_summary" });
      scenes[scenes.length - 1].summaryMessageId = `s${p.scene}`;
    } else if (type === "scene.started") scenes.push({ scene: p.scene as number, summaryMessageId: null, firstMessageId: null });
  }
  return {
    format: "table-tavern-web-save",
    version: 1,
    exported_at: new Date(now).toISOString(),
    card: { spec: "chara_card_v2", spec_version: "2.0", data: { name: "GM", first_mes: firstGm ?? "（多人房間紀錄）" } },
    import_route: "character",
    regex_allowed: true,
    user_name: s.members.find((m) => m.id === s.hostId)?.nickname ?? "",
    opening_index: null,
    messages,
    world_info: { entries: [], timed: {}, last_message_id: null, message_effects: {} },
    mvu: null,
    card_storage: {},
    extensions: { table_tavern_room: { version: 1, scenes, speakers, cast: s.cards.map((c) => ({ cardId: c.cardId, name: c.name, player: players.has(c.cardId) })) } },
  };
}
