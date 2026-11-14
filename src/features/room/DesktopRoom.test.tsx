// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { DesktopRoomCtl } from "./useDesktopRoom";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
vi.mock("@tauri-apps/api/core", () => ({ invoke: () => Promise.resolve() }));
const { DesktopRoom } = await import("./DesktopRoom");

it("⋯ 彈層：點外面、焦點不在列內按 Esc 都會收起", () => {
  const root = createRoot(document.body.appendChild(document.createElement("div")));
  const ctl = { status: null, room: null, view: null, handoff: null, nickname: "", busy: false } as unknown as DesktopRoomCtl;
  act(() => root.render(<DesktopRoom ctl={ctl} seat={{} as never} />));
  const popover = () => document.querySelector('[data-testid="room-popover"]');
  const open = () => act(() => document.querySelector<HTMLButtonElement>("[aria-expanded]")!.click());
  open();
  act(() => void document.body.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true })));
  expect(popover()).toBeNull();
  open();
  expect(popover()).not.toBeNull();
  act(() => void document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })));
  expect(popover()).toBeNull();
  act(() => root.unmount());
});
