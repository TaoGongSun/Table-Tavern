// 收回／復原疊（undo-two-clicks-summary-line）：一下收一樓（後端回傳的那一組），復原整組放回。
// 全 app 只有一疊，記著它屬於哪桌哪幕：換到別桌別幕時不顯示（比對不上），換回來照常可復原；
// 收回／放回在途中換桌，疊照發起當時那桌那幕更新——後端已經寫成的事，疊一定要記上。
import { TranscriptEvent } from "../../../shared/contracts/backend-contracts";
import { parseMarker } from "../../../shared/ui/event-text";

/** 有本文，或帶已知標頭代碼（點名事件本文本來就空）才算有內容；空白回合不落地、不放回 */
export function hasContent(event: TranscriptEvent): boolean {
  return event.text.trim() !== "" || parseMarker(event.marker) !== null;
}

export interface UndoStack {
  worldId: string;
  scene: number;
  /** 每次收回的那一樓（原序），後收的疊在最上面 */
  groups: TranscriptEvent[][];
}

export function pushUndone(
  previous: UndoStack | null,
  worldId: string,
  scene: number,
  group: TranscriptEvent[],
): UndoStack {
  return previous && previous.worldId === worldId && previous.scene === scene
    ? { ...previous, groups: [...previous.groups, group] }
    : { worldId, scene, groups: [group] };
}

/** 疊頂往下第一組有內容的位置；不是這桌這幕或全是空白事件（AI 失敗年代留下的）就 -1 */
export function restorableIndex(stack: UndoStack | null, worldId: string, scene: number): number {
  if (!stack || stack.worldId !== worldId || stack.scene !== scene) return -1;
  let index = stack.groups.length - 1;
  while (index >= 0 && !stack.groups[index].some(hasContent)) index -= 1;
  return index;
}

/** 放回後：這組與疊在上面的空白組一起消耗。疊已換成別桌別幕（或被新內容清掉）就不動 */
export function consumeFrom(
  stack: UndoStack | null,
  worldId: string,
  scene: number,
  index: number,
): UndoStack | null {
  if (!stack || stack.worldId !== worldId || stack.scene !== scene) return stack;
  return index > 0 ? { ...stack, groups: stack.groups.slice(0, index) } : null;
}

/** 送回後端的那一組：空白事件不放回；表以 JSON 文字送（Tauri 參數會把物件鍵排序，文字才保得住原順序） */
export function restorePayload(group: TranscriptEvent[]) {
  return group
    .filter(hasContent)
    .map((event) =>
      event.message_vars ? { ...event, message_vars: JSON.stringify(event.message_vars) } : event,
    );
}

function sameEvent(shown: TranscriptEvent, stored: TranscriptEvent): boolean {
  return shown.id !== undefined && stored.id !== undefined
    ? shown.id === stored.id
    : shown.ts === stored.ts && shown.kind === stored.kind && shown.text === stored.text;
}

/** 畫面尾端就是後端剛收掉的那幾則：直接切掉；對不上（畫面落後磁碟）就要重讀 */
export function tailMatches(shown: TranscriptEvent[], removed: TranscriptEvent[]): boolean {
  if (removed.length > shown.length) return false;
  const tail = shown.slice(shown.length - removed.length);
  return tail.every((event, index) => sameEvent(event, removed[index]));
}
