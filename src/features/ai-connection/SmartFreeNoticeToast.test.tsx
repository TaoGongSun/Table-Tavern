// @vitest-environment happy-dom
// turnId 為 null 的免費模型換模事件（非聊天輪）走 App 層非阻塞提示；有 turnId 的交給聊天室，這裡不顯示。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";

type Handler = (event: { payload: unknown }) => void;
const bus = vi.hoisted(() => ({ handlers: new Map<string, Set<(event: { payload: unknown }) => void>>() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: Handler) => {
    const set = bus.handlers.get(name) ?? new Set();
    set.add(handler);
    bus.handlers.set(name, set);
    return () => set.delete(handler);
  }),
}));

import { SmartFreeNoticeToast } from "./SmartFreeNoticeToast";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const emit = (name: string, payload: unknown) =>
  act(() => bus.handlers.get(name)?.forEach((handler) => handler({ payload })));

describe("SmartFreeNoticeToast", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  beforeEach(async () => {
    vi.useFakeTimers();
    bus.handlers.clear();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => {
      root!.render(<SmartFreeNoticeToast />);
      await Promise.resolve();
    });
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    vi.useRealTimers();
  });

  const text = () => host!.querySelector(".smart-free-notice-toast")?.textContent ?? "";

  it("turnId 為 null 的換模事件顯示非阻塞提示，8 秒後自動消失", () => {
    emit("smart-free-failover", { eventId: "e1", world: null, turnId: null, from: "甲", to: "乙", retried: false });
    expect(text()).toContain(t("smartFreeFailover", { from: "甲", to: "乙" }));
    act(() => vi.advanceTimersByTime(8001));
    expect(text()).toBe("");
  });

  it("一般換手事件（switched）也走同一個提示", () => {
    emit("smart-free-model-switched", { eventId: "e2", world: "w", turnId: null, model: "丙" });
    expect(text()).toContain(t("smartFreeSwitched", { model: "丙" }));
  });

  it("有 turnId 的事件不在這裡顯示（交給聊天室）", () => {
    emit("smart-free-failover", { eventId: "e3", world: "w", turnId: "t1", from: "甲", to: "乙", retried: true });
    emit("smart-free-model-switched", { eventId: "e4", world: "w", turnId: "t1", model: "丙" });
    expect(text()).toBe("");
  });

  it("同一 eventId 只顯示一次：收掉後重送同一事件不再跳出", () => {
    const payload = { eventId: "e5", world: null, turnId: null, from: "甲", to: "乙", retried: false };
    emit("smart-free-failover", payload);
    act(() => (host!.querySelector(".smart-free-notice-toast button") as HTMLElement).click());
    expect(text()).toBe("");
    emit("smart-free-failover", payload);
    expect(text()).toBe("");
  });

  it("卸載時取消監聽", () => {
    act(() => root!.unmount());
    root = null;
    expect([...bus.handlers.values()].every((set) => set.size === 0)).toBe(true);
  });
});
