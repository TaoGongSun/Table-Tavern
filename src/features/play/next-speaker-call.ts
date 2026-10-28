// 「讓 X 接話」：單次 GM 旁白點了誰、那顆鈕現在該不該掛在那則旁白下面（純推導，不存失效狀態）。
import { TranscriptEvent } from "../../shared/contracts/backend-contracts";
import { parseMarker } from "../../shared/ui/event-text";

/** GM 正文落檔當下回報：id＝落檔那則（世代已變時沒有）；next＝可接力的點名（停止或截斷的旁白給 null） */
export interface Landed {
  id: string | undefined;
  next: string | null;
}

/** 單次旁白記下的點名：哪一則旁白點了哪個角色 */
export interface NominatedCall {
  narrationId: string;
  characterId: string;
}

/** 最後一則故事事件（玩家句、對白、旁白；系統列不算）的 id */
export function lastStoryId(events: TranscriptEvent[]): string | undefined {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    if (events[index].kind !== "system") return events[index].id;
  }
  return undefined;
}

/**
 * 鈕掛在哪一則（index），不掛回 null：那則旁白仍是最後一則故事事件、被點的角色還在側欄主區。
 * 玩家再發言、別的回覆、收回旁白、角色封存或隱藏都自然失效；角色接完話後那句對白被收回，鈕就回來。
 */
export function callableAt(
  events: TranscriptEvent[],
  call: NominatedCall | null,
  castIds: readonly string[],
): number | null {
  if (!call || !castIds.includes(call.characterId)) return null;
  if (lastStoryId(events) !== call.narrationId) return null;
  const index = events.findIndex((event) => event.id === call.narrationId);
  return index < 0 ? null : index;
}

/** 那則旁白之後到尾端，是否已有點同一人的點名列（「讓 X 接話」重試時不疊列） */
export function calledSince(events: TranscriptEvent[], narrationId: string, name: string): boolean {
  const index = events.findIndex((event) => event.id === narrationId);
  if (index < 0) return false;
  return events.slice(index + 1).some((event) => {
    const marker = parseMarker(event.marker);
    return marker?.type === "gm_call" && marker.name === name;
  });
}
