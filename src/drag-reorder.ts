import { PointerEvent as ReactPointerEvent, useRef, useState } from "react";

// 拖曳排序：按住移動超過門檻才算拖曳，門檻內放開仍是單純點擊（角色卡的點擊＝選發言者）
const DRAG_THRESHOLD_PX = 5;

export function useDragReorder<T>(
  items: T[],
  keyOf: (item: T) => string,
  onReorder: (ordered: T[]) => void,
) {
  const [preview, setPreview] = useState<T[] | null>(null);
  const [draggingKey, setDraggingKey] = useState<string | null>(null);
  const rows = useRef(new Map<string, HTMLElement>());
  const dragged = useRef(false);

  // 一次只跟相鄰那列交換：越過鄰居中線就換，換完中線落到指標另一側，高度不一也不會來回抖
  function neighbourStep(y: number, order: T[], from: number): number {
    const midpoint = (index: number) => {
      const item = order[index];
      const row = item === undefined ? undefined : rows.current.get(keyOf(item));
      if (!row) return null;
      const rect = row.getBoundingClientRect();
      return rect.top + rect.height / 2;
    };
    const above = midpoint(from - 1);
    if (above !== null && y < above) return from - 1;
    const below = midpoint(from + 1);
    if (below !== null && y > below) return from + 1;
    return from;
  }

  function startDrag(event: ReactPointerEvent, item: T) {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest("button, a, input, textarea, select")) return;
    const key = keyOf(item);
    const startY = event.clientY;
    let order = items;
    let started = false;

    const move = (moveEvent: globalThis.PointerEvent) => {
      if (!started) {
        if (Math.abs(moveEvent.clientY - startY) < DRAG_THRESHOLD_PX) return;
        started = true;
        dragged.current = true;
        setDraggingKey(key);
      }
      const from = order.findIndex((candidate) => keyOf(candidate) === key);
      const target = neighbourStep(moveEvent.clientY, order, from);
      if (target === from) return;
      const next = order.slice();
      const [moved] = next.splice(from, 1);
      next.splice(target, 0, moved);
      order = next;
      setPreview(next);
    };
    const finish = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
      if (started) {
        onReorder(order);
        // 放開後瀏覽器才補送 click，等它送完再解旗標
        setTimeout(() => (dragged.current = false), 0);
      }
      setDraggingKey(null);
      setPreview(null);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", finish);
  }

  return {
    order: preview ?? items,
    draggingKey,
    justDragged: () => dragged.current,
    rowProps: (item: T) => ({
      onPointerDown: (event: ReactPointerEvent) => startDrag(event, item),
      ref: (element: HTMLElement | null) => {
        if (element) rows.current.set(keyOf(item), element);
        else rows.current.delete(keyOf(item));
      },
    }),
  };
}
