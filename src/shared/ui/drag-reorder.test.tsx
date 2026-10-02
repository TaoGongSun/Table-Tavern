// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useDragReorder } from "./drag-reorder";

interface Item {
  id: string;
}

const ITEMS: Item[] = [{ id: "a" }, { id: "b" }];

function List({ onReorder }: { onReorder: (ordered: Item[]) => void }) {
  const drag = useDragReorder(ITEMS, (item) => item.id, onReorder);
  return (
    <div>
      {drag.order.map((item) => (
        <div key={item.id} id={`row-${item.id}`} {...drag.rowProps(item)}>
          <button type="button" id={`main-${item.id}`} data-drag-handle>
            main
          </button>
          <button type="button" id={`pencil-${item.id}`}>
            edit
          </button>
        </div>
      ))}
    </div>
  );
}

describe("useDragReorder", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => {
      root?.unmount();
    });
    host?.remove();
    root = null;
    host = null;
    vi.restoreAllMocks();
  });

  function mount(onReorder: (ordered: Item[]) => void) {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => {
      root?.render(<List onReorder={onReorder} />);
    });
    // happy-dom 不排版：兩列各 40 高，依序疊放，讓相鄰交換的中線有值可比
    ITEMS.forEach((item, index) => {
      const row = document.getElementById(`row-${item.id}`)!;
      vi.spyOn(row, "getBoundingClientRect").mockReturnValue({
        top: index * 40,
        height: 40,
        bottom: index * 40 + 40,
        left: 0,
        right: 100,
        width: 100,
        x: 0,
        y: index * 40,
        toJSON: () => ({}),
      });
    });
  }

  function dragFrom(targetId: string, toY: number) {
    const target = document.getElementById(targetId)!;
    act(() => {
      target.dispatchEvent(
        new PointerEvent("pointerdown", { bubbles: true, button: 0, clientY: 20 }),
      );
    });
    act(() => {
      window.dispatchEvent(new PointerEvent("pointermove", { clientY: toY }));
    });
    act(() => {
      window.dispatchEvent(new PointerEvent("pointerup", { clientY: toY }));
    });
  }

  it("starts a drag from a button marked as the drag handle", () => {
    const onReorder = vi.fn();
    mount(onReorder);
    dragFrom("main-a", 70);
    expect(onReorder).toHaveBeenCalledTimes(1);
    expect(onReorder.mock.calls[0][0].map((item: Item) => item.id)).toEqual(["b", "a"]);
  });

  it("does not start a drag from any other button inside the row", () => {
    const onReorder = vi.fn();
    mount(onReorder);
    dragFrom("pencil-a", 70);
    expect(onReorder).not.toHaveBeenCalled();
  });

  it("a press that never leaves the threshold stays a plain click", () => {
    const onReorder = vi.fn();
    mount(onReorder);
    dragFrom("main-a", 22);
    expect(onReorder).not.toHaveBeenCalled();
  });
});
