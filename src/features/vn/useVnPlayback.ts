// VN 播放器的游標：斷格推進、打字機（40 字／秒）、auto、skip、結尾狀態（方案 §3）。
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { VnCell } from "../../shared/contracts/backend-contracts";
import type { Snapshot } from "./useVnCells";
import {
  autoDelay,
  cellText,
  charLen,
  entryKey,
  isPlayable,
  lastFormal,
  realignCursor,
  type Cursor,
  type FlatCell,
} from "./vn-cells";

const CHAR_MS = 25;
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
}: {
  cells: FlatCell[];
  lastEventKey: string | null;
  /** 正式格屬於哪一幕：變了（換幕、重玩、分岔）就丟掉舊幕的游標，照進桌位置重新定位 */
  scene: number | null;
  tail: boolean;
  generating: boolean;
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
      const key = entryKey(cells, playable);
      return key ? { key, shown: 0 } : null;
    }
    const last = lastFormal(playable);
    if (last) return { key: last.key, shown: Infinity };
    return playable[0] ? { key: playable[0].key, shown: 0 } : null;
  }, [cursor, cells, playable]);
  const effectiveRef = useRef(effective);
  effectiveRef.current = effective;

  const pos = effective ? playable.findIndex((cell) => cell.key === effective.key) : -1;
  const current = pos >= 0 ? playable[pos] : undefined;
  const next = playable[pos + 1];
  const text = current ? cellText(current.cell) : "";
  const full = charLen(text);
  const instant = skip || reducedMotion();
  const shown = instant ? full : Math.min(effective?.shown ?? 0, full);
  const typing = shown < full;
  const tailing = tail && current?.key === playable[playable.length - 1]?.key && current?.kind === "pending";

  useEffect(() => {
    if (cursor === null && effective !== null) {
      setCursor(effective);
      setWantNext(false);
    }
  }, [cursor, effective]);

  const key = current?.key;
  useEffect(() => {
    if (!typing || !key) return;
    const id = setInterval(() => {
      setCursor((c) => (c && c.key === key ? { key, shown: c.shown + 1 } : c));
    }, CHAR_MS);
    return () => clearInterval(id);
  }, [key, typing]);

  const advance = useCallback(() => {
    if (!current) return setWantNext(true);
    if (typing) return setCursor({ key: current.key, shown: Infinity });
    if (next) return setCursor({ key: next.key, shown: 0 });
    setWantNext(true);
  }, [current, typing, next]);

  // 游標還沒定位時第一格本身就是「下一格」，由上面的定位 effect 接手
  useEffect(() => {
    if (wantNext && next && cursor !== null) {
      setCursor({ key: next.key, shown: 0 });
      setWantNext(false);
    }
  }, [wantNext, next, cursor]);

  useEffect(() => {
    if (!auto || skip || typing || tailing || !next) return;
    const id = setTimeout(() => setCursor({ key: next.key, shown: 0 }), autoDelay(text));
    return () => clearTimeout(id);
  }, [auto, skip, typing, tailing, next, text]);

  useEffect(() => {
    if (!skip) return;
    if (!next) return setSkip(false);
    const id = setTimeout(() => setCursor({ key: next.key, shown: Infinity }), SKIP_MS);
    return () => clearTimeout(id);
  }, [skip, next]);

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
    if (generating) end = "waiting";
    else end = (current && TERMINAL_END[current.cell.type]) || "continue";
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
    expectNext: () => setWantNext(true),
    auto,
    setAuto,
    skip,
    setSkip,
    realign,
  };
}
