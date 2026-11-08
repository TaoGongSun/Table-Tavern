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
  expect(host.querySelector("summary")?.textContent).toContain("未找到骰號");
  await act(() =>
    root.render(
      <DicePoolView pool="d20:[19]" text="（ d20 # 1 = 3 ）" language="en" />,
    ),
  );
  expect(host.querySelector(".dice-error")?.textContent).toContain(
    "Face mismatch",
  );
  await act(() => root.render(<DicePoolView text="沃恩等待" language="en" />));
  expect(host.innerHTML).toBe("");
  await act(() => root.unmount());
});
