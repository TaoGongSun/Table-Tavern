// 真實 WebKit（Tauri macOS 的引擎）下的設定分頁按鈕列：npm run test:webkit。
// 分頁是普通按鈕，鍵盤只走原生 Tab／Enter／Space（.ai/plans/settings-tabs-tab-order.md）。
// macOS WebKit 一般 Tab 不停按鈕，這裡一律用 Option+Tab；滑鼠點沒有 tabindex 的按鈕不會給它焦點，
// 所以起點用程式 focus() 建立，再用真按鍵移動。
import { act, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import "../../App.css";
import type { AppConfig } from "../../shared/contracts/backend-contracts";

const backend = vi.hoisted(() => ({
  update: null as null | (() => Promise<unknown>),
  confirm: null as null | (() => Promise<boolean>),
  confirmCalls: 0,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    switch (cmd) {
      case "update_config":
        return backend.update!();
      case "smart_free_recommendations":
        return { limited: [], stable: [] };
      case "smart_free_status":
        return { model: "", freeDaily: null };
      case "usage_report":
        return { worlds: [] };
      case "detect_clis":
        return [];
      default:
        return null;
    }
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(() => {
    backend.confirmCalls += 1;
    return backend.confirm!();
  }),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { SettingsWindow, type SettingsTab } from "./SettingsWindow";

const CONFIG: AppConfig = {
  api_keys: {},
  tier_models: {},
  preferences: { transport: "api", language: "zh-TW" },
};

type Request = { initialTab: SettingsTab; requestKey: number };

function Harness({ request, onClose = () => {} }: { request: Request; onClose?: () => void }) {
  const [config, setConfig] = useState(CONFIG);
  return (
    <SettingsWindow
      config={config}
      onSaved={setConfig}
      onPreference={() => {}}
      sponsorUnlocked={false}
      onSponsorUnlocked={() => {}}
      onClose={onClose}
      initialTab={request.initialTab}
      requestKey={request.requestKey}
      currentWorld=""
      versionTab={<div>versions</div>}
    />
  );
}

let root: Root | null = null;
let host: HTMLDivElement | null = null;
let noTransition: HTMLStyleElement | null = null;

async function render(request: Request) {
  await act(async () => root!.render(<Harness request={request} />));
}

beforeEach(async () => {
  backend.update = async () => CONFIG;
  backend.confirm = async () => true;
  backend.confirmCalls = 0;
  noTransition = document.createElement("style");
  noTransition.textContent = "*, *::before, *::after { transition: none !important; }";
  document.head.appendChild(noTransition);
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
  await render({ initialTab: "appearance", requestKey: 0 });
});

afterEach(async () => {
  act(() => root?.unmount());
  host?.remove();
  noTransition?.remove();
  vi.restoreAllMocks();
  document.getElementById("trigger")?.remove();
  await page.viewport(414, 896);
});

const tabs = () => [...document.querySelectorAll<HTMLButtonElement>(".settings-tabs button")];
const nav = () => document.querySelector<HTMLElement>("nav.settings-tabs")!;
const dialog = () => document.querySelector<HTMLDialogElement>(".settings-modal")!;
const closeButton = () => document.querySelector<HTMLButtonElement>(".settings-close")!;
const panel = () => document.querySelector<HTMLElement>(".settings-panel")!;
const current = () => tabs().flatMap((tab, index) => (tab.getAttribute("aria-current") === "true" ? [index] : []));
const selectedIndex = () => {
  const marked = current();
  // 任何時候恰好一顆帶選中標記
  expect(marked).toHaveLength(1);
  return marked[0];
};
const focusedIndex = () => tabs().indexOf(document.activeElement as HTMLButtonElement);
const ringed = (node: Element) => getComputedStyle(node).outlineStyle !== "none";

const OPT_TAB = "{Alt>}{Tab}{/Alt}";
const OPT_SHIFT_TAB = "{Alt>}{Shift>}{Tab}{/Shift}{/Alt}";

async function press(key: string) {
  await userEvent.keyboard(`{${key}}`);
}

async function clickTab(index: number) {
  await userEvent.click(tabs()[index]);
  await vi.waitFor(() => expect(selectedIndex()).toBe(index));
}

// 程式 focus 第一顆當起點，再用 Option+Tab 走到 index：最後一步是真按鍵，外框照原生 :focus-visible
async function tabTo(index: number) {
  tabs()[0].focus();
  for (let i = 0; i < index; i++) await userEvent.keyboard(OPT_TAB);
  expect(focusedIndex()).toBe(index);
}

// 外框畫在按鈕自己的框內，且按鈕整顆在按鈕列可視範圍裡：overflow-x 裁不到外框
function expectRingInside(index: number) {
  const tab = tabs()[index];
  const style = getComputedStyle(tab);
  expect(style.outlineStyle).not.toBe("none");
  expect(parseFloat(style.outlineOffset) + parseFloat(style.outlineWidth)).toBeLessThanOrEqual(0);
  const box = tab.getBoundingClientRect();
  const view = nav().getBoundingClientRect();
  expect(box.left).toBeGreaterThanOrEqual(view.left - 0.5);
  expect(box.right).toBeLessThanOrEqual(view.left + nav().clientWidth + 0.5);
}

async function dirtyAi() {
  await clickTab(1);
  await userEvent.fill(document.querySelector<HTMLInputElement>('input[type="number"]')!, "5");
}

describe("SettingsWindow tab buttons on WebKit", () => {
  it("Option+Tab walks every tab button in order, then the close button, and back", async () => {
    await tabTo(0);
    for (let i = 1; i < 5; i++) {
      await userEvent.keyboard(OPT_TAB);
      expect(focusedIndex()).toBe(i);
      expect(ringed(tabs()[i])).toBe(true);
    }
    await userEvent.keyboard(OPT_TAB);
    expect(document.activeElement).toBe(closeButton());
    for (let i = 4; i >= 0; i--) {
      await userEvent.keyboard(OPT_SHIFT_TAB);
      expect(focusedIndex()).toBe(i);
    }
    // 走動只移焦點，不切頁
    expect(selectedIndex()).toBe(0);
  });

  it("a mouse click switches the tab and moves the single current mark", async () => {
    for (const index of [2, 4, 0, 3]) {
      await clickTab(index);
      expect(current()).toEqual([index]);
    }
  });

  it("Enter and Space switch through the click and keep a visible ring", async () => {
    await tabTo(2);
    await press("Enter");
    await vi.waitFor(() => expect(selectedIndex()).toBe(2));
    expect(focusedIndex()).toBe(2);
    expect(ringed(tabs()[2])).toBe(true);
    await userEvent.keyboard(OPT_TAB);
    await userEvent.keyboard(" ");
    await vi.waitFor(() => expect(selectedIndex()).toBe(3));
    expect(focusedIndex()).toBe(3);
    expect(ringed(tabs()[3])).toBe(true);
  });

  it("Enter and Space go through the discard guard: cancel, failure and accept", async () => {
    const answers: ((leave: boolean) => void)[] = [];
    backend.confirm = () => new Promise((resolve) => answers.push(resolve));
    await dirtyAi();
    await tabTo(3);
    await press("Enter");
    await vi.waitFor(() => expect(backend.confirmCalls).toBe(1));
    await act(async () => answers.shift()!(false));
    expect(selectedIndex()).toBe(1);
    expect(focusedIndex()).toBe(3);

    backend.confirm = () => Promise.reject(new Error("no dialog"));
    await userEvent.keyboard(" ");
    await vi.waitFor(() => expect(backend.confirmCalls).toBe(2));
    await act(async () => {});
    expect(selectedIndex()).toBe(1);
    expect(focusedIndex()).toBe(3);

    backend.confirm = async () => true;
    await press("Enter");
    await vi.waitFor(() => expect(selectedIndex()).toBe(3));
    expect(backend.confirmCalls).toBe(3);
    expect(focusedIndex()).toBe(3);
  });

  it("while saving, tab activation is refused and focus stays where the player put it", async () => {
    backend.update = () => new Promise(() => {});
    await dirtyAi();
    await userEvent.click(document.querySelector<HTMLButtonElement>(".settings-save")!);
    await userEvent.click(tabs()[0]);
    expect(selectedIndex()).toBe(1);
    await tabTo(4);
    await press("Enter");
    expect(selectedIndex()).toBe(1);
    expect(focusedIndex()).toBe(4);
  });

  it("an external switch that unmounts the focused field rescues focus onto the new tab", async () => {
    await render({ initialTab: "ai", requestKey: 1 });
    await vi.waitFor(() => expect(selectedIndex()).toBe(1));
    await userEvent.click(document.querySelector<HTMLInputElement>('input[type="number"]')!);
    await render({ initialTab: "versions", requestKey: 2 });
    await vi.waitFor(() => expect(selectedIndex()).toBe(3));
    expect(focusedIndex()).toBe(3);
  });

  it("an external switch leaves focus alone while it is still inside the window", async () => {
    await tabTo(0);
    await render({ initialTab: "versions", requestKey: 1 });
    await vi.waitFor(() => expect(selectedIndex()).toBe(3));
    expect(focusedIndex()).toBe(0);
    closeButton().focus();
    await render({ initialTab: "ai", requestKey: 2 });
    await vi.waitFor(() => expect(selectedIndex()).toBe(1));
    expect(document.activeElement).toBe(closeButton());
  });

  it("in a narrow window, tabbing to the first and last buttons scrolls them in and draws the ring inside", async () => {
    await page.viewport(320, 600);
    await tabTo(0);
    expect(nav().scrollWidth).toBeGreaterThan(nav().clientWidth);
    for (let i = 1; i < 5; i++) await userEvent.keyboard(OPT_TAB);
    expect(focusedIndex()).toBe(4);
    expectRingInside(4);
    for (let i = 3; i >= 0; i--) await userEvent.keyboard(OPT_SHIFT_TAB);
    expect(focusedIndex()).toBe(0);
    expectRingInside(0);
  });

  it("Tab never reaches the background, background focus() fails, and closing returns focus", async () => {
    const trigger = document.createElement("button");
    trigger.id = "trigger";
    trigger.textContent = "trigger";
    document.body.insertBefore(trigger, host);
    act(() => root!.unmount());
    root = createRoot(host!);
    trigger.focus();
    expect(document.activeElement).toBe(trigger);
    function Openable() {
      const [open, setOpen] = useState(true);
      return open ? (
        <Harness request={{ initialTab: "appearance", requestKey: 0 }} onClose={() => setOpen(false)} />
      ) : null;
    }
    await act(async () => root!.render(<Openable />));
    expect(dialog().contains(document.activeElement)).toBe(true);
    // 從起點出發一路按到它再次出現＝走完整圈（上限 60 步）。一般 Tab 不停按鈕（macOS 預設），起點改用面板
    for (const [keys, start] of [
      [OPT_TAB, tabs()[0]],
      [OPT_SHIFT_TAB, tabs()[0]],
      ["{Tab}", panel()],
      ["{Shift>}{Tab}{/Shift}", panel()],
    ] as const) {
      start.focus();
      const seen = new Set<Element>();
      let steps = 0;
      do {
        await userEvent.keyboard(keys);
        steps += 1;
        const active = document.activeElement!;
        seen.add(active);
        // 每一步不是在 dialog 裡，就是焦點離開了文件（activeElement 回 body、文件失焦）
        if (active === document.body) expect(document.hasFocus(), `${keys} #${steps}`).toBe(false);
        else expect(dialog().contains(active), `${keys} #${steps}: ${active.outerHTML.slice(0, 60)}`).toBe(true);
        expect(active).not.toBe(trigger);
      } while (document.activeElement !== start && steps < 60);
      expect(document.activeElement, `${keys} never came back`).toBe(start);
      expect(seen.has(document.body), `${keys} never left the document`).toBe(true);
      expect(seen.has(panel()), `${keys} skipped the panel`).toBe(true);
    }
    tabs()[0].focus();
    trigger.focus();
    expect(focusedIndex()).toBe(0);
    await press("Escape");
    await vi.waitFor(() => expect(document.querySelector(".settings-modal")).toBeNull());
    await vi.waitFor(() => expect(document.activeElement).toBe(trigger));
  });
});
