// @vitest-environment happy-dom
// Claude 訂閱超額提示：整個 app 執行期間只出一次，不分桌；卸載空窗不漏不重。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";

type Handler = (event: { payload: unknown }) => void;
const bus = vi.hoisted(() => ({
  handlers: new Set<(event: { payload: unknown }) => void>(),
  // 測試可把 listen 的註冊卡住，模擬非同步註冊尚未完成就卸載
  gate: null as Promise<void> | null,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (_name: string, handler: Handler) => {
    if (bus.gate) await bus.gate;
    bus.handlers.add(handler);
    return () => bus.handlers.delete(handler);
  }),
}));

import { ClaudeOverageToast, resetClaudeOverageForTest } from "./ClaudeOverageToast";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const emit = (payload: unknown) =>
  act(() => [...bus.handlers].forEach((handler) => handler({ payload })));

describe("ClaudeOverageToast", () => {
  let root: Root;
  let host: HTMLDivElement;

  const mount = async () => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => {
      root.render(<ClaudeOverageToast />);
      await Promise.resolve();
    });
  };
  const unmount = () => {
    act(() => root.unmount());
    host.remove();
  };
  const text = () => host.querySelector(".claude-overage-toast")?.textContent ?? "";

  beforeEach(async () => {
    bus.handlers.clear();
    bus.gate = null;
    resetClaudeOverageForTest();
  });

  afterEach(() => {
    if (host?.isConnected) unmount();
  });

  it("三種觀測各配一句；只有整段 1 小時才講 2 倍，拿不到用量不講倍數", async () => {
    for (const [observed, key] of [
      ["one-hour", "claudeOverageOneHour"],
      ["other", "claudeOverageOther"],
      ["unknown", "claudeOverageUnknown"],
    ] as const) {
      resetClaudeOverageForTest();
      await mount();
      emit({ eventId: observed, observed });
      expect(text()).toContain(t(key));
      unmount();
    }
    expect(t("claudeOverageUnknown")).not.toMatch(/2|1\.25/);
  });

  it("多桌多次、同一拍連發兩個，只出一次；關掉、重掛也不再出", async () => {
    await mount();
    act(() =>
      [...bus.handlers].forEach((handler) => {
        handler({ payload: { eventId: "a", observed: "one-hour" } });
        handler({ payload: { eventId: "b", observed: "other" } });
      }),
    );
    expect(text()).toContain(t("claudeOverageOneHour"));
    expect(host.querySelectorAll(".claude-overage-toast")).toHaveLength(1);
    act(() => (host.querySelector("button") as HTMLButtonElement).click());
    expect(text()).toBe("");
    emit({ eventId: "c", observed: "one-hour" }); // 另一桌又超額
    expect(text()).toBe("");
    unmount();
    await mount(); // 離桌再回來（重掛）
    emit({ eventId: "d", observed: "one-hour" });
    expect(text()).toBe("");
  });

  it("listen 還沒註冊完就卸載：不留監聽、不耗名額，重掛後下一個事件照常顯示", async () => {
    let open!: () => void;
    bus.gate = new Promise<void>((resolve) => (open = resolve));
    await mount();
    unmount();
    await act(async () => {
      open();
      await bus.gate;
      await Promise.resolve();
    });
    expect(bus.handlers.size).toBe(0);
    bus.gate = null;
    await mount();
    emit({ eventId: "e", observed: "other" });
    expect(text()).toContain(t("claudeOverageOther"));
  });
});
