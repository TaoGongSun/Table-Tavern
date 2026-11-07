// @vitest-environment happy-dom
// 入門卡：簡中改推矽基流動貼金鑰；其他語系仍是 OpenRouter 一鍵連線。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { setLang, t } from "../../i18n";
import type { AppConfig } from "../../shared/contracts/backend-contracts";

const patches = vi.hoisted(() => [] as Record<string, unknown>[]);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args?: { patch?: Record<string, unknown> }) => {
    if (cmd === "update_config") {
      patches.push(args?.patch ?? {});
      return { api_keys: {}, tier_models: {}, preferences: {} };
    }
    return null;
  }),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { Onboarding } from "./Onboarding";
import { SILICONFLOW_BASE_URL } from "./siliconflow";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const mounted: { root: Root; host: HTMLDivElement }[] = [];
const config: AppConfig = { api_keys: {}, tier_models: {}, preferences: {} };

function mount() {
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  mounted.push({ root, host });
  act(() => root.render(<Onboarding config={config} onSaved={() => {}} />));
  return host;
}

async function flush() {
  await act(async () => {
    for (let i = 0; i < 10; i += 1) await Promise.resolve();
  });
}

afterEach(() => {
  for (const { root, host } of mounted.splice(0)) {
    act(() => root.unmount());
    host.remove();
  }
  patches.length = 0;
  setLang("zh-TW");
});

describe("入門卡依語系分流", () => {
  it("簡中沒有連接 OpenRouter，貼金鑰送出走矽基流動", async () => {
    setLang("zh-CN");
    const host = mount();
    const buttons = Array.from(host.querySelectorAll("button")).map((node) => node.textContent);
    expect(buttons).not.toContain(t("onboardConnectBtn"));
    expect(buttons).toContain(t("cnGetKeyBtn"));
    const key = host.querySelector(`[aria-label="${t("cnKeyLabel")}"]`) as HTMLInputElement;
    expect(key).toBeTruthy();
    act(() => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(key, "sk-sf-test");
      key.dispatchEvent(new Event("input", { bubbles: true }));
    });
    act(() => host.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })));
    await flush();
    expect(patches).toEqual([
      {
        api_keys: { siliconflow: "sk-sf-test" },
        preferences: { transport: "api", base_url: SILICONFLOW_BASE_URL },
      },
    ]);
  });

  it("簡中貼到指令時提示去矽基流動拿金鑰", () => {
    setLang("zh-CN");
    const host = mount();
    const key = host.querySelector(`[aria-label="${t("cnKeyLabel")}"]`) as HTMLInputElement;
    act(() => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(key, "export KEY=sk-abc");
      key.dispatchEvent(new Event("input", { bubbles: true }));
    });
    const alert = host.querySelector('[role="alert"]');
    expect(alert?.textContent).toBe(t("apiKeyLooksPastedSf"));
    expect(alert?.textContent ?? "").not.toContain("OpenRouter");
  });

  it("繁中仍是 OpenRouter 一鍵連線", () => {
    setLang("zh-TW");
    const host = mount();
    expect(host.textContent).toContain(t("onboardConnectBtn"));
    expect(host.querySelector(`[aria-label="${t("cnKeyLabel")}"]`)).toBeNull();
  });
});
