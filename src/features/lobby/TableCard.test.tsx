// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { WorldMeta } from "../../shared/contracts/backend-contracts";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));

import { TableCard } from "./TableCard";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const WORLD: WorldMeta = { id: "w1", name: "Alpha", read_only: false, needs_repair: false };

describe("TableCard", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  const onEnter = vi.fn();
  const onRename = vi.fn();
  const onDelete = vi.fn();

  beforeEach(() => {
    onEnter.mockReset();
    onRename.mockReset();
    onDelete.mockReset();
  });

  afterEach(() => {
    act(() => {
      root?.unmount();
    });
    host?.remove();
    root = null;
    host = null;
    document.body.innerHTML = "";
  });

  function mount(world: WorldMeta = WORLD, busy = false) {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => {
      root?.render(
        <TableCard world={world} busy={busy} onEnter={onEnter} onRename={onRename} onDelete={onDelete} />,
      );
    });
  }

  const optionsButton = () =>
    host!.querySelector<HTMLButtonElement>(`[aria-label="${t("itemOptionsAria", { name: "Alpha" })}"]`)!;
  const menuItem = (label: string) =>
    [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
      (item) => item.textContent === label,
    )!;
  const input = () => host!.querySelector<HTMLInputElement>("input");

  function startRename() {
    act(() => optionsButton().click());
    act(() => menuItem(t("lobbyRename")).click());
  }

  function type(value: string) {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
    act(() => {
      setter.call(input()!, value);
      input()!.dispatchEvent(new Event("input", { bubbles: true }));
    });
  }

  it("enters from the cover button and is disabled while busy", () => {
    mount();
    host!.querySelector<HTMLButtonElement>(".table-cover")!.click();
    expect(onEnter).toHaveBeenCalledWith("w1");
    act(() => root?.render(<TableCard world={WORLD} busy onEnter={onEnter} onRename={onRename} onDelete={onDelete} />));
    expect(host!.querySelector<HTMLButtonElement>(".table-cover")!.disabled).toBe(true);
    expect(optionsButton().disabled).toBe(true);
  });

  it("submits a rename once on Enter (form submit), even though the input then blurs", () => {
    mount();
    startRename();
    type("Beta");
    const form = host!.querySelector("form")!;
    act(() => {
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    });
    expect(onRename).toHaveBeenCalledTimes(1);
    expect(onRename).toHaveBeenCalledWith("w1", "Beta");
    expect(input()).toBeNull();
  });

  it("returns focus to the options button after Enter or Escape, but not after a blur", () => {
    mount();
    startRename();
    type("Beta");
    act(() => {
      host!.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    });
    expect(document.activeElement).toBe(optionsButton());

    document.body.focus();
    startRename();
    act(() => {
      input()!.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    });
    expect(document.activeElement).toBe(optionsButton());

    (document.activeElement as HTMLElement).blur();
    startRename();
    type("Gamma");
    act(() => {
      input()!.dispatchEvent(new FocusEvent("focusout", { bubbles: true }));
    });
    expect(document.activeElement).not.toBe(optionsButton());
  });

  it("submits on blur", () => {
    mount();
    startRename();
    type("Gamma");
    act(() => {
      input()!.dispatchEvent(new FocusEvent("focusout", { bubbles: true }));
    });
    expect(onRename).toHaveBeenCalledWith("w1", "Gamma");
    expect(input()).toBeNull();
  });

  it("Escape cancels without submitting", () => {
    mount();
    startRename();
    type("Nope");
    act(() => {
      input()!.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    });
    expect(onRename).not.toHaveBeenCalled();
    expect(input()).toBeNull();
  });

  it("read-only and needs-repair tables cannot be renamed but can still be deleted", () => {
    mount({ ...WORLD, read_only: true });
    expect(host!.textContent).toContain(t("readOnlyBadge"));
    act(() => optionsButton().click());
    expect(menuItem(t("lobbyRename")).getAttribute("aria-disabled")).toBe("true");
    act(() => menuItem(t("lobbyRename")).click());
    expect(input()).toBeNull();
    act(() => menuItem(t("deleteTableTitle")).click());
    expect(onDelete).toHaveBeenCalledWith("w1");
  });
});
