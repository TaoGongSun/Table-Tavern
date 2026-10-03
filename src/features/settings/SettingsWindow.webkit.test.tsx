// 真實 WebKit（Tauri macOS 的引擎）才看得到的分頁列焦點可見性：npm run test:webkit。
// 點擊、按鍵一律經 provider 送真輸入。每個案例先用滑鼠點一顆分頁：WebKit 記得「上次焦點來自滑鼠」時，
// 程式移焦不會符合 :focus-visible（見 .ai/plans/settings-tabs-focus-visible.md）。
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

function Harness({ request }: { request: Request }) {
  const [config, setConfig] = useState(CONFIG);
  return (
    <SettingsWindow
      config={config}
      onSaved={setConfig}
      onPreference={() => {}}
      sponsorUnlocked={false}
      onSponsorUnlocked={() => {}}
      onClose={() => {}}
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
  await page.viewport(414, 896);
});

const tabs = () => [...document.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
const tablist = () => document.querySelector<HTMLElement>('[role="tablist"]')!;
const selectedIndex = () => tabs().findIndex((tab) => tab.getAttribute("aria-selected") === "true");
const focusedIndex = () => tabs().indexOf(document.activeElement as HTMLButtonElement);
const ringed = (node: Element) => getComputedStyle(node).outlineStyle !== "none";
const marked = (node: HTMLElement) => node.dataset.focusRing !== undefined;
const ringedTabs = () => tabs().flatMap((tab, index) => (ringed(tab) ? [index] : []));

async function press(key: string) {
  await userEvent.keyboard(`{${key}}`);
}

async function clickTab(index: number) {
  await userEvent.click(tabs()[index]);
  await vi.waitFor(() => expect(focusedIndex()).toBe(index));
}

// 舊引擎：focus() 不認 focusVisible，只剩 data-focus-ring 兜底
function stripFocusVisible() {
  const native = HTMLElement.prototype.focus;
  vi.spyOn(HTMLElement.prototype, "focus").mockImplementation(function (
    this: HTMLElement,
    options?: FocusOptions,
  ) {
    native.call(this, options ? { preventScroll: options.preventScroll } : undefined);
  });
}

// 外框畫在分頁自己的框內，且分頁整顆在分頁列可視範圍裡：overflow-x 裁不到外框
function expectRingInside(index: number) {
  const tab = tabs()[index];
  const style = getComputedStyle(tab);
  expect(style.outlineStyle).not.toBe("none");
  expect(parseFloat(style.outlineOffset) + parseFloat(style.outlineWidth)).toBeLessThanOrEqual(0);
  const box = tab.getBoundingClientRect();
  const view = tablist().getBoundingClientRect();
  expect(box.left).toBeGreaterThanOrEqual(view.left - 0.5);
  expect(box.right).toBeLessThanOrEqual(view.left + tablist().clientWidth + 0.5);
}

describe("SettingsWindow tabs on WebKit", () => {
  it("arrow keys after a mouse-focused tab show a visible focus on every step", async () => {
    await clickTab(0);
    expect(ringedTabs()).toEqual([]);
    await press("ArrowRight");
    expect(focusedIndex()).toBe(1);
    expect(ringedTabs()).toEqual([1]);
    await press("ArrowLeft");
    expect(focusedIndex()).toBe(0);
    expect(ringedTabs()).toEqual([0]);
    await press("End");
    expect(focusedIndex()).toBe(4);
    expect(ringedTabs()).toEqual([4]);
    await press("Home");
    expect(focusedIndex()).toBe(0);
    expect(ringedTabs()).toEqual([0]);
    // 方向鍵只移焦點，不切頁
    expect(selectedIndex()).toBe(0);
  });

  it("Home on the first tab and End on the last still light the ring", async () => {
    await clickTab(0);
    await press("Home");
    expect(focusedIndex()).toBe(0);
    expect(ringedTabs()).toEqual([0]);
    await clickTab(4);
    expect(ringedTabs()).toEqual([]);
    await press("End");
    expect(focusedIndex()).toBe(4);
    expect(ringedTabs()).toEqual([4]);
  });

  it("a mouse click after arrows clears the ring, on the same tab or another", async () => {
    await clickTab(0);
    await press("ArrowRight");
    expect(ringedTabs()).toEqual([1]);
    await clickTab(1);
    expect(marked(tabs()[1])).toBe(false);
    expect(ringedTabs()).toEqual([]);
    await press("ArrowRight");
    expect(ringedTabs()).toEqual([2]);
    await clickTab(3);
    expect(tabs().some(marked)).toBe(false);
    expect(ringedTabs()).toEqual([]);
  });

  it("a mouse click clears a ring the engine drew on its own, with no marker", async () => {
    await clickTab(1);
    // 沒被攔的按鍵讓 WebKit 自己把當下焦點判成 :focus-visible，不經 focusFrom
    await userEvent.keyboard("a");
    expect(tabs()[1].matches(":focus-visible")).toBe(true);
    expect(marked(tabs()[1])).toBe(false);
    await clickTab(1);
    expect(ringedTabs()).toEqual([]);
  });

  it("without focusVisible support, a mouse click on the arrowed-to tab clears the ring", async () => {
    stripFocusVisible();
    await clickTab(0);
    await press("ArrowRight");
    expect(marked(tabs()[1])).toBe(true);
    expect(ringedTabs()).toEqual([1]);
    await clickTab(1);
    expect(marked(tabs()[1])).toBe(false);
    expect(ringedTabs()).toEqual([]);
  });

  it("Enter and Space activate the tab and keep the ring", async () => {
    await clickTab(0);
    await press("ArrowRight");
    await press("ArrowRight");
    await press("Enter");
    await vi.waitFor(() => expect(selectedIndex()).toBe(2));
    expect(focusedIndex()).toBe(2);
    expect(ringedTabs()).toEqual([2]);
    await press("ArrowRight");
    await userEvent.keyboard(" ");
    await vi.waitFor(() => expect(selectedIndex()).toBe(3));
    expect(focusedIndex()).toBe(3);
    expect(ringedTabs()).toEqual([3]);
  });

  it("cancelling the discard keeps the tab; the arrowed-to tab keeps focus and ring", async () => {
    const answers: ((leave: boolean) => void)[] = [];
    backend.confirm = () => new Promise((resolve) => answers.push(resolve));
    await clickTab(1);
    await userEvent.fill(document.querySelector<HTMLInputElement>('input[type="number"]')!, "5");
    await clickTab(1);
    await press("ArrowRight");
    await press("Enter");
    await vi.waitFor(() => expect(backend.confirmCalls).toBe(1));
    await act(async () => answers.shift()!(false));
    expect(selectedIndex()).toBe(1);
    expect(focusedIndex()).toBe(2);
    expect(ringedTabs()).toEqual([2]);
    // 取消流程跑完、確認窗鎖已解：再按一次會重新問
    await press("Enter");
    await vi.waitFor(() => expect(backend.confirmCalls).toBe(2));
    await act(async () => answers.shift()!(false));
    expect(selectedIndex()).toBe(1);
  });

  it("while saving, tab activation is refused and focus stays where the player put it", async () => {
    backend.update = () => new Promise(() => {});
    await clickTab(1);
    await userEvent.fill(document.querySelector<HTMLInputElement>('input[type="number"]')!, "5");
    await userEvent.click(document.querySelector<HTMLButtonElement>(".settings-save")!);
    await clickTab(0);
    expect(selectedIndex()).toBe(1);
    expect(ringedTabs()).toEqual([]);
    await press("End");
    await press("Enter");
    expect(selectedIndex()).toBe(1);
    expect(focusedIndex()).toBe(4);
    expect(ringedTabs()).toEqual([4]);
  });

  it("an external switch that unmounts the focused field rescues focus with a ring", async () => {
    await render({ initialTab: "ai", requestKey: 1 });
    await vi.waitFor(() => expect(selectedIndex()).toBe(1));
    await userEvent.click(document.querySelector<HTMLInputElement>('input[type="number"]')!);
    await render({ initialTab: "versions", requestKey: 2 });
    await vi.waitFor(() => expect(selectedIndex()).toBe(3));
    expect(focusedIndex()).toBe(3);
    expect(ringedTabs()).toEqual([3]);
  });

  it("an external switch leaves focus alone while it is still inside the window", async () => {
    await clickTab(0);
    await render({ initialTab: "versions", requestKey: 1 });
    await vi.waitFor(() => expect(selectedIndex()).toBe(3));
    expect(focusedIndex()).toBe(0);
    expect(ringedTabs()).toEqual([]);
  });

  it("without focusVisible support, a narrow tab bar scrolls and draws the ring inside", async () => {
    stripFocusVisible();
    await page.viewport(320, 600);
    await clickTab(0);
    expect(tablist().scrollWidth).toBeGreaterThan(tablist().clientWidth);
    await press("End");
    expect(focusedIndex()).toBe(4);
    expect(marked(tabs()[4])).toBe(true);
    expectRingInside(4);
    await press("Home");
    expect(focusedIndex()).toBe(0);
    expect(marked(tabs()[4])).toBe(false);
    expectRingInside(0);
  });
});
