// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import { DicePoolView } from "./DicePoolView";
(
  globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

it("未引用時仍有收合池列、錯面使用文字警告，舊事件沒有池列", async () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  await act(() =>
    root.render(
      <DicePoolView pool="d20:[19]" text="沃恩等待" language="zh-TW" />,
    ),
  );
  expect(host.querySelector("details")?.open).toBe(false);
  expect(host.querySelector("details")?.className).toContain("dice-idle");
  expect(host.querySelector("summary")?.textContent).toBe("骰池");
  await act(() =>
    root.render(
      <DicePoolView pool="d20:[19]" text="（ d20 # 1 = 3 ）" language="en" />,
    ),
  );
  expect(host.querySelector(".dice-cell.dice-error")?.textContent).toContain(
    "GM wrote 3; actual roll is 19",
  );
  // 收合列也看得出有不符
  expect(host.querySelector("summary")?.textContent).toBe("Dice pool ⚠ 1");
  await act(() =>
    root.render(
      <DicePoolView pool="d20:[19]" text="(d20#1=19)" language="zh-TW" />,
    ),
  );
  const used = host.querySelector<HTMLElement>(".dice-used");
  expect(used?.textContent).toBe("#1=19 ✓");
  expect(used?.title).toBe("GM 用了");
  expect(host.querySelector("summary")?.textContent).toBe("骰池");
  await act(() => root.render(<DicePoolView text="沃恩等待" language="en" />));
  expect(host.innerHTML).toBe("");
  await act(() => root.unmount());
});
