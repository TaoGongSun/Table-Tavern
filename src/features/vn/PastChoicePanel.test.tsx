// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => [{ option: "我自己打的一句", tip: 1, ended: false, scene: 1, answer_event_id: null }]),
}));
import { PastChoicePanel } from "./PastChoicePanel";
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

it("聊天視圖自己打的那句就是本分支選的：按了往下走，不切分支", async () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const onCurrent = vi.fn();
  const onSwitch = vi.fn();
  await act(async () =>
    root.render(
      <PastChoicePanel worldId="w" eventId="e" options={["甲", "乙"]} answered="我自己打的一句" disabled={false} onCurrent={onCurrent} onSwitch={onSwitch} onFork={() => {}} />,
    ),
  );
  const row = [...host.querySelectorAll("button")].find((b) => b.textContent?.startsWith("我自己打的一句"))!;
  expect(row.className).toBe("btn vn-choice");
  await act(async () => row.click());
  expect(onCurrent).toHaveBeenCalledOnce();
  expect(onSwitch).not.toHaveBeenCalled();
  act(() => root.unmount());
});
