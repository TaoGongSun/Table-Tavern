// 測試用的假 chat_relay：照 Rust `chat_turn::relay::drive`＋`solo_relay` 的規則，把一條接力拆回
// gm_narrate／chat_with_character／append_transcript／read_transcript 幾支子呼叫（交給測試自己的 handler），
// 經 Channel 回報 start／delta／landed／arrived，最後回 RelayReport。前端測試的期待因此照舊寫在子呼叫上。
// 只給測試 import；正式路徑不經這裡。
import type { RelayMessage, RelayReport, TranscriptEvent } from "../../../shared/contracts/backend-contracts";
import { parseMarker } from "../../../shared/ui/event-text";

type Dispatch = (command: string, args: Record<string, unknown>) => Promise<unknown>;
type Card = { name: string; stage?: boolean } | undefined;
type Nominee = { stop: true } | { stop: false; player: boolean; id: string; name: string };

const PLAYER = "__PLAYER__";

/** 那則旁白之後到尾端，是否已有點同一人的點名列（「讓 X 接話」重試時不疊列） */
function calledSince(events: TranscriptEvent[], narrationId: string, name: string): boolean {
  const index = events.findIndex((event) => event.id === narrationId);
  if (index < 0) return false;
  return events.slice(index + 1).some((event) => {
    const marker = parseMarker(event.marker);
    return marker?.type === "gm_call" && marker.name === name;
  });
}
const EMPTY = "AI_EMPTY_RESPONSE: 空白回合不寫進故事";

interface Narration {
  text: string;
  raw?: string | null;
  next?: string | null;
  state_updates?: { path: string; value: string }[];
  arrived_characters?: string[];
  aborted?: boolean;
  truncated?: boolean;
  state_error?: string | null;
}

interface Reply {
  text: string;
  raw?: string | null;
  aborted?: boolean;
  truncated?: boolean;
}

/**
 * dispatch：測試自己的 invoke（子呼叫照樣記進測試的呼叫清單）；cardOf：後端讀卡（沒有＝讀不到）；
 * maxSpeakers：後端讀的 `max_round_speakers`。回傳的 intercept 放在測試 invoke mock 最前面：
 * chat_relay、chat_relay_release 由這裡接走（不記）；chat_abort 記下停止後照常往下。
 */
export function createRelayFake(dispatch: Dispatch, cardOf: (id: string) => Card, maxSpeakers = 3) {
  const relays = new Map<string, { interrupt: boolean; release: boolean }>();

  async function drive(args: Record<string, unknown>): Promise<RelayReport> {
    const worldId = args.worldId as string;
    const relayId = args.relayId as string;
    const mode = args.mode as string;
    const actionId = args.actionId as string | null;
    const playerName = (args.playerName as string) ?? "";
    const channel = args.onEvent as { onmessage: ((message: RelayMessage) => void) | null };
    const send = (message: RelayMessage) => channel.onmessage?.(message);
    const signal = { interrupt: false, release: false };
    relays.set(relayId, signal);
    const halt = () => (signal.interrupt ? "interrupt" : signal.release ? "release" : null);
    const receipts: TranscriptEvent[] = [];
    const stateErrors: string[] = [];
    let aborted = false;
    let owed = false;
    let used = 0;
    type Step = { at: "narrate" } | { at: "call"; card: string } | { at: "reply"; card: string } | { at: "done" };
    let step = (mode === "request" || mode === "nominated" ? { at: "call", card: args.cardId } : { at: "narrate" }) as Step;
    const writeRow = mode === "narrateThenOne" || mode === "advance";
    const oneShot = mode !== "advance";

    const classify = (next: string): Nominee => {
      if (next === PLAYER) return { stop: false, player: true, id: next, name: playerName };
      const card = cardOf(next);
      return !card || card.stage ? { stop: true } : { stop: false, player: false, id: next, name: card.name };
    };
    const stamp = (event: TranscriptEvent): TranscriptEvent => ({ ...event, ...(actionId ? { action_id: actionId } : {}) });
    const append = async (event: TranscriptEvent, extra: Record<string, unknown>, next: string | null = null) => {
      const stamped = stamp(event);
      // 測試沒給 append_transcript 時當後端照收照回
      const landed = ((await dispatch("append_transcript", {
        worldId,
        scene: 0,
        event: stamped,
        turnId: null,
        turnPart: null,
        characterTurn: null,
        ...extra,
      })) ?? stamped) as TranscriptEvent;
      receipts.push(landed);
      send({ type: "landed", event: landed, next });
    };
    const landCall = (name: string) =>
      append({ ts: "ts", speaker_id: "", speaker_name: "GM", kind: "system", text: "", marker: { type: "gm_call", name } }, {});
    const deltas = () => ({ onmessage: (text: string) => send({ type: "delta", text }) });

    const narrate = async () => {
      const turnId = crypto.randomUUID();
      send({ type: "start", turnId, speaker: "", kind: "narration" });
      const piece = (await dispatch("gm_narrate", { worldId, turnId, onDelta: deltas(), actionId })) as Narration;
      if (!piece.aborted && !piece.text.trim()) throw EMPTY;
      if (piece.arrived_characters?.length) send({ type: "arrived", ids: piece.arrived_characters });
      const cut = Boolean(piece.aborted || piece.truncated);
      if (piece.text.trim()) {
        const event: TranscriptEvent = {
          ts: "ts",
          speaker_id: "",
          speaker_name: "GM",
          kind: "narration",
          text: piece.text,
          ...(piece.raw ? { raw: piece.raw } : {}),
          ...(cut ? { truncated: true } : {}),
        };
        for (let attempt = 0; ; attempt += 1) {
          try {
            await append(event, { turnId, turnPart: "main" }, cut ? null : (piece.next ?? null));
            break;
          } catch (reason) {
            if (attempt >= 2 || String(reason).startsWith("AI_EMPTY_RESPONSE")) {
              owed = true;
              throw reason;
            }
          }
        }
      }
      if (!piece.aborted) {
        if (piece.state_error) stateErrors.push(piece.state_error);
        const updates = piece.state_updates ?? [];
        if (updates.length > 0) {
          const text = updates.map((update) => `${update.path}：${update.value}`).join("\n");
          await append(
            { ts: "ts", speaker_id: "", speaker_name: "GM", kind: "system", text, marker: { type: "state_update" } },
            { turnId, turnPart: "state_update" },
          );
        }
      }
      used += 1;
      if (cut || halt() !== null) {
        step = { at: "done" };
        if (piece.aborted || halt() === "interrupt") aborted = true;
        return;
      }
      const nominee = piece.next ? classify(piece.next) : ({ stop: true } as Nominee);
      if (nominee.stop) {
        step = { at: "done" };
        return;
      }
      if (writeRow) await landCall(nominee.name);
      step = nominee.player || mode === "narrate" ? { at: "done" } : { at: "reply", card: nominee.id };
    };

    const call = async (card: string) => {
      const nominee = classify(card);
      if (nominee.stop) {
        step = { at: "done" };
        return;
      }
      const write =
        mode === "nominated" &&
        !calledSince(
          ((await dispatch("read_transcript", { worldId, scene: 0 })) as TranscriptEvent[] | null) ?? [],
          args.narrationId as string,
          nominee.name,
        );
      if (write) await landCall(nominee.name);
      if (halt() !== null) {
        if (halt() === "interrupt") aborted = true;
        step = { at: "done" };
        return;
      }
      step = nominee.player ? { at: "done" } : { at: "reply", card: nominee.id };
    };

    const reply = async (card: string) => {
      const turnId = crypto.randomUUID();
      const nominee = classify(card);
      const name = nominee.stop ? "" : nominee.name;
      send({ type: "start", turnId, speaker: card, kind: "dialogue" });
      const line = (await dispatch("chat_with_character", {
        worldId,
        characterId: card,
        turnId,
        onDelta: deltas(),
        actionId,
      })) as Reply;
      if (!line.aborted && !line.text.trim()) throw EMPTY;
      if (line.text.trim()) {
        const cut = Boolean(line.aborted || line.truncated);
        await append(
          {
            ts: "ts",
            speaker_id: card,
            speaker_name: name,
            kind: "dialogue",
            text: line.text,
            ...(line.raw ? { raw: line.raw } : {}),
            ...(cut ? { truncated: true } : {}),
          },
          { characterTurn: turnId },
        );
      }
      if (line.aborted) aborted = true;
      step = line.aborted || oneShot || classify(card).stop || used >= maxSpeakers ? { at: "done" } : { at: "narrate" };
    };

    try {
      while (step.at !== "done") {
        const halted = halt();
        if (halted === "interrupt") {
          aborted = true;
          break;
        }
        if (halted === "release") break;
        if (step.at === "narrate") {
          if (used >= Math.max(1, maxSpeakers)) break;
          await narrate();
        } else if (step.at === "call") await call(step.card);
        else await reply(step.card);
        if (aborted) break;
      }
      return { aborted, receipts, stateErrors, error: null, resync: false, owed };
    } catch (reason) {
      return { aborted: false, receipts, stateErrors: [], error: String(reason), resync: false, owed };
    } finally {
      relays.delete(relayId);
    }
  }

  return {
    intercept(command: string, args: Record<string, unknown>): Promise<unknown> | undefined {
      if (command === "chat_relay") return drive(args);
      if (command === "chat_relay_release") {
        const signal = relays.get(args.relayId as string);
        if (signal) signal.release = true;
        return Promise.resolve(null);
      }
      if (command === "chat_abort") {
        const signal = relays.get(args.turnId as string);
        if (signal) signal.interrupt = true;
      }
      return undefined;
    },
  };
}
