// VN 播放器的純函式：攤平格、可播判定、正式化對位、舞台摺疊、auto 延遲、換一組錯誤碼。
// 規則見 .ai/plans/vn-mode-pkg2.md §2–§4。
import type {
  Backdrop,
  FlagHint,
  TranscriptEvent,
  VnCell,
  VnEventCells,
} from "../../shared/contracts/backend-contracts";

/** 對白格裡玩家的 speaker_id（Rust `PLAYER_SENTINEL`）：主角不上舞台，只出名牌 */
export const PLAYER_ID = "__PLAYER__";

export interface FlatCell {
  /** 正式格＝`<事件 id>#<格序>`；暫定格＝`pending:<串流世代>#<格序>` */
  key: string;
  /** 正式格的事件鍵（舊事件沒 id 用位置代）；暫定格為 null */
  eventKey: string | null;
  index: number;
  kind: TranscriptEvent["kind"] | "pending";
  cell: VnCell;
  /** 這則事件的格邊旗標提示（同一則的格共用） */
  hints?: FlagHint[];
  /** 這則事件快照的章號 */
  chapter?: number;
  /** 換幕摘要的格：不播、只進 log */
  recap?: boolean;
}

export interface Cursor {
  key: string;
  /** 這格已打出的字數（以字元計，不切開代理對） */
  shown: number;
}

export const charLen = (text: string) => [...text].length;
export const sliceChars = (text: string, n: number) => [...text].slice(0, n).join("");

export function cellText(cell: VnCell): string {
  return cell.type === "narration" || cell.type === "dialogue" ? cell.text : "";
}

export const eventKeyOf = (event: VnEventCells, position: number) =>
  event.event_id ?? `@${position}`;

export function flatten(events: VnEventCells[]): FlatCell[] {
  return events.flatMap((event, position) => {
    const eventKey = eventKeyOf(event, position);
    return event.cells.map((cell, index) => ({
      key: `${eventKey}#${index}`,
      eventKey,
      index,
      kind: event.kind,
      cell,
      hints: event.hints,
      chapter: event.chapter,
      recap: event.recap,
    }));
  });
}

export function pendingCells(generation: number, cells: VnCell[]): FlatCell[] {
  return cells.map((cell, index) => ({
    key: `pending:${generation}#${index}`,
    eventKey: null,
    index,
    kind: "pending" as const,
    cell,
  }));
}

/** 終止格：選項、章末、結局（游標停在它們上面時對話框留著前一格文字） */
export const isTerminal = (cell: VnCell) =>
  cell.type === "choices" || cell.type === "chapter" || cell.type === "ending";

/**
 * 游標可停的格：文字格（玩家句與換幕摘要除外，只進 log）＋仍有效的正式終止格（選項、章末、結局）。
 * 終止格只在它屬於本幕最後一則正式事件時有效；之後有玩家句或任何新事件＝已消耗。串流暫定格裡的終止標記
 * 可能在提交時被剝掉，不當停點。`[bg]` 不停（舞台照摺疊）。
 */
export function isPlayable(cell: FlatCell, lastEventKey: string | null): boolean {
  if (cell.kind === "player" || cell.recap) return false;
  if (isTerminal(cell.cell)) return cell.kind !== "pending" && cell.eventKey === lastEventKey;
  return cellText(cell.cell) !== "";
}

/** 進桌位置：最後一則非玩家、非摘要正式事件的第一個可停格；還沒有正式事件（空桌第一回合）就從第一格播 */
export function entryKey(cells: FlatCell[], playable: FlatCell[]): string | null {
  const last = [...cells]
    .reverse()
    .find((cell) => cell.kind !== "player" && cell.kind !== "pending" && !cell.recap);
  const first = last && playable.find((cell) => cell.eventKey === last.eventKey);
  return (first ?? lastFormal(playable) ?? playable[0])?.key ?? null;
}

/** 最後一個正式可停格：游標鍵不見時退到這裡，不跳進還在長的暫定格 */
export const lastFormal = (playable: FlatCell[]) =>
  [...playable].reverse().find((cell) => cell.kind !== "pending");

/**
 * 暫定格換成正式格時的游標對位：以「該事件已顯示文字長度」在正式格上找累積長度跨過它的那格。
 * 落在格界時停在前一格文字格的結尾，不跳到其後的零字格。
 */
export function realignCursor(
  tentative: VnCell[],
  cursor: { index: number; shown: number },
  formal: VnCell[],
): { index: number; shown: number } {
  const before = tentative
    .slice(0, cursor.index)
    .reduce((sum, cell) => sum + charLen(cellText(cell)), 0);
  const current = tentative[cursor.index];
  const consumed = before + Math.min(cursor.shown, current ? charLen(cellText(current)) : 0);
  let acc = 0;
  let last: { index: number; shown: number } | null = null;
  for (const [index, cell] of formal.entries()) {
    const length = charLen(cellText(cell));
    if (length === 0) continue;
    if (acc + length >= consumed) return { index, shown: consumed - acc };
    acc += length;
    last = { index, shown: length };
  }
  return last ?? { index: 0, shown: 0 };
}

export interface Stage {
  /** 槽位：[左, 右, 中]，值是角色 id */
  slots: (string | null)[];
  names: Record<string, string>;
  /** 每位角色最近一句的表情，沿用到下次說話 */
  expressions: Record<string, string>;
  /** 每位角色目前的 `@變體`，沿用到下次切換（換地點清場也保留） */
  variants: Record<string, string>;
  speaker: string | null;
  directive: "closeup" | "shake" | null;
  location: string | null;
  time: string | null;
  /** 目前的背景鍵 */
  backdrop: Backdrop | null;
}

/** 進場順序：首位左、次位右、第三位中 */
const SLOT_ORDER = [0, 1, 2];

/** 從幕首摺疊到 `index`（含）：站位照進場順序，第四位進場時最久沒說話的人讓位；換地點清場 */
export function stageAt(cells: FlatCell[], index: number): Stage {
  const stage: Stage = {
    slots: [null, null, null],
    names: {},
    expressions: {},
    variants: {},
    speaker: null,
    directive: null,
    location: null,
    time: null,
    backdrop: null,
  };
  let recent: string[] = [];
  for (const { cell } of cells.slice(0, index + 1)) {
    if (cell.type === "bg") {
      if (cell.location !== stage.location) {
        stage.slots = [null, null, null];
        recent = [];
      }
      stage.location = cell.location;
      stage.time = cell.time;
      // 正式格的時段後端已補好沿用值；串流暫定格只有明寫的，沒寫就沿用目前的
      const period = cell.period ?? stage.backdrop?.period ?? "day";
      stage.backdrop = { location: cell.location, slug: cell.slug, period };
      continue;
    }
    if (cell.type !== "dialogue" || !cell.speaker_id || cell.speaker_id === PLAYER_ID) continue;
    const id = cell.speaker_id;
    stage.names[id] = cell.speaker_name;
    stage.expressions[id] = cell.expression;
    if (cell.variant) stage.variants[id] = cell.variant;
    if (!stage.slots.includes(id)) {
      const free = SLOT_ORDER.find((slot) => stage.slots[slot] === null);
      const idle = recent.find((who) => stage.slots.includes(who));
      stage.slots[free ?? stage.slots.indexOf(idle ?? null)] = id;
    }
    recent = [...recent.filter((who) => who !== id), id];
  }
  const current = cells[index]?.cell;
  if (current?.type === "dialogue" && current.speaker_id && stage.slots.includes(current.speaker_id)) {
    stage.speaker = current.speaker_id;
    stage.directive = current.directive;
  } else if (current?.type === "dialogue") {
    stage.directive = current.directive === "shake" ? "shake" : null;
  }
  return stage;
}

/** auto：本格打完後等 1200 + 40×字數 ms，上限 6000 */
export const autoDelay = (text: string) => Math.min(6000, 1200 + 40 * charLen(text));

export type RerollError = "busy" | "stale" | "failed" | "unavailable";

/** `vn_reroll_choices` 的錯誤字串是 `VN_REROLL_<碼>: 說明`（invoke 可能包一層 `Error: `） */
export function rerollError(raw: unknown): RerollError | null {
  const match = /^(?:Error:\s*)?VN_REROLL_(BUSY|STALE|FAILED|UNAVAILABLE):/.exec(String(raw));
  return match ? (match[1].toLowerCase() as RerollError) : null;
}
