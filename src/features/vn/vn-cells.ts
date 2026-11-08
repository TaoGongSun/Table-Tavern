// VN 播放器的純函式：攤平格、可播判定、正式化對位、舞台摺疊、auto 延遲、換一組錯誤碼。
// 規則見 .ai/plans/vn-mode-pkg2.md §2–§4。
import type {
  Backdrop,
  FlagHint,
  TranscriptEvent,
  VnCell,
  VnChoiceMark,
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
  /** 這則是選項點（後端照共用選項邊界算）：已選＝answered 有字 */
  choice?: VnChoiceMark;
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
      choice: event.choice,
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
 * 游標可停的格：文字格（玩家句與換幕摘要除外，只進 log）＋正式終止格。串流暫定格裡的終止標記可能在提交時
 * 被剝掉，不當停點。`[bg]`、`[bgm]`、`[sfx]` 不停（舞台照摺疊）。
 * - 選項格看後端的選項點標記（包 6 §2.2）：已選過的可停（過去的選項面板）；未選的只在它是本幕最後一則
 *   正式事件時停，後面還有角色線就等播完再出面板（`liveChoice`）；不是選項點（被取代）不停。
 * - 章末、結局只在屬於本幕最後一則正式事件時有效。
 */
export function isPlayable(cell: FlatCell, lastEventKey: string | null): boolean {
  if (cell.kind === "player" || cell.recap) return false;
  if (!isTerminal(cell.cell)) return cellText(cell.cell) !== "";
  if (cell.kind === "pending") return false;
  if (cell.cell.type === "choices" && cell.choice?.answered) return true;
  if (cell.cell.type === "choices" && !cell.choice) return false;
  return cell.eventKey === lastEventKey;
}

/** 本幕還沒選的選項格（正式格）：玩家句之後才算選過，夾在中間的角色線播完照樣出面板 */
export const liveChoice = (cells: FlatCell[]) =>
  [...cells]
    .reverse()
    .find((cell) => cell.kind !== "pending" && cell.cell.type === "choices" && cell.choice?.answered === null);

/** 沒有閱讀位置與指定位置時的進桌位置：幕首第一個可停格（沒讀過就從頭播）；還沒有正式格（空桌第一回合）就是第一個暫定格 */
export const entryKey = (playable: FlatCell[]): string | null =>
  (playable.find((cell) => cell.kind !== "pending") ?? playable[0])?.key ?? null;

/**
 * 進新幕的指定位置（包 6、包 8 §2.6）：last＝最後一個正式格；cell＝那一格；start＝幕首第一格；
 * after＝那則事件（玩家句不可停）之後第一個可停格；lastChoice＝最後一個選過的選項格。找不到回 undefined。
 */
export type EntryAt =
  | { kind: "last" }
  | { kind: "cell"; key: string }
  | { kind: "start" }
  | { kind: "after"; eventKey: string }
  | { kind: "lastChoice" };

export function entryCell(cells: FlatCell[], playable: FlatCell[], at: EntryAt): FlatCell | undefined {
  switch (at.kind) {
    case "last":
      return lastFormal(playable);
    case "cell":
      return playable.find((cell) => cell.key === at.key);
    case "start":
      return playable.find((cell) => cell.kind !== "pending");
    case "lastChoice":
      return [...playable].reverse().find((cell) => cell.cell.type === "choices" && cell.choice?.answered);
    case "after": {
      const from = cells.findIndex((cell) => cell.eventKey === at.eventKey);
      if (from < 0) return undefined;
      return cells.slice(from).find((cell) => cell.eventKey !== at.eventKey && playable.includes(cell));
    }
  }
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
  /** 目前天氣 clear／overcast／fog／rain */
  weather: string;
  /** null＝還沒寫過配樂（播主題曲）；空陣列＝靜音 */
  bgm: string[] | null;
}

/** 進場順序：首位左、次位右、第三位中 */
const SLOT_ORDER = [0, 1, 2];

/** 從幕首摺疊到 `index`（含）：站位照進場順序，第四位進場時最久沒說話的人讓位；換地點清場 */
export function stageAt(cells: FlatCell[], index: number, trailingExits = false): Stage {
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
    weather: "clear",
    bgm: null,
  };
  let recent: string[] = [];
  // 最後文字格讀完後也處理尾端退場；不提早套用背景、音樂或改動閱讀游標。
  for (const [i, { cell }] of cells.slice(0, trailingExits ? undefined : index + 1).entries()) {
    if (i > index && cell.type !== "exit") continue;
    if (cell.type === "exit") {
      stage.slots = stage.slots.map((id) => id === cell.character_id ? null : id);
      recent = recent.filter((id) => id !== cell.character_id);
      continue;
    }
    if (cell.type === "bg") {
      if (cell.location !== stage.location) {
        stage.slots = [null, null, null];
        recent = [];
      }
      stage.location = cell.location;
      stage.time = cell.time;
      // 正式格的時段後端已補好沿用值；串流暫定格只有明寫的，沒寫就沿用目前的
      const period = cell.period ?? stage.backdrop?.period ?? "day";
      // 天氣同理：暫定格沒寫＝同地點沿用、換地點回晴
      stage.weather = cell.weather ?? (stage.backdrop?.slug === cell.slug ? stage.weather : "clear");
      stage.backdrop = { location: cell.location, slug: cell.slug, period };
      continue;
    }
    if (cell.type === "bgm") {
      stage.bgm = cell.moods;
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

/**
 * 已讀鍵（包 6 §2.6）：格內容的 FNV-1a 64 位元雜湊。分岔與作品包匯入都會重發事件 id，用內容當鍵才對得上；
 * 一字不差的重複句算同一格（罕見、無害）。
 */
export function cellHash(cell: VnCell): string {
  const body =
    cell.type === "dialogue"
      ? `d\u0000${cell.speaker_id ?? cell.speaker_name}\u0000${cell.text}`
      : cell.type === "choices"
        ? `c\u0000${cell.options.join("\u0000")}`
        : `${cell.type}\u0000${cellText(cell)}`;
  let hash = 0xcbf29ce484222325n;
  for (const byte of new TextEncoder().encode(body)) {
    hash ^= BigInt(byte);
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash.toString(16).padStart(16, "0");
}

/** auto：本格打完後等 1200 + 40×字數 ms，上限 6000 */
/** auto 停留時間；factor＝玩家選的 auto 等待檔位倍率 */
export const autoDelay = (text: string, factor = 1) => Math.round(factor * Math.min(6000, 1200 + 40 * charLen(text)));

/** 標題「設定」的打字速度（每字毫秒，0＝瞬間全出）與 auto 等待（倍率）檔位，存全域偏好（包 6 §7.1） */
export const TEXT_SPEEDS = { slow: 50, normal: 25, fast: 12, instant: 0 } as const;
export const AUTO_WAITS = { short: 0.5, normal: 1, long: 2 } as const;
export type TextSpeed = keyof typeof TEXT_SPEEDS;
export type AutoWait = keyof typeof AUTO_WAITS;

/** 偏好值 → 檔位；沒存或認不得就回預設檔 normal（＝原本寫死的速度） */
export function pickLevel<T extends string>(levels: Record<T, number>, value: unknown): T {
  return typeof value === "string" && Object.prototype.hasOwnProperty.call(levels, value) ? (value as T) : ("normal" as T);
}

export type RerollError = "busy" | "stale" | "failed" | "unavailable";

/** `vn_reroll_choices` 的錯誤字串是 `VN_REROLL_<碼>: 說明`（invoke 可能包一層 `Error: `） */
export function rerollError(raw: unknown): RerollError | null {
  const match = /^(?:Error:\s*)?VN_REROLL_(BUSY|STALE|FAILED|UNAVAILABLE):/.exec(String(raw));
  return match ? (match[1].toLowerCase() as RerollError) : null;
}
