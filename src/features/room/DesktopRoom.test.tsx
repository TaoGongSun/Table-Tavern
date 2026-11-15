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

it("交給手機的連結出來後彈層照樣收得起來，再按 ⋯ 還在；手機配對成房主後不再顯示", () => {
  const root = createRoot(document.body.appendChild(document.createElement("div")));
  const view = (hostId: string) => ({ room: { hostId }, me: { memberId: "m1", nickname: "" }, members: [], cards: [] });
  const ctl = { status: null, room: null, view: view("m1"), handoff: "https://room.test/#k=1&p=PAIR", nickname: "", busy: false } as unknown as DesktopRoomCtl;
  const render = () => act(() => root.render(<DesktopRoom ctl={ctl} seat={{} as never} />));
  const link = () => document.querySelector<HTMLInputElement>('[data-testid="room-popover"] .room-value[readonly]')?.value;
  const open = () => act(() => document.querySelector<HTMLButtonElement>("[aria-expanded]")!.click());
  render();
  expect(document.querySelector('[data-testid="room-popover"]')).toBeNull();
  open();
  expect(link()).toBe(ctl.handoff);
  act(() => void document.body.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true })));
  expect(document.querySelector('[data-testid="room-popover"]')).toBeNull();
  open();
  expect(link()).toBe(ctl.handoff);
  ctl.view = view("m2") as never;
  render();
  expect(link()).toBeUndefined();
  act(() => root.unmount());
});
