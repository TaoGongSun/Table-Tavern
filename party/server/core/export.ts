// 有損匯出成網頁存檔契約 v1（協定 §9.3、src/shared/contracts/web-save/web-save.md）：只帶文字，不帶卡片原檔與成員資料。
import { keptCount, undoneRounds, type KeepThrough } from "./snapshot";
import type { ActionItem, RoomState, Speaker, StoredEvent } from "./types";

type PublicSpeaker = Speaker extends infer S
  ? S extends { memberId: string }
    ? Omit<S, "memberId">
    : S
  : never;
interface Scene {
  scene: number;
  summaryMessageId: string | null;
  firstMessageId: string | null;
}

export function toWebSave(s: RoomState, events: StoredEvent[], now: number) {
  const messages: Array<{
    id: string;
    role: "user" | "char";
    text: string;
    ts: string;
    interrupted?: true;
  }> = [];
  const speakers: Record<string, PublicSpeaker> = {};
  const scenes: Scene[] = [{ scene: 1, summaryMessageId: null, firstMessageId: null }];
  const players = new Set(s.cards.filter((c) => c.claimedBy).map((c) => c.cardId));
  let firstGm: string | null = null;
  // 重開補送撤段用：每段（含不輸出的系統段）的 (回合, 段號) 與到它為止的訊息數
  const marks: Array<{ round: number; part: number; n: number }> = [];
  let from = 0; // 本幕第一段在 marks 裡的位置
  const mark = (round: number, part: number) => marks.push({ round, part, n: messages.length });
  const push = (
    id: string,
    role: "user" | "char",
    text: string,
    at: number,
    speaker: PublicSpeaker,
    interrupted = false,
  ) => {
    messages.push({
      id,
      role,
      text,
      ts: new Date(at).toISOString(),
      ...(interrupted ? { interrupted: true as const } : {}),
    });
    speakers[id] = speaker;
    scenes[scenes.length - 1].firstMessageId ??= id;
  };
  const undone = undoneRounds(events); // 房主收回的回合不輸出
  for (const { type, at, payload: p } of events) {
    if (undone.has(p.round as number) && (type === "story.part" || type === "round.resolved"))
      continue;
    if (type === "round.resolved") {
      (p.batch as ActionItem[]).forEach((a, i) => {
        players.add(a.cardId);
        if (a.kind !== "act") return;
        push(`r${p.round}a${i}`, "user", `${a.name}：${a.text}`, at, {
          kind: "player",
          cardId: a.cardId,
          name: a.name,
        });
        mark(p.round as number, -1 - i);
      });
    } else if (type === "story.part") {
      const sp = p.speaker as Speaker;
      if (sp.kind === "system") {
        mark(p.round as number, p.part as number);
        continue; // 觀望與系統段不輸出
      }
      if (sp.kind === "gm") firstGm ??= (p.text as string) || null;
      const pubSpeaker =
        sp.kind === "player" ? { kind: sp.kind, cardId: sp.cardId, name: sp.name } : sp;
      push(
        `r${p.round}p${p.part}`,
        sp.kind === "player" ? "user" : "char",
        sp.kind === "gm" ? (p.text as string) : `${sp.name}：${p.text}`,
        at,
        pubSpeaker,
        p.interrupted === true,
      );
      mark(p.round as number, p.part as number);
    } else if (type === "scene.summary") {
      push(`s${p.scene}`, "char", p.text as string, at, {
        kind: "system",
        marker: "scene_summary",
      });
      mark(p.round as number, 32);
      scenes[scenes.length - 1].summaryMessageId = `s${p.scene}`;
    } else if (type === "round.undone")
      mark(p.round as number, 33); // 收回提示：快照有這一段，補送可能保留到它
    else if (type === "scene.started") {
      scenes.push({ scene: p.scene as number, summaryMessageId: null, firstMessageId: null });
      from = marks.length;
    } else if (type === "story.truncated") {
      const k = keptCount(marks, p.after as KeepThrough, from);
      for (const m of messages.splice(k > 0 ? marks[k - 1].n : 0)) delete speakers[m.id];
      marks.length = k;
      const scene = scenes[scenes.length - 1];
      scene.firstMessageId =
        scene.firstMessageId && speakers[scene.firstMessageId] ? scene.firstMessageId : null;
    }
  }
  return {
    format: "table-tavern-web-save",
    version: 1,
    exported_at: new Date(now).toISOString(),
    card: {
      spec: "chara_card_v2",
      spec_version: "2.0",
      data: { name: "GM", first_mes: firstGm ?? "（多人房間紀錄）" },
    },
    import_route: "character",
    regex_allowed: true,
    user_name: s.members.find((m) => m.id === s.hostId)?.nickname ?? "",
    opening_index: null,
    messages,
    world_info: { entries: [], timed: {}, last_message_id: null, message_effects: {} },
    mvu: null,
    card_storage: {},
    extensions: {
      table_tavern_room: {
        version: 1,
        scenes,
        speakers,
        cast: s.cards.map((c) => ({
          cardId: c.cardId,
          name: c.name,
          player: players.has(c.cardId),
        })),
      },
    },
  };
}
