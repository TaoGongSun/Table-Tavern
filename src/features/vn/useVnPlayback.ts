// VN 播放器的游標：斷格推進、逐格回溯、打字機（速度依設定，預設 40 字／秒）、auto、skip、結尾狀態（包 2 §3、包 6 §2.5）。
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { VnCell } from "../../shared/contracts/backend-contracts";
import type { Snapshot } from "./useVnCells";
import {
  autoDelay,
  cellText,
  charLen,
  entryCell,
  entryKey,
  isPlayable,
  isTerminal,
  lastFormal,
  liveChoice,
  realignCursor,
  type Cursor,
  type EntryAt,
  type FlatCell,
} from "./vn-cells";

const SKIP_MS = 80;

const reducedMotion = () =>
  typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

/** 結尾狀態；chapter＝停在章末格（章結束流程）、ended＝停在結局格（完結畫面） */
export type EndState = "waiting" | "choices" | "chapter" | "ended" | "continue" | null;

const TERMINAL_END: Record<string, EndState> = { choices: "choices", chapter: "chapter", ending: "ended" };

export function useVnPlayback({
  cells,
  lastEventKey,
  tail,
  generating,
  scene,
  entry,
  resumeKey,
  hold,
  isRead,
  paused,
  charMs,
  autoFactor,
}: {
  cells: FlatCell[];
  lastEventKey: string | null;
  /** 正式格屬於哪一幕：變了（換幕、重玩、分岔）就丟掉舊幕的游標，照進桌位置重新定位 */
  scene: number | null;
  tail: boolean;
  generating: boolean;
  /** 進新幕的指定位置（包 6、包 8）：讀檔那一格、最後一格（分岔、切換分支後直接回到面板）、重播桌的幕首等 */
  entry: { at: EntryAt } | null;
  /** 這幕上次讀到的格（`vn-progress.json`），沒有指定位置時用 */
  resumeKey: string | undefined;
  /** 進度還沒讀回來：先不定位，免得落在預設位置後續讀位置來不及用 */
  hold: boolean;
  /** 這格讀過了沒（skip 只跳已讀，§2.6） */
  isRead: (cell: FlatCell) => boolean;
  /** 標題畫面或面板蓋著播放器：打字機、auto、skip、等下一格都先停，auto／skip 開關留著，收起後接著跑 */
  paused: boolean;
  /** 打字機每字毫秒（0＝瞬間全出）與 auto 等待倍率：標題設定的檔位，改了立即生效 */
  charMs: number;
  autoFactor: number;
}) {
  const playable = useMemo(
    () => cells.filter((cell) => isPlayable(cell, lastEventKey)),
    [cells, lastEventKey],
  );
  const [cursor, setCursor] = useState<Cursor | null>(null);
  const [auto, setAuto] = useState(false);
  const [skip, setSkip] = useState(false);
  // 玩家已要下一格（按了推進、選項或繼續）但還沒長出來：一長出來就移過去
  const [wantNext, setWantNext] = useState(false);
  const [cursorScene, setCursorScene] = useState(scene);
  if (cursorScene !== scene) {
    setCursorScene(scene);
    setCursor(null);
  }

  // 游標鍵不見了（選項被玩家句消耗、收回、撤掉沒落檔的暫定段）退到最後一個正式可停格；還沒定位就用進桌位置
  const effective = useMemo<Cursor | null>(() => {
    if (cursor && playable.some((cell) => cell.key === cursor.key)) return cursor;
    if (cursor === null) {
      if (hold) return null;
      const has = (key: string | null | undefined) => (key && playable.some((cell) => cell.key === key) ? key : null);
      if (entry) {
        const exact = entryCell(cells, playable, entry.at);
        const cell = exact ?? lastFormal(playable);
        if (cell) return { key: cell.key, shown: exact && entry.at.kind !== "last" ? 0 : Infinity };
      }
      const resumed = has(resumeKey);
      if (resumed) return { key: resumed, shown: 0 };
      const key = entryKey(playable);
      return key ? { key, shown: 0 } : null;
    }
    const last = lastFormal(playable);
    if (last) return { key: last.key, shown: Infinity };
    return playable[0] ? { key: playable[0].key, shown: 0 } : null;
  }, [cursor, cells, playable, entry, resumeKey, hold]);
  const effectiveRef = useRef(effective);
  effectiveRef.current = effective;

  const pos = effective ? playable.findIndex((cell) => cell.key === effective.key) : -1;
  const current = pos >= 0 ? playable[pos] : undefined;
  const next = playable[pos + 1];
  const text = current ? cellText(current.cell) : "";
  const full = charLen(text);
  // skip 只對已讀格全出：開 skip 時目前格未讀＝下面的 effect 關掉 skip、這格照常打字（不被 skip 變成已讀）
  const currentRead = current !== undefined && isRead(current);
  const instant = (skip && currentRead) || charMs <= 0 || reducedMotion();
  const shown = instant ? full : Math.min(effective?.shown ?? 0, full);
  const typing = shown < full;
  const tailing = tail && current?.key === playable[playable.length - 1]?.key && current?.kind === "pending";
  const live = useMemo(() => liveChoice(cells), [cells]);

  useEffect(() => {
    if (cursor === null && effective !== null) {
      setCursor(effective);
      setWantNext(false);
    }
  }, [cursor, effective]);

  const key = current?.key;
  useEffect(() => {
    if (paused || !typing || !key) return;
    const id = setInterval(() => {
      setCursor((c) => (c && c.key === key ? { key, shown: c.shown + 1 } : c));
    }, charMs);
    return () => clearInterval(id);
  }, [key, typing, paused, charMs]);

  const advance = useCallback(() => {
    if (!current) return setWantNext(true);
    if (typing) return setCursor({ key: current.key, shown: Infinity });
    if (next) return setCursor({ key: next.key, shown: 0 });
    setWantNext(true);
  }, [current, typing, next]);

  /** 逐格回溯：純顯示，回到上一個可停格（字全出）；幕首就不動。停在終止格時對話框已顯示前一格的字，
   *  再往前一格才看得出動了（實測：從選項面板按上一格第一下像沒反應） */
  const behind = current && isTerminal(current.cell) && pos >= 2 && !isTerminal(playable[pos - 1].cell) ? 2 : 1;
  const previous = pos >= behind ? playable[pos - behind] : pos > 0 ? playable[pos - 1] : undefined;
  const back = useCallback(() => {
    if (!previous) return;
    setSkip(false);
    setWantNext(false);
    setCursor({ key: previous.key, shown: Infinity });
  }, [previous]);

  // 游標還沒定位時第一格本身就是「下一格」，由上面的定位 effect 接手
  useEffect(() => {
    if (!paused && wantNext && next && cursor !== null) {
      setCursor({ key: next.key, shown: 0 });
      setWantNext(false);
    }
  }, [wantNext, next, cursor, paused]);

  useEffect(() => {
    if (paused || !auto || skip || typing || tailing || !next) return;
    const id = setTimeout(() => setCursor({ key: next.key, shown: 0 }), autoDelay(text, autoFactor));
    return () => clearTimeout(id);
  }, [paused, auto, skip, typing, tailing, next, text, autoFactor]);

  // skip 只跳已讀（包 6 §2.6）：目前格或下一格未讀就停在目前格
  const nextRead = next !== undefined && isRead(next);
  useEffect(() => {
    if (!skip || paused) return;
    if (!currentRead || !next || !nextRead) return setSkip(false);
    const id = setTimeout(() => setCursor({ key: next.key, shown: Infinity }), SKIP_MS);
    return () => clearTimeout(id);
  }, [skip, paused, next, currentRead, nextRead]);

  /** 暫定段落檔成正式事件：游標若停在那段裡，對位到正式格（與格的更新同一批） */
  const realign = useCallback((snapshot: Snapshot, eventKey: string, formal: VnCell[]) => {
    const at = effectiveRef.current;
    const prefix = `pending:${snapshot.generation}#`;
    if (!at?.key.startsWith(prefix)) return;
    const index = Number(at.key.slice(prefix.length));
    const aligned = realignCursor(snapshot.cells, { index, shown: at.shown }, formal);
    setCursor({ key: `${eventKey}#${aligned.index}`, shown: aligned.shown });
  }, []);

  let end: EndState = null;
  if (!typing && !next && !tailing) {
    // 過去的選項格（已選）在這裡收尾＝玩家句還沒有回覆：照「繼續」；未選的選項播完夾在後面的角色線才出面板
    const past = current?.cell.type === "choices" && Boolean(current.choice?.answered);
    if (generating) end = "waiting";
    else end = (current && !past && TERMINAL_END[current.cell.type]) || (live ? "choices" : "continue");
  }

  return {
    current,
    /** 目前格在全部格（含玩家句、`[bg]`）裡的位置：舞台摺疊與 log 用 */
    flatIndex: current ? cells.indexOf(current) : cells.length - 1,
    shown,
    typing,
    tailing,
    end,
    playable,
    advance,
    back,
    /** 跳到這一格（同一幕讀檔、重播桌同一幕換位置）；找不到就不動 */
    jump: (at: EntryAt) => {
      const cell = entryCell(cells, playable, at);
      if (!cell) return;
      setSkip(false);
      setWantNext(false);
      setCursor({ key: cell.key, shown: 0 });
    },
    canBack: previous !== undefined,
    /** 本幕還沒選的選項格 */
    live,
    expectNext: () => setWantNext(true),
    auto,
    setAuto,
    skip,
    setSkip,
    realign,
  };
}
